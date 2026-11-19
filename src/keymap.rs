//! Atalhos configuráveis: um arquivo de texto `keys` na pasta de configuração, uma linha por atalho:
//!
//! ```text
//! Ctrl+Shift+N = NewFolder
//! F4 = Terminal
//! Ctrl+Alt+C = CopyPath
//! ```
//!
//! O que está no arquivo vale antes dos atalhos de fábrica; os de fábrica que não foram trocados continuam
//! valendo. "Editar atalhos" na paleta cria o arquivo com a lista de todos os comandos e abre no editor;
//! "Recarregar atalhos" aplica sem reiniciar. A paleta e a doca mostram o atalho novo.

use crate::commands::{self, Cmd};
use iced::keyboard::{key::Named, Key, Modifiers};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Default)]
struct Map {
    by_chord: HashMap<String, Cmd>,
    by_cmd: HashMap<String, String>,
}

static MAP: RwLock<Option<Map>> = RwLock::new(None);

pub fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor").join("keys"))
}

/// Nome estável do comando no arquivo (o nome da variante: `NewFolder`, `CopyPath`...).
pub fn cmd_id(cmd: Cmd) -> String {
    format!("{cmd:?}")
}

fn all_cmds() -> impl Iterator<Item = Cmd> {
    commands::ALL.iter().copied().chain(std::iter::once(Cmd::Palette))
}

fn parse_cmd(id: &str) -> Option<Cmd> {
    all_cmds().find(|c| cmd_id(*c).eq_ignore_ascii_case(id.trim()))
}

/// Forma canônica de um atalho: modificadores em ordem fixa (`Ctrl`, `Alt`, `Shift`) e a tecla.
pub fn normalize(chord: &str) -> Option<String> {
    let parts: Vec<&str> = chord.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    let (key, mods) = parts.split_last()?;
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    for m in mods {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" | "cmd" | "⌘" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            _ => return None,
        }
    }
    let key = canonical_key(key)?;
    let mut out = String::new();
    for (on, name) in [(ctrl, "Ctrl+"), (alt, "Alt+"), (shift, "Shift+")] {
        if on {
            out.push_str(name);
        }
    }
    out.push_str(&key);
    Some(out)
}

fn canonical_key(k: &str) -> Option<String> {
    let lower = k.to_lowercase();
    let named = [
        "enter",
        "space",
        "tab",
        "backspace",
        "delete",
        "home",
        "end",
        "pageup",
        "pagedown",
        "up",
        "down",
        "left",
        "right",
        "escape",
        "insert",
    ];
    if let Some(n) = named.iter().find(|n| **n == lower) {
        let mut c = n.chars();
        return c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str());
    }
    if let Some(num) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=24).contains(n)) {
        return Some(format!("F{num}"));
    }
    let mut chars = k.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c.to_uppercase().collect()),
        _ => None,
    }
}

/// O atalho de uma tecla apertada, na forma canônica (para comparar com o arquivo).
pub fn chord_of(key: &Key, mods: Modifiers) -> Option<String> {
    let name = match key {
        Key::Named(n) => match n {
            Named::Enter => "Enter".to_string(),
            Named::Space => "Space".into(),
            Named::Tab => "Tab".into(),
            Named::Backspace => "Backspace".into(),
            Named::Delete => "Delete".into(),
            Named::Insert => "Insert".into(),
            Named::Home => "Home".into(),
            Named::End => "End".into(),
            Named::PageUp => "Pageup".into(),
            Named::PageDown => "Pagedown".into(),
            Named::ArrowUp => "Up".into(),
            Named::ArrowDown => "Down".into(),
            Named::ArrowLeft => "Left".into(),
            Named::ArrowRight => "Right".into(),
            Named::Escape => "Escape".into(),
            other => {
                let s = format!("{other:?}");
                if s.starts_with('F') && s[1..].parse::<u8>().is_ok() {
                    s
                } else {
                    return None;
                }
            }
        },
        Key::Character(c) => c.to_uppercase(),
        Key::Unidentified => return None,
    };
    let mut out = String::new();
    if mods.command() {
        out.push_str("Ctrl+");
    }
    if mods.alt() {
        out.push_str("Alt+");
    }
    if mods.shift() {
        out.push_str("Shift+");
    }
    out.push_str(&name);
    Some(out)
}

