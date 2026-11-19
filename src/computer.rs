//! "Este Computador": a pasta virtual onde o arbor abre, como no Explorer do Windows.
//! Mostra as pastas do usuário e os discos/volumes montados, com espaço livre e total.

use arbor_core::entry::natural_key;
use arbor_core::{Entry, Ext};
use std::path::{Path, PathBuf};

pub const TITLE: &str = "Este Computador";

/// Caminho-sentinela: nunca colide com uma pasta real porque o arbor só navega por caminhos absolutos.
const SENTINEL: &str = "arbor:computer";

pub fn path() -> PathBuf {
    PathBuf::from(SENTINEL)
}

pub fn is(p: &Path) -> bool {
    p.as_os_str() == SENTINEL
}

#[derive(Debug, Clone)]
pub struct Drive {
    pub label: String,
    pub path: PathBuf,
    /// Bytes livres e totais; `None` se o volume não respondeu.
    pub space: Option<(u64, u64)>,
}

pub fn places() -> Vec<(String, PathBuf)> {
    let candidates = [
        ("Início", dirs::home_dir()),
        ("Área de trabalho", dirs::desktop_dir()),
        ("Documentos", dirs::document_dir()),
        ("Downloads", dirs::download_dir()),
        ("Imagens", dirs::picture_dir()),
        ("Músicas", dirs::audio_dir()),
        ("Vídeos", dirs::video_dir()),
    ];
    candidates.into_iter().filter_map(|(n, p)| p.map(|p| (n.to_string(), p))).collect()
}

/// Discos e volumes. Roda fora da interface: um drive de rede desconectado pode demorar a responder.
pub fn drives() -> Vec<Drive> {
    let mut v: Vec<(String, PathBuf)> = Vec::new();
    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", letter as char));
        if root.exists() {
            v.push((format!("Disco ({}:)", letter as char), root));
        }
    }
    #[cfg(not(windows))]
    {
        v.push(("Sistema (/)".to_string(), PathBuf::from("/")));
        let mut roots = vec![PathBuf::from("/Volumes")];
        if let Ok(user) = std::env::var("USER") {
            roots.push(PathBuf::from("/media").join(&user));
            roots.push(PathBuf::from("/run/media").join(&user));
        }
        roots.push(PathBuf::from("/mnt"));
        for root in roots {
            if let Ok(rd) = std::fs::read_dir(&root) {
                for e in rd.flatten().filter(|e| e.path().is_dir()) {
                    if e.path() == Path::new("/Volumes/Macintosh HD") {
                        continue;
                    }
                    v.push((e.file_name().to_string_lossy().into_owned(), e.path()));
                }
            }
        }
    }
    v.into_iter().map(|(label, path)| Drive { space: space(&path), label, path }).collect()
}

/// Pastas do usuário primeiro, depois os discos, na ordem em que o Explorer agrupa.
pub fn entries(places: &[(String, PathBuf)], drives: &[Drive]) -> Vec<Entry> {
    let folders = places.iter().filter(|(_, p)| p.is_dir()).map(|(n, p)| entry(n, p, 0));
    let disks = drives.iter().map(|d| entry(&d.label, &d.path, d.space.map_or(0, |(_, total)| total)));
    folders.chain(disks).collect()
}

fn entry(name: &str, path: &Path, size: u64) -> Entry {
    Entry {
        name: name.to_string(),
        path: path.to_path_buf(),
        is_dir: true,
        is_link: false,
        hidden: false,
        size,
        modified: None,
        ext: Ext::default(),
        key: natural_key(name),
    }
}

#[cfg(unix)]
fn space(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` é uma string C válida e `st` é um buffer do tamanho certo que a chamada preenche.
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let block = st.f_frsize as u64;
    Some((st.f_bavail as u64 * block, st.f_blocks as u64 * block))
}

#[cfg(windows)]
fn space(path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: `wide` termina em zero; os ponteiros de saída apontam para u64 vivos.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) };
    (ok != 0).then_some((free, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinela_nao_e_caminho_real() {
        assert!(is(&path()));
        assert!(path().is_relative(), "toda pasta real do arbor é absoluta");
        assert!(!is(Path::new("/")));
    }

    #[test]
    fn pastas_antes_dos_discos_e_pastas_inexistentes_somem() {
        let tmp = std::env::temp_dir();
        let places = vec![("Temp".to_string(), tmp.clone()), ("Sumiu".to_string(), tmp.join("nao-existe-arbor"))];
        let drives = vec![Drive { label: "Disco".into(), path: PathBuf::from("/"), space: Some((1, 10)) }];
        let e = entries(&places, &drives);
        let names: Vec<_> = e.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Temp", "Disco"]);
        assert!(e.iter().all(|e| e.is_dir));
        assert_eq!(e[1].size, 10);
    }

    #[test]
    fn raiz_tem_espaco() {
        let root = if cfg!(windows) { "C:\\" } else { "/" };
        let (free, total) = space(Path::new(root)).expect("espaço da raiz");
        assert!(total > 0 && free <= total);
    }
}
