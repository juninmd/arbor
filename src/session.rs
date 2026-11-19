//! Sessão: as abas de cada painel salvas em disco para reabrir como estavam, e a opção que liga isso.
//!
//! Por padrão o arbor abre do zero (como o Explorer). Com "Restaurar abas ao abrir" ligado na paleta, ele
//! volta com as mesmas abas, painéis e visões. O arquivo é pequeno (`session.json` na pasta de configuração)
//! e só é regravado quando algo nele muda, numa thread à parte.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Session {
    pub dual: bool,
    pub active: usize,
    /// Para cada painel: aba ativa e abas (caminho, grade).
    pub panes: [(usize, Vec<(PathBuf, bool)>); 2],
}

fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor"))
}

fn session_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("session.json"))
}

fn settings_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("settings"))
}

/// Desligado por padrão: o arbor abre na pasta inicial, como o Explorer.
pub fn restore_enabled() -> bool {
    let text = settings_file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    text.lines().any(|l| l.trim() == "restore_session=true")
}

pub fn set_restore_enabled(on: bool) -> Result<(), String> {
    let file = settings_file().ok_or("sem pasta de configuração")?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().filter(|l| !l.trim().starts_with("restore_session=")).map(str::to_string).collect();
    lines.push(format!("restore_session={on}"));
    std::fs::write(&file, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

impl Session {
    pub fn to_json(&self) -> String {
        let panes: Vec<Value> = self
            .panes
            .iter()
            .map(|(active, tabs)| {
                let tabs: Vec<Value> = tabs.iter().map(|(p, grid)| json!({ "path": p.to_string_lossy(), "grid": grid })).collect();
                json!({ "active": active, "tabs": tabs })
            })
            .collect();
        json!({ "dual": self.dual, "active": self.active, "panes": panes }).to_string()
    }

    /// Lê a sessão descartando abas cujas pastas não existem mais. `None` se não sobrar nenhuma aba num painel.
    pub fn from_json(text: &str, exists: impl Fn(&Path) -> bool) -> Option<Session> {
        let v: Value = serde_json::from_str(text).ok()?;
        let mut session = Session { dual: v["dual"].as_bool()?, active: v["active"].as_u64()?.min(1) as usize, ..Default::default() };
        let panes = v["panes"].as_array()?;
        for (i, pane) in panes.iter().take(2).enumerate() {
            let tabs: Vec<(PathBuf, bool)> = pane["tabs"]
                .as_array()?
                .iter()
                .filter_map(|t| Some((PathBuf::from(t["path"].as_str()?), t["grid"].as_bool().unwrap_or(false))))
                .filter(|(p, _)| exists(p))
                .collect();
            if tabs.is_empty() {
                return None;
            }
            let active = (pane["active"].as_u64().unwrap_or(0) as usize).min(tabs.len() - 1);
            session.panes[i] = (active, tabs);
        }
        (panes.len() >= 2).then_some(session)
    }

    pub fn load() -> Option<Session> {
        let text = std::fs::read_to_string(session_file()?).ok()?;
        Session::from_json(&text, Path::is_dir)
    }

    /// Grava numa thread à parte: nunca segura a interface.
    pub fn save(&self) {
        let Some(file) = session_file() else { return };
        let text = self.to_json();
        std::thread::spawn(move || {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = file.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &file);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta_e_descarta_pastas_sumidas() {
        let s = Session {
            dual: true,
            active: 1,
            panes: [(1, vec![(PathBuf::from("/a"), false), (PathBuf::from("/b"), true)]), (0, vec![(PathBuf::from("/c"), false)])],
        };
        let text = s.to_json();
        assert_eq!(Session::from_json(&text, |_| true), Some(s));
        let without_b = Session::from_json(&text, |p| p != Path::new("/b")).unwrap();
        assert_eq!(without_b.panes[0], (0, vec![(PathBuf::from("/a"), false)]));
        // Um painel sem nenhuma pasta válida invalida a sessão.
        assert_eq!(Session::from_json(&text, |p| p != Path::new("/c")), None);
        assert_eq!(Session::from_json("lixo", |_| true), None);
    }
}
