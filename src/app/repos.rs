//! Pastas que são repositórios git ganham um selo no ícone. A checagem (um `stat` de `.git` por
//! subpasta) roda fora da interface depois da listagem completa, com teto para pastas enormes.

use super::{App, Message};
use crate::{discovery, network};
use iced::Task;
use std::collections::HashSet;
use std::path::PathBuf;

/// Acima disso a pasta é um depósito, não um lugar de projetos: não vale um `stat` por subpasta.
const MAX_DIRS: usize = 4096;

fn scan(dirs: Vec<PathBuf>) -> HashSet<PathBuf> {
    dirs.into_iter().filter(|d| crate::props::is_repo(d)).collect()
}

impl App {
    pub(super) fn scan_repos(&mut self, id: u64) -> Task<Message> {
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        // Compartilhamentos e compactados: lento ou inexistente no disco.
        if discovery::is(&tab.path) || network::server_root(&tab.path).is_some() || arbor_core::archive::split(&tab.path).is_some() {
            return Task::none();
        }
        let dirs: Vec<PathBuf> = tab.entries.iter().filter(|e| e.is_dir).map(|e| e.path.clone()).collect();
        if dirs.is_empty() || dirs.len() > MAX_DIRS {
            tab.repos.clear();
            return Task::none();
        }
        let dir = tab.path.clone();
        Task::perform(async move { tokio::task::spawn_blocking(move || scan(dirs)).await.unwrap_or_default() }, move |repos| {
            Message::Repos { tab: id, dir: dir.clone(), repos }
        })
    }

    pub(super) fn on_repos(&mut self, id: u64, dir: PathBuf, repos: HashSet<PathBuf>) -> Task<Message> {
        // A aba pode ter mudado de pasta enquanto checava.
        if let Some(tab) = self.find_tab(id).filter(|t| t.path == dir) {
            tab.repos = repos;
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_dir_or_worktree_file_marks_a_repo() {
        let root = std::env::temp_dir().join(format!("arbor-repos-{}", std::process::id()));
        let (clone, worktree, plain) = (root.join("clone"), root.join("wt"), root.join("plain"));
        std::fs::create_dir_all(clone.join(".git")).unwrap();
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: ../clone/.git/worktrees/wt").unwrap();
        std::fs::create_dir_all(&plain).unwrap();
        let found = scan(vec![clone.clone(), worktree.clone(), plain.clone()]);
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(found, HashSet::from([clone, worktree]), "pasta comum não ganha selo");
    }
}
