//! Abrir arquivos: imagens vão para o Vislo quando ele está instalado; o resto, para o app padrão do sistema.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn open(path: &Path) -> Result<(), String> {
    if !path.is_dir() && vislo_core::formats::is_image(path) {
        if let Some(vislo) = vislo() {
            if spawn_vislo(vislo, path).is_ok() {
                return Ok(());
            }
        }
    }
    open::that_detached(path).map_err(|e| e.to_string())
}

/// Abre no app padrão mesmo quando é imagem (comando "Abrir com o app do sistema").
pub fn open_default(path: &Path) -> Result<(), String> {
    open::that_detached(path).map_err(|e| e.to_string())
}

/// "Abrir com…": roda o programa escolhido com o arquivo como argumento (sem shell, nada é interpretado).
/// Aceita nome no PATH ou caminho completo; no macOS, também o nome de um app ("TextEdit").
pub fn open_with(program: &str, file: &Path) -> Result<(), String> {
    let exe = Path::new(program);
    let found = if exe.is_absolute() {
        exe.is_file().then(|| exe.to_path_buf())
    } else if program.contains(['/', '\\']) {
        None
    } else {
        let name = if cfg!(windows) && exe.extension().is_none() { format!("{program}.exe") } else { program.to_string() };
        std::env::var_os("PATH").and_then(|p| search_path(&p, &name))
    };
    if let Some(exe) = found {
        return Command::new(exe).arg(file).spawn().map(drop).map_err(|e| e.to_string());
    }
    #[cfg(target_os = "macos")]
    if !program.contains('/') {
        return Command::new("open").arg("-a").arg(program).arg(file).spawn().map(drop).map_err(|e| e.to_string());
    }
    Err(format!("programa não encontrado: {program}"))
}

/// Diálogo nativo "Abrir com" do Windows.
#[cfg(windows)]
pub fn open_with_dialog(file: &Path) -> Result<(), String> {
    Command::new("rundll32").arg("shell32.dll,OpenAs_RunDLL").arg(file).spawn().map(drop).map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn open_with_dialog(_file: &Path) -> Result<(), String> {
    Err("sem diálogo nativo neste sistema".into())
}

/// Programas que o Explorer oferece "Executar como administrador".
pub fn elevatable(path: &Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
    matches!(ext.as_deref(), Some("exe" | "bat" | "cmd" | "msi"))
}

/// "Executar como administrador": o Windows mostra o UAC e roda o programa elevado.
#[cfg(windows)]
pub fn run_as_admin(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
    let file: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let dir: Vec<u16> = path.parent().unwrap_or(path).as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: todas as strings terminam em zero e vivem até o fim da chamada.
    let code = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), dir.as_ptr(), SW_SHOWNORMAL) }
        as isize;
    // Acima de 32 é sucesso; 5 é acesso negado (o usuário recusou o UAC).
    match code {
        c if c > 32 => Ok(()),
        5 => Err("o pedido de administrador foi recusado".into()),
        c => Err(format!("o Windows não executou (código {c})")),
    }
}

#[cfg(not(windows))]
pub fn run_as_admin(_path: &Path) -> Result<(), String> {
    Err("executar como administrador só existe no Windows".into())
}

/// Mostra o arquivo no gerenciador do sistema (útil para coisas que o arbor ainda não faz).
/// Ctrl+N: outra janela do arbor (outro processo), na mesma pasta ou em "Este Computador".
pub fn new_window(dir: Option<&Path>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.env("ARBOR_MULTI", "1"); // senão a instância única transformaria a janela nova em aba
    if let Some(d) = dir {
        cmd.arg(d);
    }
    cmd.spawn().map(drop).map_err(|e| e.to_string())
}

pub fn reveal(path: &Path) -> Result<(), String> {
    let dir = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
    open::that_detached(dir).map_err(|e| e.to_string())
}

