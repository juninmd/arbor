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

/// Nome mostrado no lugar do caminho para os locais virtuais.
pub fn virtual_title(p: &Path) -> Option<&'static str> {
    if is(p) {
        Some(TITLE)
    } else if crate::trashbin::is(p) {
        Some(crate::trashbin::TITLE)
    } else if crate::home::is(p) {
        Some(crate::home::TITLE)
    } else {
        None
    }
}

/// Locais que não são pastas de verdade ("Este Computador" e "Lixeira"): sem escrever, colar ou soltar.
pub fn is_virtual(p: &Path) -> bool {
    is(p) || crate::trashbin::is(p) || crate::home::is(p) || crate::network::server_root(p).is_some()
}

#[derive(Debug, Clone)]
pub struct Drive {
    pub label: String,
    pub path: PathBuf,
    /// Bytes livres e totais; `None` se o volume não respondeu.
    pub space: Option<(u64, u64)>,
    pub media: Media,
}

/// Tipo de unidade, para o ícone como no Explorer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Media {
    /// Onde o sistema está instalado.
    System,
    Fixed,
    /// Pendrive, cartão de memória.
    Usb,
    Optical,
    Network,
}

impl Drive {
    /// Pendrive, cartão, DVD: mostra "Ejetar".
    pub fn removable(&self) -> bool {
        matches!(self.media, Media::Usb | Media::Optical)
    }
}

/// Tipo pelo sistema de arquivos montado (Linux, `/proc/mounts`); `None` deixa a regra pela pasta decidir.
#[cfg(any(not(windows), test))]
pub fn media_of_fstype(fs: &str) -> Option<Media> {
    match fs {
        "nfs" | "nfs4" | "cifs" | "smb3" | "smbfs" | "fuse.sshfs" | "9p" | "afpfs" => Some(Media::Network),
        "iso9660" | "udf" => Some(Media::Optical),
        _ => None,
    }
}

pub fn places() -> Vec<(String, PathBuf)> {
    let candidates = [
        ("Pasta pessoal", dirs::home_dir()),
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
    let mut v: Vec<(String, PathBuf, Media)> = Vec::new();
    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        if let Some((label, media)) = win::volume(letter as char) {
            v.push((label, PathBuf::from(format!("{}:\\", letter as char)), media));
        }
    }
    #[cfg(not(windows))]
    {
        // Montagens de rede e de disco óptico pelo tipo do sistema de arquivos; o resto pela pasta.
        let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
        let by_fs = |p: &Path| {
            mounts.lines().find_map(|l| {
                let mut f = l.split_whitespace();
                let (_, at, fs) = (f.next()?, f.next()?, f.next()?);
                (Path::new(at) == p).then(|| media_of_fstype(fs)).flatten()
            })
        };
        v.push(("Sistema (/)".to_string(), PathBuf::from("/"), Media::System));
        let mut roots = vec![PathBuf::from("/Volumes")];
        if let Ok(user) = std::env::var("USER") {
            roots.push(PathBuf::from("/media").join(&user));
            roots.push(PathBuf::from("/run/media").join(&user));
        }
        roots.push(PathBuf::from("/mnt"));
        for root in roots {
            // /mnt costuma ter montagens fixas; /media, /run/media e /Volumes são de mídia removível.
            let media = if root.starts_with("/mnt") { Media::Fixed } else { Media::Usb };
            if let Ok(rd) = std::fs::read_dir(&root) {
                for e in rd.flatten().filter(|e| e.path().is_dir()) {
                    if e.path() == Path::new("/Volumes/Macintosh HD") {
                        continue;
                    }
                    let media = by_fs(&e.path()).unwrap_or(media);
                    v.push((e.file_name().to_string_lossy().into_owned(), e.path(), media));
                }
            }
        }
        // smb://, sftp:// abertos pelo gerenciador do sistema (gio) ficam montados pelo GVFS aqui.
        // SAFETY: getuid não falha nem toca memória.
        #[cfg(target_os = "linux")]
        if let Ok(rd) = std::fs::read_dir(PathBuf::from(format!("/run/user/{}/gvfs", unsafe { libc::getuid() }))) {
            for e in rd.flatten() {
                v.push((crate::network::gvfs_label(&e.file_name().to_string_lossy()), e.path(), Media::Network));
            }
        }
    }
    v.into_iter().map(|(label, path, media)| Drive { space: space(&path), label, path, media }).collect()
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
mod win {
    use super::Media;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumeInformationW};

    /// Nome como no Explorer ("Backup (D:)", "Disco Local (C:)") e tipo; `None` se a letra não existe.
    pub fn volume(letter: char) -> Option<(String, Media)> {
        let root: Vec<u16> = std::ffi::OsStr::new(&format!("{letter}:\\")).encode_wide().chain(Some(0)).collect();
        // SAFETY: `root` termina em zero.
        let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
        // 0 = desconhecido, 1 = sem raiz (letra livre).
        if kind <= 1 {
            return None;
        }
        let mut name = [0u16; 261];
        // SAFETY: buffer de 261 u16 com o tamanho informado; os demais ponteiros podem ser nulos.
        let ok = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                name.as_mut_ptr(),
                name.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        };
        let label = if ok != 0 { String::from_utf16_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(0)]) } else { String::new() };
        let fallback = match kind {
            2 => "Unidade USB",
            4 => "Unidade de rede",
            5 => "Unidade de DVD",
            _ => "Disco Local",
        };
        let label = if label.trim().is_empty() { fallback.to_string() } else { label };
        let system = std::env::var("SystemDrive").is_ok_and(|d| d.eq_ignore_ascii_case(&format!("{letter}:")));
        let media = match kind {
            2 => Media::Usb,
            4 => Media::Network,
            5 => Media::Optical,
            _ if system => Media::System,
            _ => Media::Fixed,
        };
        Some((format!("{label} ({letter}:)"), media))
    }
}

