//! Filtros de cor (XYplorer "Color Filters", cores por tipo do Total Commander): pinta o nome dos itens que
//! casam com uma máscara. Não é tag: é regra visual, sem nada gravado por arquivo. Arquivo `colors` na
//! pasta de configuração, uma regra por linha, a primeira que casa vale:
//!
//! ```text
//! *.log;*.tmp = cinza
//! *.rs = #f74c00
//! ```

use arbor_core::search::Mask;
use iced::Color;
use std::path::PathBuf;
use std::sync::RwLock;

static RULES: RwLock<Vec<(Mask, Color)>> = RwLock::new(Vec::new());

const NAMED: &[(&str, (u8, u8, u8))] = &[
    ("vermelho", (0xff, 0x6b, 0x6b)),
    ("laranja", (0xff, 0xa9, 0x4d)),
    ("amarelo", (0xff, 0xd8, 0x4d)),
    ("verde", (0x6b, 0xe3, 0x8f)),
    ("azul", (0x6b, 0xb6, 0xff)),
    ("roxo", (0xb9, 0x8c, 0xff)),
    ("rosa", (0xff, 0x8c, 0xc6)),
    ("cinza", (0x8a, 0x90, 0xa0)),
];

pub fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor").join("colors"))
}

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim().to_lowercase();
    if let Some(&(_, (r, g, b))) = NAMED.iter().find(|(n, _)| *n == s) {
        return Some(Color::from_rgb8(r, g, b));
    }
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some(Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Regras válidas e as linhas que não deu para ler.
fn parse(text: &str) -> (Vec<(Mask, Color)>, Vec<String>) {
    let (mut rules, mut bad) = (vec![], vec![]);
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        match line.rsplit_once('=').and_then(|(m, c)| Some((Mask::parse(m)?, parse_color(c)?))) {
            Some(rule) => rules.push(rule),
            None => bad.push(line.to_string()),
        }
    }
    (rules, bad)
}

/// Relê o arquivo; devolve quantas regras valem e as linhas ignoradas.
pub fn reload() -> (usize, Vec<String>) {
    let text = file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    let (rules, bad) = parse(&text);
    let n = rules.len();
    if let Ok(mut r) = RULES.write() {
        *r = rules;
    }
    (n, bad)
}

/// Cor do nome, se alguma regra casa.
pub fn of(name: &str) -> Option<Color> {
    RULES.read().ok()?.iter().find(|(m, _)| m.matches(name)).map(|(_, c)| *c)
}

/// Cria o arquivo com exemplos comentados, se ainda não existe.
pub fn ensure_file() -> Result<PathBuf, String> {
    let f = file().ok_or("sem pasta de configuração")?;
    if !f.exists() {
        if let Some(dir) = f.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let names: Vec<&str> = NAMED.iter().map(|(n, _)| *n).collect();
        let text = format!(
            "# Filtros de cor do arbor: máscara = cor. A primeira regra que casa vale.\n\
             # Cores: {} ou #rrggbb. Salve e use \"Recarregar filtros de cor\" na paleta.\n#\n\
             # *.log;*.tmp;*.bak = cinza\n# *.rs;*.toml = laranja\n# *.pdf = vermelho\n",
            names.join(", ")
        );
        std::fs::write(&f, text).map_err(|e| e.to_string())?;
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_regras_cores_por_nome_e_hex_e_aponta_linhas_ruins() {
        let (rules, bad) = parse("# comentário\n*.log;*.tmp = cinza\n*.rs = #F74C00\nsem igual\n*.x = cor-que-nao-existe\n");
        assert_eq!(rules.len(), 2);
        assert!(rules[0].0.matches("a.TMP"));
        assert_eq!(rules[1].1, Color::from_rgb8(0xf7, 0x4c, 0x00));
        assert_eq!(bad, ["sem igual", "*.x = cor-que-nao-existe"]);
    }
}
