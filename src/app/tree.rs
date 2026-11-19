//! Árvore de pastas da barra lateral (como o painel de navegação do Explorer): discos e locais se
//! expandem em subpastas, lidas fora da interface na primeira vez que se abre cada nó.

use super::{App, Message};
use iced::Task;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Pastas com mais filhos que isso mostram só as primeiras (uma árvore não é lugar para 50 mil itens).
pub const MAX_CHILDREN: usize = 500;

#[derive(Default)]
pub struct Tree {
    pub open: HashSet<PathBuf>,
    pub children: HashMap<PathBuf, Vec<PathBuf>>,
}

/// Subpastas de `dir` em ordem natural; ocultas só com `hidden`.
pub fn subdirs(dir: &Path, hidden: bool) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<(Box<[u8]>, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let is_dir = e.file_type().is_ok_and(|t| t.is_dir() || (t.is_symlink() && e.path().is_dir()));
            (is_dir && (hidden || !name.starts_with('.'))).then(|| (arbor_core::entry::natural_key(&name), e.path()))
        })
        .collect();
    v.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    v.into_iter().take(MAX_CHILDREN).map(|(_, p)| p).collect()
}

impl App {
    pub(super) fn tree_toggle(&mut self, path: PathBuf) -> Task<Message> {
        if self.tree.open.remove(&path) {
            return Task::none();
        }
        self.tree.open.insert(path.clone());
        let hidden = self.show_hidden;
        let p = path.clone();
        Task::perform(async move { tokio::task::spawn_blocking(move || subdirs(&p, hidden)).await.unwrap_or_default() }, move |dirs| {
            Message::TreeLoaded { path: path.clone(), dirs }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subpastas_em_ordem_natural_sem_arquivos_nem_ocultas() {
        let t = tempfile::tempdir().unwrap();
        for d in ["img10", "img2", ".git", "Álbum"] {
            std::fs::create_dir(t.path().join(d)).unwrap();
        }
        std::fs::write(t.path().join("a.txt"), "").unwrap();
        let names = |v: Vec<PathBuf>| v.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();
        assert_eq!(names(subdirs(t.path(), false)), ["img2", "img10", "Álbum"]);
        assert_eq!(subdirs(t.path(), true).len(), 4);
        assert!(subdirs(&t.path().join("nao-existe"), false).is_empty());
    }
}