/// Abre o diálogo "Mapear unidade de rede" do próprio Windows; a nova letra aparece ao atualizar (F5).
pub fn map_drive_dialog() -> Result<(), String> {
    #[cfg(windows)]
    return std::process::Command::new("rundll32.exe")
        .args(["shell32.dll,SHHelpShortcuts_RunDLL", "Connect"])
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string());
    #[cfg(not(windows))]
    Err("só existe no Windows".into())
}

/// Ejeta (desmonta) uma mídia removível pelo utilitário do sistema, sem passar por shell.
pub fn eject(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let out = {
        // Só uma letra de unidade válida entra no script: nada do usuário vira código.
        let s = path.to_string_lossy();
        let letter = s.chars().next().filter(|c| c.is_ascii_alphabetic() && s.len() <= 3 && s[1..].starts_with(':'));
        let letter = letter.ok_or("caminho de unidade inválido")?;
        let script = format!("(New-Object -ComObject Shell.Application).Namespace(17).ParseName('{letter}:').InvokeVerb('Eject')");
        std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).output()
    };
    #[cfg(target_os = "macos")]
    let out = std::process::Command::new("diskutil").arg("eject").arg(path).output();
    #[cfg(all(unix, not(target_os = "macos")))]
    let out = std::process::Command::new("gio").args(["mount", "-e"]).arg(path).output().and_then(|o| {
        if o.status.success() {
            Ok(o)
        } else {
            std::process::Command::new("umount").arg(path).output()
        }
    });
    let out = out.map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
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
        let drives = vec![Drive { label: "Disco".into(), path: PathBuf::from("/"), space: Some((1, 10)), media: Media::System }];
        let e = entries(&places, &drives);
        let names: Vec<_> = e.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Temp", "Disco"]);
        assert!(e.iter().all(|e| e.is_dir));
        assert_eq!(e[1].size, 10);
    }

    #[test]
    fn network_and_optical_mounts_are_recognized_and_only_removable_media_ejects() {
        assert_eq!(media_of_fstype("cifs"), Some(Media::Network));
        assert_eq!(media_of_fstype("fuse.sshfs"), Some(Media::Network));
        assert_eq!(media_of_fstype("iso9660"), Some(Media::Optical));
        assert_eq!(media_of_fstype("ext4"), None);
        let d = |media| Drive { label: String::new(), path: PathBuf::new(), space: None, media };
        assert!(d(Media::Usb).removable() && d(Media::Optical).removable());
        assert!(!d(Media::Network).removable() && !d(Media::System).removable() && !d(Media::Fixed).removable());
    }

    #[test]
    fn raiz_tem_espaco() {
        let root = if cfg!(windows) { "C:\\" } else { "/" };
        let (free, total) = space(Path::new(root)).expect("espaço da raiz");
        assert!(total > 0 && free <= total);
    }
}
