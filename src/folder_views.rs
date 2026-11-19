//! Cada pasta lembra ordenação e modo (lista/grade), como no Explorer. Um registro por linha em
//! `~/.config/arbor/views`: `chave desc grade<TAB>caminho`.

use arbor_core::entry::{Sort, SortKey};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Limite de pastas lembradas: o arquivo não cresce sem fim.
const MAX: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    pub sort: Sort,
    pub grid: bool,
}

#[derive(Debug, Default)]
pub struct FolderViews {
    file: Option<PathBuf>,
    map: HashMap<PathBuf, View>,
}

fn key_code(k: SortKey) -> &'static str {
    match k {
        SortKey::Name => "name",
        SortKey::Size => "size",
        SortKey::Modified => "modified",
        SortKey::Kind => "kind",
    }
}

fn parse_line(line: &str) -> Option<(PathBuf, View)> {
    let (head, path) = line.split_once('\t')?;
    let mut it = head.split(' ');
    let key = match it.next()? {
        "name" => SortKey::Name,
        "size" => SortKey::Size,
        "modified" => SortKey::Modified,
        "kind" => SortKey::Kind,
        _ => return None,
    };
    let desc = it.next()? == "1";
    let grid = it.next()? == "1";
    (!path.is_empty()).then(|| (PathBuf::from(path), View { sort: Sort { key, desc }, grid }))
}

impl FolderViews {
    pub fn load(file: Option<PathBuf>) -> FolderViews {
        let text = file.as_deref().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
        let map = text.lines().filter_map(parse_line).take(MAX).collect();
        FolderViews { file, map }
    }

    pub fn default_file() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("arbor").join("views"))
    }

    pub fn get(&self, dir: &Path) -> View {
        self.map.get(dir).copied().unwrap_or_default()
    }

    /// Guarda a visão da pasta; a padrão não ocupa linha.
    pub fn set(&mut self, dir: &Path, view: View) -> Result<(), String> {
        if view == View::default() {
            if self.map.remove(dir).is_none() {
                return Ok(());
            }
        } else if (self.map.len() >= MAX && !self.map.contains_key(dir)) || self.map.insert(dir.to_path_buf(), view) == Some(view) {
            return Ok(());
        }
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(file) = &self.file else { return Ok(()) };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text: String = self
            .map
            .iter()
            .filter(|(p, _)| !p.to_string_lossy().contains(['\n', '\t']))
            .map(|(p, v)| format!("{} {} {}\t{}\n", key_code(v.sort.key), v.sort.desc as u8, v.grid as u8, p.to_string_lossy()))
            .collect();
        std::fs::write(file, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lembra_por_pasta_e_sobrevive_a_reabrir() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("cfg").join("views");
        let mut v = FolderViews::load(Some(f.clone()));
        let fotos = Path::new("/home/ana/Fotos ação");
        assert_eq!(v.get(fotos), View::default(), "pasta nova usa o padrão");
        let grade = View { sort: Sort { key: SortKey::Modified, desc: true }, grid: true };
        v.set(fotos, grade).unwrap();
        let v = FolderViews::load(Some(f.clone()));
        assert_eq!(v.get(fotos), grade);
        assert_eq!(v.get(Path::new("/outra")), View::default());
    }

    #[test]
    fn voltar_ao_padrao_apaga_e_linha_ruim_e_ignorada() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("views");
        std::fs::write(&f, "lixo\nsize 0 1\t/a\nzzz 1 1\t/b\n").unwrap();
        let mut v = FolderViews::load(Some(f.clone()));
        assert!(v.get(Path::new("/a")).grid);
        assert_eq!(v.get(Path::new("/b")), View::default());
        v.set(Path::new("/a"), View::default()).unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "");
    }
}
