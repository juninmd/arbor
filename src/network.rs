//! Locais de rede como no Explorer: `\\servidor` lista os compartilhamentos (Windows) e as montagens de
//! rede do GVFS (`smb://` aberto pelo Nautilus/gio) aparecem como unidades de rede no Linux.

use arbor_core::Entry;
use std::path::{Path, PathBuf};

/// `\\servidor` ou `\\servidor\` sem compartilhamento: o nível que o Explorer mostra como lista de pastas
/// compartilhadas. Aceita `/` também, que o Windows trata igual.
pub fn server_root(path: &Path) -> Option<String> {
    let s = path.to_str()?;
    let rest = s.strip_prefix(r"\\").or_else(|| s.strip_prefix("//"))?;
    let rest = rest.trim_end_matches(['\\', '/']);
    // "?" e "." são os prefixos de caminho do Windows (\\?\C:\, \\.\pipe), não servidores.
    let valid = !rest.is_empty() && !rest.contains(['\\', '/']) && rest != "?" && rest != ".";
    valid.then(|| rest.to_string())
}

/// Pasta acima de `\\servidor\compartilhamento`: o próprio servidor, como no Explorer. O `Path` do Windows
/// trata o compartilhamento como raiz e não tem pai.
pub fn share_parent(path: &Path) -> Option<PathBuf> {
    let s = path.to_str()?;
    let rest = s.strip_prefix(r"\\").or_else(|| s.strip_prefix("//"))?.trim_end_matches(['\\', '/']);
    let (server, share) = rest.split_once(['\\', '/'])?;
    let valid = !server.is_empty() && !share.is_empty() && !share.contains(['\\', '/']) && server != "?" && server != ".";
    valid.then(|| PathBuf::from(format!(r"\\{server}")))
}

/// Entradas de pasta para cada compartilhamento de `server`.
pub fn share_entries(server: &str, shares: &[String]) -> Vec<Entry> {
    shares
        .iter()
        .map(|name| {
            let path = PathBuf::from(format!(r"\\{server}\{name}"));
            let mut e = Entry::synthetic(name, true, 0);
            e.path = path;
            e
        })
        .collect()
}

/// Compartilhamentos de disco visíveis de `server`; os administrativos (`C$`, `ADMIN$`) ficam de fora.
#[cfg(windows)]
pub fn shares(server: &str) -> Result<Vec<String>, String> {
    use windows_sys::Win32::NetworkManagement::NetManagement::NetApiBufferFree;
    use windows_sys::Win32::Storage::FileSystem::{NetShareEnum, SHARE_INFO_1, STYPE_DISKTREE, STYPE_MASK};
    const MAX_PREFERRED_LENGTH: u32 = u32::MAX;
    let name: Vec<u16> = format!(r"\\{server}").encode_utf16().chain(Some(0)).collect();
    let (mut buf, mut read, mut total) = (std::ptr::null_mut::<u8>(), 0u32, 0u32);
    // SAFETY: `name` termina em zero; a API aloca `buf`, liberado abaixo com NetApiBufferFree.
    let status = unsafe { NetShareEnum(name.as_ptr(), 1, &mut buf, MAX_PREFERRED_LENGTH, &mut read, &mut total, std::ptr::null_mut()) };
    if status != 0 {
        return Err(format!("O servidor {server} não respondeu (erro {status})."));
    }
    // SAFETY: com nível 1 a API devolve `read` estruturas SHARE_INFO_1 contíguas.
    let items = unsafe { std::slice::from_raw_parts(buf as *const SHARE_INFO_1, read as usize) };
    let mut out: Vec<String> = items
        .iter()
        .filter(|s| s.shi1_type & STYPE_MASK == STYPE_DISKTREE)
        // SAFETY: nomes são strings terminadas em zero dentro do buffer ainda vivo.
        .map(|s| unsafe { wide_str(s.shi1_netname) })
        .filter(|n| !n.ends_with('$'))
        .collect();
    // SAFETY: buffer alocado pela NetShareEnum acima.
    unsafe { NetApiBufferFree(buf.cast()) };
    out.sort_by_key(|s| s.to_lowercase());
    Ok(out)
}

#[cfg(windows)]
pub(crate) unsafe fn wide_str(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    // SAFETY: o chamador garante uma string terminada em zero.
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}

/// Sem a API do Windows, pergunta ao `smbclient` (Samba) como convidado.
#[cfg(not(windows))]
pub fn shares(server: &str) -> Result<Vec<String>, String> {
    let out = std::process::Command::new("smbclient")
        .args(["-g", "-N", "-L", &format!("//{server}")])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|_| format!("Para listar os compartilhamentos de {server}, instale o smbclient."))?;
    let names = parse_smbclient(&String::from_utf8_lossy(&out.stdout));
    if names.is_empty() && !out.status.success() {
        return Err(format!("O servidor {server} não respondeu ou pediu senha; monte com o gerenciador do sistema."));
    }
    Ok(names)
}

