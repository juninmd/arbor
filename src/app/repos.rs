//! Git na pasta aberta: subpastas que são repositórios (um `stat` de `.git` cada, com teto) e o estado
//! do repositório onde a pasta fica (branch, worktree, diff). Tudo fora da interface, após a listagem.

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
            tab.git = None;
            return Task::none();
        }
        let dirs: Vec<PathBuf> = tab.entries.iter().filter(|e| e.is_dir).map(|e| e.path.clone()).collect();
        let dir = tab.path.clone();
        if dirs.is_empty() || dirs.len() > MAX_DIRS {
            tab.repos.clear();
            return self.scan_git(id, dir);
        }
        let git = self.scan_git(id, dir.clone());
        let repos = Task::perform(async move { tokio::task::spawn_blocking(move || scan(dirs)).await.unwrap_or_default() }, move |repos| {
            Message::Repos { tab: id, dir: dir.clone(), repos }
        });
        Task::batch([git, repos])
    }

    fn scan_git(&self, id: u64, dir: PathBuf) -> Task<Message> {
        let d = dir.clone();
        Task::perform(async move { tokio::task::spawn_blocking(move || crate::git::read(&d)).await.ok().flatten() }, move |info| {
            Message::Git { tab: id, dir: dir.clone(), info }
        })
    }

    pub(super) fn on_git(&mut self, id: u64, dir: PathBuf, info: Option<crate::git::GitInfo>) -> Task<Message> {
        if let Some(tab) = self.find_tab(id).filter(|t| t.path == dir) {
            tab.git = info;
        }
        Task::none()
    }

    pub(super) fn on_repos(&mut self, id: u64, dir: PathBuf, repos: HashSet<PathBuf>) -> Task<Message> {
        // A aba pode ter mudado de pasta enquanto checava.
        if let Some(tab) = self.find_tab(id).filter(|t| t.path == dir) {
            tab.repos = repos;
        }
        Task::none()
    }
}

impl App {
    /// Branch do git da pasta da aba, lida fora da interface depois da listagem.
    pub(super) fn read_branch(&mut self, id: u64) -> Task<Message> {
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        if crate::computer::is_virtual(&tab.path) || network::server_root(&tab.path).is_some() {
            tab.branch = None;
            return Task::none();
        }
        let dir = tab.path.clone();
        Task::perform(
            {
                let dir = dir.clone();
                async move { tokio::task::spawn_blocking(move || crate::git_branch::branch_of(&dir)).await.ok().flatten() }
            },
            move |branch| Message::Branch { tab: id, dir: dir.clone(), branch },
        )
    }

    pub(super) fn on_branch(&mut self, id: u64, dir: PathBuf, branch: Option<String>) -> Task<Message> {
        if let Some(tab) = self.find_tab(id).filter(|t| t.path == dir) {
            tab.branch = branch;
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
