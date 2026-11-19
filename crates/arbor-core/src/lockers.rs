//! "Quem está usando este arquivo?": os programas que mantêm um arquivo aberto e impedem apagar ou mover.
//!
//! * **Windows:** Restart Manager (`RmGetList`), a mesma API que o instalador usa para pedir "feche o app X".
//! * **Linux:** procura em `/proc/<pid>/fd` links para o arquivo (ou, para pastas, para algo dentro dela,
//!   inclusive a pasta de trabalho do processo).
//! * **macOS:** `lsof`.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locker {
    pub pid: u32,
    pub name: String,
}

/// Texto curto para mensagens: "Word (1234), Excel (88)".
pub fn describe(lockers: &[Locker]) -> String {
    lockers.iter().map(|l| format!("{} ({})", l.name, l.pid)).collect::<Vec<_>>().join(", ")
}

/// Programas que estão com `path` aberto. Vazio se ninguém (ou se não deu para descobrir).
pub fn who_uses(path: &Path) -> Vec<Locker> {
    let mut v = platform(path);
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.pid.cmp(&b.pid)));
    v.dedup();
    v.retain(|l| l.pid != std::process::id());
    v
}

/// O erro parece de "arquivo em uso"? (Windows: violação de compartilhamento/bloqueio; Unix: EBUSY/ETXTBSY.)
pub fn is_in_use(e: &std::io::Error) -> bool {
    #[cfg(windows)]
    const CODES: &[i32] = &[32, 33]; // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION
    #[cfg(unix)]
    const CODES: &[i32] = &[16, 26]; // EBUSY, ETXTBSY
    #[cfg(not(any(unix, windows)))]
    const CODES: &[i32] = &[];
    e.raw_os_error().is_some_and(|c| CODES.contains(&c)) || (cfg!(windows) && e.kind() == std::io::ErrorKind::PermissionDenied)
}

/// Se `e` é de arquivo em uso, devolve um erro que diz por quem. Senão, devolve `e` como veio.
pub fn explain(e: std::io::Error, path: &Path) -> std::io::Error {
    if !is_in_use(&e) {
        return e;
    }
    let who = who_uses(path);
    if who.is_empty() {
        return e;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    std::io::Error::new(e.kind(), format!("{name} está em uso por {}. Feche e tente de novo.", describe(&who)))
}

#[cfg(target_os = "linux")]
fn platform(path: &Path) -> Vec<Locker> {
    let Ok(target) = std::fs::canonicalize(path) else { return vec![] };
    let is_dir = target.is_dir();
    let hits = |p: &Path| p == target || (is_dir && p.starts_with(&target));
    let mut out = Vec::new();
    let Ok(procs) = std::fs::read_dir("/proc") else { return out };
    for proc in procs.flatten() {
        let Some(pid) = proc.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
        let base = proc.path();
        let mut uses = std::fs::read_link(base.join("cwd")).is_ok_and(|c| is_dir && hits(&c));
        if !uses {
            if let Ok(fds) = std::fs::read_dir(base.join("fd")) {
                uses = fds.flatten().any(|fd| std::fs::read_link(fd.path()).is_ok_and(|l| hits(&l)));
            }
        }
        if uses {
            let name = std::fs::read_to_string(base.join("comm")).map(|s| s.trim().to_string()).unwrap_or_else(|_| pid.to_string());
            out.push(Locker { pid, name });
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn platform(path: &Path) -> Vec<Locker> {
    let flag = if path.is_dir() { "+D" } else { "--" };
    let Ok(out) = std::process::Command::new("lsof").args(["-F", "pc", flag]).arg(path).output() else { return vec![] };
    let mut v = Vec::new();
    let mut pid = None;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.parse().ok();
        } else if let (Some(c), Some(p)) = (line.strip_prefix('c'), pid) {
            v.push(Locker { pid: p, name: c.to_string() });
        }
    }
    v
}

#[cfg(windows)]
fn platform(path: &Path) -> Vec<Locker> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
    use windows_sys::Win32::System::RestartManager::*;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut session = 0u32;
    let mut key = [0u16; (CCH_RM_SESSION_KEY + 1) as usize];
    // SAFETY: chamadas documentadas do Restart Manager; buffers do tamanho pedido pela própria API.
    unsafe {
        if RmStartSession(&mut session, 0, key.as_mut_ptr()) != 0 {
            return vec![];
        }
        let files = [wide.as_ptr()];
        let mut out = Vec::new();
        if RmRegisterResources(session, 1, files.as_ptr(), 0, std::ptr::null(), 0, std::ptr::null()) == 0 {
            let (mut needed, mut count, mut reasons) = (0u32, 0u32, 0u32);
            let mut infos: Vec<RM_PROCESS_INFO> = Vec::new();
            loop {
                let r = RmGetList(session, &mut needed, &mut count, infos.as_mut_ptr(), &mut reasons);
                if r == ERROR_MORE_DATA {
                    infos = vec![std::mem::zeroed(); needed as usize];
                    count = needed;
                    continue;
                }
                if r == 0 {
                    for info in infos.iter().take(count as usize) {
                        let name = &info.strAppName;
                        let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
                        out.push(Locker { pid: info.Process.dwProcessId, name: String::from_utf16_lossy(&name[..len]) });
                    }
                }
                break;
            }
        }
        RmEndSession(session);
        out
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn platform(_path: &Path) -> Vec<Locker> {
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn acha_o_proprio_processo_segurando_o_arquivo() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("aberto.txt");
        std::fs::write(&f, "x").unwrap();
        let _held = std::fs::File::open(&f).unwrap();
        // `who_uses` tira o próprio processo; a busca crua precisa achá-lo.
        assert!(platform(&f).iter().any(|l| l.pid == std::process::id()));
        assert!(platform(d.path()).iter().any(|l| l.pid == std::process::id()));
        assert!(who_uses(&f).iter().all(|l| l.pid != std::process::id()));
    }

    #[test]
    fn descreve() {
        let v = [Locker { pid: 1, name: "WINWORD.EXE".into() }, Locker { pid: 2, name: "a".into() }];
        assert_eq!(describe(&v), "WINWORD.EXE (1), a (2)");
    }
}