/// Lê o arquivo de texto. Devolve os atalhos e as linhas com problema (para avisar o usuário).
pub fn parse(text: &str) -> (Vec<(String, Cmd)>, Vec<String>) {
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        match line.split_once('=').and_then(|(k, c)| Some((normalize(k)?, parse_cmd(c)?))) {
            Some(pair) => ok.push(pair),
            None => bad.push(line.to_string()),
        }
    }
    (ok, bad)
}

/// (Re)carrega o arquivo. Devolve quantos atalhos valeram e as linhas ignoradas.
pub fn reload() -> (usize, Vec<String>) {
    let text = file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    let (pairs, bad) = parse(&text);
    let mut map = Map::default();
    for (chord, cmd) in &pairs {
        map.by_chord.insert(chord.clone(), *cmd);
        map.by_cmd.insert(cmd_id(*cmd), display(chord));
    }
    let n = pairs.len();
    if let Ok(mut g) = MAP.write() {
        *g = Some(map);
    }
    (n, bad)
}

fn display(chord: &str) -> String {
    if cfg!(target_os = "macos") {
        chord.replace("Ctrl+", "⌘+")
    } else {
        chord.to_string()
    }
}

/// O comando que o usuário ligou a esta tecla, se houver.
pub fn lookup(key: &Key, mods: Modifiers) -> Option<Cmd> {
    let chord = chord_of(key, mods)?;
    MAP.read().ok()?.as_ref()?.by_chord.get(&chord).copied()
}

/// Atalho personalizado de um comando (para a paleta e a doca).
pub fn custom(cmd: Cmd) -> Option<String> {
    MAP.read().ok()?.as_ref()?.by_cmd.get(&cmd_id(cmd)).cloned()
}

/// Cria o arquivo com instruções e a lista de comandos, se ainda não existir, e devolve o caminho.
pub fn ensure_file() -> Result<PathBuf, String> {
    let f = file().ok_or("sem pasta de configuração")?;
    if f.exists() {
        return Ok(f);
    }
    if let Some(dir) = f.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut text = String::from(
        "# Atalhos do arbor: uma linha por atalho, no formato  Tecla = Comando\n\
         # Modificadores: Ctrl (⌘ no macOS), Alt, Shift. Teclas: letras, números, F1–F24, Enter, Space, Tab,\n\
         # Backspace, Delete, Insert, Home, End, PageUp, PageDown, Up, Down, Left, Right, Escape.\n\
         # O que estiver aqui vale antes dos atalhos de fábrica. Salve e use \"Recarregar atalhos\" na paleta.\n\
         #\n# Exemplo:\n# Ctrl+Shift+N = NewFolder\n#\n# Comandos (atalho de fábrica entre parênteses):\n",
    );
    for cmd in all_cmds() {
        let sc = cmd.default_shortcut();
        let sc = if sc.is_empty() { String::new() } else { format!("  ({sc})") };
        text.push_str(&format!("#   {:<18} {}{}\n", cmd_id(cmd), cmd.label(), sc));
    }
    std::fs::write(&f, text).map_err(|e| e.to_string())?;
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_e_le_o_arquivo() {
        assert_eq!(normalize("shift + ctrl + n").as_deref(), Some("Ctrl+Shift+N"));
        assert_eq!(normalize("f4").as_deref(), Some("F4"));
        assert_eq!(normalize("Alt+pageDown").as_deref(), Some("Alt+Pagedown"));
        assert_eq!(normalize("Hyper+X"), None);
        let (ok, bad) = parse("# comentário\nCtrl+Shift+N = NewFolder\nF4 = terminal # abre o terminal\nCtrl+Q = NaoExiste\n");
        assert_eq!(ok, vec![("Ctrl+Shift+N".to_string(), Cmd::NewFolder), ("F4".to_string(), Cmd::Terminal)]);
        assert_eq!(bad, ["Ctrl+Q = NaoExiste"]);
    }

    #[test]
    fn tecla_apertada_vira_o_mesmo_texto() {
        let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
        let n = Key::Character("n".into());
        // No macOS `command()` é a tecla ⌘; nos outros, Ctrl.
        let mods = if cfg!(target_os = "macos") { Modifiers::LOGO | Modifiers::SHIFT } else { ctrl_shift };
        assert_eq!(chord_of(&n, mods).as_deref(), Some("Ctrl+Shift+N"));
        assert_eq!(chord_of(&Key::Named(Named::F4), Modifiers::empty()).as_deref(), Some("F4"));
    }
}
