//! Layouts nomeados (Directory Opus "Layouts", workspaces do Favnyr): o conjunto de abas e painéis salvo com
//! um nome e reaberto depois. Cada layout é um `layouts/<nome>.json` na pasta de configuração, no mesmo
//! formato da sessão.

use crate::session::Session;
use std::path::{Path, PathBuf};

fn dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor").join("layouts"))
}

/// Nome vira nome de arquivo: nada de barras, `..` ou caracteres que o Windows recusa.
pub fn valid(name: &str) -> Result<&str, String> {
    let name = name.trim();
    let bad = name.is_empty()
        || name.len() > 60
        || name.starts_with('.')
        || name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'));
    if bad {
        Err("Nome de layout inválido (sem / \\ : * ? \" < > |)".into())
    } else {
        Ok(name)
    }
}

pub fn save(name: &str, session: &Session) -> Result<(), String> {
    let name = valid(name)?;
    let dir = dir().ok_or("sem pasta de configuração")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{name}.json")), session.to_json()).map_err(|e| e.to_string())
}

pub fn load(name: &str) -> Result<Session, String> {
    let name = valid(name)?;
    let file = dir().ok_or("sem pasta de configuração")?.join(format!("{name}.json"));
    let text = std::fs::read_to_string(&file).map_err(|_| format!("Layout \"{name}\" não existe. Salvos: {}", names().join(", ")))?;
    Session::from_json(&text, Path::is_dir).ok_or_else(|| format!("As pastas do layout \"{name}\" não existem mais"))
}

/// Layouts salvos, do mais recente para o mais antigo.
pub fn names() -> Vec<String> {
    let Some(read) = dir().and_then(|d| std::fs::read_dir(d).ok()) else { return vec![] };
    let mut found: Vec<(std::time::SystemTime, String)> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().strip_suffix(".json")?.to_string();
            Some((e.metadata().and_then(|m| m.modified()).ok()?, name))
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found.into_iter().map(|(_, n)| n).collect()
}

#[cfg(test)]
mod tests {
    use super::valid;

    #[test]
    fn nome_de_layout_nao_escapa_da_pasta() {
        assert_eq!(valid("  Trabalho 2 ").unwrap(), "Trabalho 2");
        for bad in ["", "../x", "a/b", "a\\\\b", ".oculto", "c:", "x?"] {
            assert!(valid(bad).is_err(), "{bad}");
        }
    }
}
