//! Tamanho das pastas na coluna Tamanho (Directory Opus, XYplorer, o Espaço do Total Commander). Desligado
//! por padrão porque varre a árvore inteira; ligado pela paleta, fica salvo. Roda depois da listagem
//! completa, fora da interface, e para quando a aba muda de pasta.

use super::{App, Message, Tab};
use crate::{computer, discovery, network};
use arbor_core::{sort_entries, Entry, SortKey};
use iced::Task;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const SETTING: &str = "folder_sizes";

pub fn enabled() -> bool {
    crate::session::setting(SETTING).as_deref() == Some("true")
}

/// Cancela a medição em andamento da aba (troca de pasta, nova listagem).
pub fn cancel(tab: &mut Tab) {
    tab.sizes_stop.store(true, Ordering::Relaxed);
    tab.dir_sizes = false;
}

impl App {
    pub(super) fn measure_dirs(&mut self, id: u64) -> Task<Message> {
        if !self.folder_sizes {
            return Task::none();
        }
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        let path = &tab.path;
        // Compartilhamentos, compactados e listas virtuais: lento ou sem árvore no disco.
        if tab.search.is_some()
            || computer::is_virtual(path)
            || discovery::is(path)
            || network::server_root(path).is_some()
            || arbor_core::archive::split(path).is_some()
        {
            return Task::none();
        }
        let dirs: Vec<PathBuf> = tab.entries.iter().filter(|e| e.is_dir && !e.is_link).map(|e| e.path.clone()).collect();
        if dirs.is_empty() {
            return Task::none();
        }
        cancel(tab);
        let stop = Arc::new(AtomicBool::new(false));
        tab.sizes_stop = stop.clone();
        let dir = tab.path.clone();
        Task::perform(
            async move { tokio::task::spawn_blocking(move || arbor_core::du::sizes(&dirs, &stop)).await.ok().flatten() },
            move |sizes| Message::DirSizes { tab: id, dir: dir.clone(), sizes },
        )
    }

    pub(super) fn on_dir_sizes(&mut self, id: u64, dir: PathBuf, sizes: Option<Vec<(PathBuf, u64)>>) -> Task<Message> {
        let show = self.show_hidden;
        let Some(sizes) = sizes else { return Task::none() };
        let Some(tab) = self.find_tab(id).filter(|t| t.path == dir) else { return Task::none() };
        let sizes: std::collections::HashMap<PathBuf, u64> = sizes.into_iter().collect();
        let sort = tab.sort;
        tab.edit_entries(show, |entries| {
            let list = Arc::<Vec<Entry>>::make_mut(entries);
            for e in list.iter_mut().filter(|e| e.is_dir) {
                if let Some(&s) = sizes.get(&e.path) {
                    e.size = s;
                }
            }
            if sort.key == SortKey::Size {
                sort_entries(list, sort);
            }
        });
        tab.dir_sizes = true;
        Task::none()
    }

    pub(super) fn toggle_folder_sizes(&mut self) -> Task<Message> {
        self.folder_sizes = !self.folder_sizes;
        let saved = crate::session::set_setting(SETTING, if self.folder_sizes { "true" } else { "false" });
        let show = self.show_hidden;
        let mut tasks = vec![];
        let ids: Vec<u64> = self.panes.iter().flat_map(|p| p.tabs.iter().map(|t| t.id)).collect();
        for id in ids {
            if self.folder_sizes {
                tasks.push(self.measure_dirs(id));
            } else if let Some(tab) = self.find_tab(id) {
                cancel(tab);
                tab.edit_entries(show, |entries| {
                    for e in Arc::<Vec<Entry>>::make_mut(entries).iter_mut().filter(|e| e.is_dir) {
                        e.size = 0;
                    }
                });
            }
        }
        let text = match saved {
            Err(e) => format!("Tamanho das pastas: {e}"),
            Ok(()) if self.folder_sizes => "Tamanho das pastas ligado: calculado em segundo plano".into(),
            Ok(()) => "Tamanho das pastas desligado".into(),
        };
        tasks.push(self.notify(text, false));
        Task::batch(tasks)
    }
}
