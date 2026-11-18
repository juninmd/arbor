//! Leitura do JSON de release do GitHub e do `SHA256SUMS.txt`; nada de I/O aqui.

use serde_json::Value;

/// Uma versão nova pronta para oferecer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    pub notes: String,
    pub page: String,
    pub asset_name: String,
    pub asset_url: String,
    pub sums_url: String,
}

pub const SUMS_ASSET: &str = "SHA256SUMS.txt";

/// Nome do binário que o workflow de release publica para esta plataforma, por exemplo
/// `arbor-0.4.0-linux-x86_64` ou `arbor-0.4.0-windows-x86_64.exe`.
pub fn asset_name(version: &str, os: &str, arch: &str) -> String {
    let ext = if os == "windows" { ".exe" } else { "" };
    format!("arbor-{version}-{os}-{arch}{ext}")
}

pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.trim().trim_start_matches('v').split('.');
    let mut next = || parts.next()?.parse::<u64>().ok();
    let v = (next()?, next()?, next()?);
    parts.next().is_none().then_some(v)
}

pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (parse_version(current), parse_version(candidate)) {
        (Some(c), Some(n)) => n > c,
        _ => false,
    }
}

fn download_url(release: &Value, name: &str, prefix: &str) -> Option<String> {
    release["assets"]
        .as_array()?
        .iter()
        .find(|a| a["name"].as_str() == Some(name))
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|u| u.starts_with(prefix)) // nunca segue um link fora das releases deste repositório
        .map(str::to_string)
}

/// A release estável mais nova, se for mais nova que `current` e trouxer o binário desta plataforma e as somas.
pub fn pick(release: &Value, current: &str, prefix: &str, os: &str, arch: &str) -> Option<Update> {
    if release["draft"].as_bool() == Some(true) || release["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = release["tag_name"].as_str()?;
    if !is_newer(current, tag) {
        return None;
    }
    let version = tag.trim_start_matches('v').to_string();
    let asset_name = asset_name(&version, os, arch);
    Some(Update {
        notes: release["body"].as_str().unwrap_or_default().trim().to_string(),
        page: release["html_url"].as_str().unwrap_or_default().to_string(),
        asset_url: download_url(release, &asset_name, prefix)?,
        sums_url: download_url(release, SUMS_ASSET, prefix)?,
        asset_name,
        version,
    })
}

/// Procura `file` na saída do `sha256sum` (`<hex>  nome` ou `<hex> *nome`).
pub fn expected_hash(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == file && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PREFIX: &str = "https://github.com/juninmd/arbor/releases/download/";

    fn release(tag: &str, assets: &[&str]) -> Value {
        json!({
            "tag_name": tag,
            "html_url": format!("https://github.com/juninmd/arbor/releases/tag/{tag}"),
            "body": "## Features\n- algo",
            "assets": assets.iter().map(|n| json!({"name": n, "browser_download_url": format!("{PREFIX}{tag}/{n}")})).collect::<Vec<_>>(),
        })
    }

    #[test]
    fn versoes() {
        assert!(is_newer("0.1.0", "v0.2.0"));
        assert!(is_newer("0.9.9", "1.0.0"));
        assert!(!is_newer("0.2.0", "v0.2.0"));
        assert!(!is_newer("0.2.0", "v0.1.9"));
        assert!(!is_newer("0.2.0", "lixo"));
        assert_eq!(parse_version("v1.2.3.4"), None);
    }

    #[test]
    fn escolhe_o_binario_da_plataforma() {
        let r = release("v0.3.0", &["arbor-0.3.0-linux-x86_64", "arbor-0.3.0-windows-x86_64.exe", SUMS_ASSET]);
        let u = pick(&r, "0.2.0", PREFIX, "windows", "x86_64").unwrap();
        assert_eq!(u.version, "0.3.0");
        assert_eq!(u.asset_name, "arbor-0.3.0-windows-x86_64.exe");
        assert!(u.asset_url.ends_with("/v0.3.0/arbor-0.3.0-windows-x86_64.exe"));
        assert!(pick(&r, "0.2.0", PREFIX, "macos", "aarch64").is_none(), "sem binário para a plataforma");
        assert!(pick(&r, "0.3.0", PREFIX, "linux", "x86_64").is_none(), "já atualizado");
    }

    #[test]
    fn recusa_link_de_fora_rascunho_e_sem_somas() {
        let mut r = release("v0.3.0", &["arbor-0.3.0-linux-x86_64", SUMS_ASSET]);
        r["assets"][0]["browser_download_url"] = json!("https://malicioso.example/arbor");
        assert!(pick(&r, "0.2.0", PREFIX, "linux", "x86_64").is_none());
        let r = release("v0.3.0", &["arbor-0.3.0-linux-x86_64"]);
        assert!(pick(&r, "0.2.0", PREFIX, "linux", "x86_64").is_none());
        let mut r = release("v0.3.0", &["arbor-0.3.0-linux-x86_64", SUMS_ASSET]);
        r["prerelease"] = json!(true);
        assert!(pick(&r, "0.2.0", PREFIX, "linux", "x86_64").is_none());
    }

    #[test]
    fn le_somas() {
        let h = "a".repeat(64);
        let sums = format!("{h}  arbor-0.3.0-linux-x86_64\n{}  *arbor-0.3.0-windows-x86_64.exe\n", "B".repeat(64));
        assert_eq!(expected_hash(&sums, "arbor-0.3.0-linux-x86_64"), Some(h));
        assert_eq!(expected_hash(&sums, "arbor-0.3.0-windows-x86_64.exe"), Some("b".repeat(64)));
        assert_eq!(expected_hash(&sums, "outro"), None);
        assert_eq!(expected_hash("curto  arbor", "arbor"), None);
    }
}
