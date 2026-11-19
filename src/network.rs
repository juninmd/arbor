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
unsafe fn wide_str(p: *const u16) -> String {
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

#[cfg(not(windows))]
pub fn shares(server: &str) -> Result<Vec<String>, String> {
    Err(format!("Listar os compartilhamentos de {server} só funciona no Windows; monte com o gerenciador do sistema."))
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
}