/// Linhas `Disk|nome|comentário` de `smbclient -g -L`; os administrativos (`C$`) ficam de fora.
#[cfg(any(not(windows), test))]
fn parse_smbclient(out: &str) -> Vec<String> {
    let mut names: Vec<String> = out
        .lines()
        .filter_map(|l| l.strip_prefix("Disk|")?.split('|').next())
        .filter(|n| !n.is_empty() && !n.ends_with('$'))
        .map(str::to_string)
        .collect();
    names.sort_by_key(|s| s.to_lowercase());
    names
}

#[cfg(any(target_os = "linux", test))]
/// `\\servidor\compartilhamento` vira (servidor, compartilhamento).
pub fn split_share(path: &Path) -> Option<(String, String)> {
    let server = share_parent(path)?;
    let share = path.to_str()?.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next()?.to_string();
    Some((server_root(&server)?, share))
}

/// No Linux um compartilhamento não é caminho do sistema: monta pelo GVFS (`gio mount`) e devolve a pasta.
#[cfg(target_os = "linux")]
pub fn mount_share(server: &str, share: &str) -> Result<PathBuf, String> {
    // SAFETY: getuid não tem pré-condições.
    let dir = PathBuf::from(format!(
        "/run/user/{}/gvfs/smb-share:server={},share={}",
        unsafe { libc::getuid() },
        server.to_lowercase(),
        share.to_lowercase()
    ));
    if dir.is_dir() {
        return Ok(dir);
    }
    let out = std::process::Command::new("gio")
        .args(["mount", &format!("smb://{server}/{share}")])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|_| "Para abrir compartilhamentos, instale o gvfs (gio).".to_string())?;
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(format!("Não montou \\\\{server}\\{share}: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Nome legível de uma montagem do GVFS: "smb-share:server=nas,share=fotos" vira "fotos em nas".
#[cfg(any(target_os = "linux", test))]
pub fn gvfs_label(dir_name: &str) -> String {
    let Some((scheme, args)) = dir_name.split_once(':') else { return dir_name.to_string() };
    let field = |k: &str| args.split(',').find_map(|kv| kv.strip_prefix(k)?.strip_prefix('=')).map(str::to_string);
    match (field("share"), field("host").or_else(|| field("server"))) {
        (Some(share), Some(host)) => format!("{share} em {host}"),
        (None, Some(host)) => format!("{host} ({})", scheme.trim_end_matches("-share")),
        _ => dir_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bare_server_paths_list_shares() {
        assert_eq!(server_root(Path::new(r"\\nas")), Some("nas".into()));
        assert_eq!(server_root(Path::new(r"\\nas\")), Some("nas".into()));
        assert_eq!(server_root(Path::new("//nas")), Some("nas".into()));
        assert_eq!(server_root(Path::new(r"\\nas\fotos")), None, "compartilhamento já é pasta comum");
        assert_eq!(server_root(Path::new(r"\\?\C:\")), None);
        assert_eq!(server_root(Path::new(r"\\.\pipe")), None);
        assert_eq!(server_root(Path::new(r"\\")), None);
        assert_eq!(server_root(Path::new("/home/nas")), None);
    }

    #[test]
    fn up_from_a_share_goes_to_its_server() {
        assert_eq!(share_parent(Path::new(r"\\nas\fotos")), Some(PathBuf::from(r"\\nas")));
        assert_eq!(share_parent(Path::new(r"\\nas\fotos\")), Some(PathBuf::from(r"\\nas")));
        assert_eq!(share_parent(Path::new(r"\\nas\fotos\2024")), None, "subpasta usa o pai comum");
        assert_eq!(share_parent(Path::new(r"\\?\C:")), None);
        assert_eq!(share_parent(Path::new(r"\\nas")), None);
    }

    #[test]
    fn shares_become_folders_under_the_server() {
        let e = share_entries("nas", &["fotos".into()]);
        assert_eq!(e[0].name, "fotos");
        assert!(e[0].is_dir);
        assert_eq!(e[0].path, PathBuf::from(r"\\nas\fotos"));
    }

    #[test]
    fn gvfs_mount_names_read_like_explorer() {
        assert_eq!(gvfs_label("smb-share:server=nas.local,share=fotos"), "fotos em nas.local");
        assert_eq!(gvfs_label("sftp:host=pi"), "pi (sftp)");
        assert_eq!(gvfs_label("estranho"), "estranho");
    }

    #[test]
    fn smbclient_lists_disk_shares_without_admin_ones() {
        let out = "Disk|fotos|Fotos da casa\nIPC|IPC$|IPC Service\nDisk|C$|Default share\nDisk|Backup|\n";
        assert_eq!(parse_smbclient(out), ["Backup", "fotos"]);
    }

    #[test]
    fn share_paths_split_into_server_and_share() {
        assert_eq!(split_share(Path::new(r"\\nas\fotos")), Some(("nas".into(), "fotos".into())));
        assert_eq!(split_share(Path::new(r"\\nas")), None);
        assert_eq!(split_share(Path::new(r"\\nas\fotos\2024")), None);
    }
}