pub fn terminal_here(dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let result = Command::new("cmd")
        .args(["/C", "start", "", "wt", "-d"])
        .arg(dir)
        .spawn()
        .or_else(|_| Command::new("cmd").args(["/C", "start", "cmd"]).current_dir(dir).spawn());
    #[cfg(target_os = "macos")]
    let result = Command::new("open").args(["-a", "Terminal"]).arg(dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = linux_terminals(std::env::var("TERMINAL").ok().as_deref())
        .iter()
        .find_map(|t| Command::new(t).current_dir(dir).spawn().ok())
        .ok_or_else(|| std::io::Error::other("nenhum terminal encontrado (defina $TERMINAL)"));
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// `$TERMINAL` primeiro (só o nome do programa, sem shell), depois os terminais comuns.
#[cfg(all(unix, not(target_os = "macos")))]
fn linux_terminals(env: Option<&str>) -> Vec<String> {
    let common = ["x-terminal-emulator", "gnome-terminal", "konsole", "kitty", "alacritty", "wezterm", "foot", "xterm"];
    let own = env.map(str::trim).filter(|t| !t.is_empty() && !t.contains(char::is_whitespace));
    own.into_iter().chain(common).map(String::from).collect()
}

pub fn vislo() -> Option<&'static Path> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND.get_or_init(find_vislo).as_deref()
}

fn spawn_vislo(vislo: &Path, file: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    if vislo.extension().is_some_and(|e| e == "app") {
        return Command::new("open").arg("-a").arg(vislo).arg(file).spawn().map(|_| ());
    }
    Command::new(vislo).arg(file).spawn().map(|_| ())
}

fn find_vislo() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ARBOR_VISLO").map(PathBuf::from).filter(|p| p.exists()) {
        return Some(p);
    }
    let exe = if cfg!(windows) { "vislo.exe" } else { "vislo" };
    if let Some(p) = std::env::var_os("PATH").and_then(|path| search_path(&path, exe)) {
        return Some(p);
    }
    known_locations().into_iter().find(|p| p.exists())
}

pub fn search_path(path_var: &std::ffi::OsStr, exe: &str) -> Option<PathBuf> {
    std::env::split_paths(path_var).map(|d| d.join(exe)).find(|p| p.is_file())
}

fn known_locations() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(windows)]
    {
        for var in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
                v.push(base.join("Vislo").join("vislo.exe"));
                v.push(base.join("Programs").join("Vislo").join("vislo.exe"));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        v.push(PathBuf::from("/Applications/Vislo.app"));
        if let Some(home) = dirs::home_dir() {
            v.push(home.join("Applications/Vislo.app"));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        v.push(PathBuf::from("/usr/bin/vislo"));
        v.push(PathBuf::from("/usr/local/bin/vislo"));
        if let Some(home) = dirs::home_dir() {
            v.push(home.join(".local/bin/vislo"));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_programs_offer_run_as_admin() {
        assert!(elevatable(Path::new(r"C:\setup.EXE")) && elevatable(Path::new("instalar.msi")));
        assert!(!elevatable(Path::new("foto.jpg")) && !elevatable(Path::new("exe")));
    }

    #[test]
    fn acha_no_path() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("vislo"), "").unwrap();
        let path = std::env::join_paths([PathBuf::from("/nao/existe"), d.path().to_path_buf()]).unwrap();
        assert_eq!(search_path(&path, "vislo"), Some(d.path().join("vislo")));
        assert_eq!(search_path(&path, "outro"), None);
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn terminal_do_ambiente_vem_primeiro() {
        assert_eq!(linux_terminals(Some("kitty"))[..2], ["kitty", "x-terminal-emulator"]);
        assert_eq!(linux_terminals(Some("sh -c x"))[0], "x-terminal-emulator", "com argumentos não é só um programa");
        assert_eq!(linux_terminals(None)[0], "x-terminal-emulator");
    }

    #[test]
    fn abrir_com_recusa_caminho_relativo_e_inexistente() {
        let f = std::env::temp_dir();
        assert!(open_with("../vislo", &f).is_err(), "caminho relativo não é procurado");
        assert!(open_with("/nao/existe/prog", &f).is_err());
    }
}
