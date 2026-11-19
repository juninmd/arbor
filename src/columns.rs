//! Colunas da lista (Tipo, Tamanho, Modificado): mostrar/ocultar e largura por arraste, como no
//! Explorer. Guardadas em `~/.config/arbor/columns`, uma por linha: `chave largura visível`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Col {
    Kind,
    Size,
    Modified,
}

pub const ALL: [Col; 3] = [Col::Kind, Col::Size, Col::Modified];
pub const MIN_W: f32 = 40.0;
pub const MAX_W: f32 = 400.0;

impl Col {
    fn index(self) -> usize {
        self as usize
    }

    fn code(self) -> &'static str {
        match self {
            Col::Kind => "kind",
            Col::Size => "size",
            Col::Modified => "modified",
        }
    }

    fn default_width(self) -> f32 {
        match self {
            Col::Kind => 64.0,
            Col::Size => 80.0,
            Col::Modified => 118.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Columns {
    show: [bool; 3],
    width: [f32; 3],
}

impl Default for Columns {
    fn default() -> Self {
        Columns { show: [true; 3], width: ALL.map(Col::default_width) }
    }
}

impl Columns {
    pub fn shown(&self, c: Col) -> bool {
        self.show[c.index()]
    }

    pub fn width(&self, c: Col) -> f32 {
        self.width[c.index()]
    }

    pub fn toggle(&mut self, c: Col) {
        self.show[c.index()] = !self.show[c.index()];
    }

    pub fn set_width(&mut self, c: Col, w: f32) {
        self.width[c.index()] = if w.is_finite() { w.clamp(MIN_W, MAX_W) } else { c.default_width() };
    }

    pub fn file() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("arbor").join("columns"))
    }

    pub fn load(file: Option<&Path>) -> Columns {
        let mut c = Columns::default();
        let text = file.and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            let (Some(code), Some(w), Some(show)) = (it.next(), it.next(), it.next()) else { continue };
            let Some(col) = ALL.into_iter().find(|c| c.code() == code) else { continue };
            if let Ok(w) = w.parse::<f32>() {
                c.set_width(col, w);
            }
            c.show[col.index()] = show != "0";
        }
        c
    }

    pub fn save(&self, file: Option<&Path>) -> Result<(), String> {
        let file = file.ok_or("sem pasta de configuração")?;
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text: String = ALL.iter().map(|&c| format!("{} {:.0} {}\n", c.code(), self.width(c), self.shown(c) as u8)).collect();
        std::fs::write(file, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocultar_e_alargar_sobrevive_a_reabrir() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("cfg").join("columns");
        let mut c = Columns::load(Some(&f));
        assert_eq!(c, Columns::default(), "sem arquivo usa o padrão");
        c.toggle(Col::Kind);
        c.set_width(Col::Modified, 160.0);
        c.save(Some(&f)).unwrap();
        let c = Columns::load(Some(&f));
        assert!(!c.shown(Col::Kind) && c.shown(Col::Size));
        assert_eq!(c.width(Col::Modified), 160.0);
    }

    #[test]
    fn largura_fica_nos_limites_e_linha_ruim_e_ignorada() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("columns");
        std::fs::write(&f, "size 5 1\nlixo\nkind abc 0\nmodified 9999 1\n").unwrap();
        let c = Columns::load(Some(&f));
        assert_eq!(c.width(Col::Size), MIN_W);
        assert_eq!(c.width(Col::Modified), MAX_W);
        assert!(!c.shown(Col::Kind));
        assert_eq!(c.width(Col::Kind), 64.0);
        let mut c = c;
        c.set_width(Col::Size, f32::NAN);
        assert_eq!(c.width(Col::Size), 80.0);
    }
}
