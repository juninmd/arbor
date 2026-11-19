//! "Rede" do Explorer: os computadores visíveis na rede local. No Windows vem do serviço Navegador
//! (`NetServerEnum`); no Linux, dos anúncios SMB via Avahi (`avahi-browse`), quando instalado.

use arbor_core::Entry;
use std::path::{Path, PathBuf};

pub const TITLE: &str = "Rede";
const SENTINEL: &str = "arbor:network";

pub fn path() -> PathBuf {
    PathBuf::from(SENTINEL)
}

pub fn is(p: &Path) -> bool {
    p.as_os_str() == SENTINEL
}

/// Uma pasta `\\nome` por computador; abrir lista os compartilhamentos.
pub fn computer_entries(names: &[String]) -> Vec<Entry> {
    names
        .iter()
        .map(|name| {
            let mut e = Entry::synthetic(name, true, 0);
            e.path = PathBuf::from(format!(r"\\{name}"));
            e
        })
        .collect()
}

/// Nomes únicos, sem diferenciar maiúsculas, em ordem alfabética.
fn dedup(mut names: Vec<String>) -> Vec<String> {
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

/// Linhas resolvidas (`=`) de `avahi-browse -tpr _smb._tcp`: o 7º campo é o host ("nas.local").
#[cfg(any(not(windows), test))]
fn parse_avahi(out: &str) -> Vec<String> {
    let hosts = out.lines().filter(|l| l.starts_with('=')).filter_map(|l| l.split(';').nth(6)).filter(|h| !h.is_empty());
    dedup(hosts.map(str::to_string).collect())
}

#[cfg(not(windows))]
pub fn computers() -> Result<Vec<String>, String> {
    let out = std::process::Command::new("avahi-browse")
        .args(["-tpr", "_smb._tcp"])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|_| "Para ver os computadores da rede, instale o avahi-utils (avahi-browse).".to_string())?;
    Ok(parse_avahi(&String::from_utf8_lossy(&out.stdout)))
}

#[cfg(windows)]
pub fn computers() -> Result<Vec<String>, String> {
    use windows_sys::Win32::NetworkManagement::NetManagement::{NetApiBufferFree, NetServerEnum, SERVER_INFO_100, SV_TYPE_ALL};
    let (mut buf, mut read, mut total) = (std::ptr::null_mut::<u8>(), 0u32, 0u32);
    // SAFETY: servidor e domínio nulos = máquina local e domínio atual; a API aloca `buf`, liberado abaixo.
    let status = unsafe {
        NetServerEnum(std::ptr::null(), 100, &mut buf, u32::MAX, &mut read, &mut total, SV_TYPE_ALL, std::ptr::null(), std::ptr::null_mut())
    };
    // 6118 = ERROR_NO_BROWSER_SERVERS_FOUND: rede sem o serviço Navegador, comum no Windows 10+.
    if status == 6118 {
        return Ok(vec![]);
    }
    if status != 0 {
        return Err(format!("A rede não respondeu (erro {status})."));
    }
    // SAFETY: com nível 100 a API devolve `read` estruturas SERVER_INFO_100 contíguas.
    let items = unsafe { std::slice::from_raw_parts(buf as *const SERVER_INFO_100, read as usize) };
    // SAFETY: nomes são strings terminadas em zero dentro do buffer ainda vivo.
    let names = items.iter().map(|s| unsafe { crate::network::wide_str(s.sv100_name) }).filter(|n| !n.is_empty()).collect();
    // SAFETY: buffer alocado pela NetServerEnum acima.
    unsafe { NetApiBufferFree(buf.cast()) };
    Ok(dedup(names))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avahi_lists_each_resolved_host_once() {
        let out = "+;eth0;IPv4;NAS;Microsoft Windows Network;local\n\
                   =;eth0;IPv4;NAS;Microsoft Windows Network;local;nas.local;192.168.0.2;445;\n\
                   =;eth0;IPv6;NAS;Microsoft Windows Network;local;nas.local;fe80::1;445;\n\
                   =;eth0;IPv4;PC;Microsoft Windows Network;local;Pc.local;192.168.0.3;445;\n";
        assert_eq!(parse_avahi(out), ["nas.local", "Pc.local"]);
        assert!(parse_avahi("").is_empty());
    }

    #[test]
    fn computers_open_as_servers() {
        let e = computer_entries(&["NAS".into()]);
        assert!(e[0].is_dir);
        assert_eq!(e[0].path, PathBuf::from(r"\\NAS"));
        assert_eq!(crate::network::server_root(&e[0].path), Some("NAS".into()));
        assert!(is(&path()) && !is(Path::new("/rede")));
    }
}
