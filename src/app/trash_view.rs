//! Comandos da Lixeira navegável e o "Excluir permanentemente" (Shift+Delete), sempre com confirmação.

use super::{App, Message, Overlay};
use crate::commands::Cmd;
use crate::{computer, trashbin};
use arbor_core::{Entry, Snapshot};
use iced::Task;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Irreversible {
    Purge(Vec<PathBuf>),
    Empty,
    DeleteNow(Vec<PathBuf>),
}

fn blocking(f: impl FnOnce() -> Result<String, String> + Send + 'static) -> Task<Message> {
    Task::perform(async move { tokio::task::spawn_blocking(f).await.unwrap_or_else(|e| Err(e.to_string())) }, Message::TrashDone)
}

fn count(n: usize) -> String {
    if n == 1 {
        "1 item".into()
    } else {
        format!("{n} itens")
    }
}

impl App {
    pub(super) fn on_trash_listed(&mut self, result: Result<Vec<Entry>, String>) -> Task<Message> {
        let (entries, error) = match result {
            Ok(v) => (v, None),
            Err(e) => (vec![], Some(e)),
        };
        let snap = Snapshot { entries: Arc::new(entries), complete: true, elapsed: Duration::ZERO, error };
        let tabs: Vec<_> =
            self.panes.iter().flat_map(|p| &p.tabs).filter(|t| trashbin::is(&t.path)).map(|t| (t.id, t.generation)).collect();
        Task::batch(tabs.into_iter().map(|(id, generation)| self.on_listed(id, generation, snap.clone())).collect::<Vec<_>>())
    }

    pub(super) fn trash_cmd(&mut self, cmd: Cmd) -> Task<Message> {
        let in_trash = trashbin::is(&self.tab().path);
        let targets = self.tab().targets();
        match cmd {
            Cmd::Restore if in_trash && !targets.is_empty() => {
                blocking(move || trashbin::restore(&targets).map(|n| format!("{} restaurado(s)", count(n))))
            }
            Cmd::Purge if in_trash && !targets.is_empty() => {
                let text = format!("Excluir {} da Lixeira para sempre?", count(targets.len()));
                self.overlay = Overlay::Confirm { text, action: Irreversible::Purge(targets) };
                Task::none()
            }
            Cmd::EmptyTrash => {
                self.overlay = Overlay::Confirm { text: "Esvaziar a Lixeira? Não dá para desfazer.".into(), action: Irreversible::Empty };
                Task::none()
            }
            Cmd::DeleteNow if !computer::is_virtual(&self.tab().path) && !targets.is_empty() => {
                let text = format!("Excluir {} permanentemente, sem passar pela Lixeira?", count(targets.len()));
                self.overlay = Overlay::Confirm { text, action: Irreversible::DeleteNow(targets) };
                Task::none()
            }
            Cmd::DeleteNow if trashbin::is(&self.tab().path) => self.trash_cmd(Cmd::Purge),
            _ => Task::none(),
        }
    }

    pub(super) fn confirmed(&mut self) -> Task<Message> {
        let Overlay::Confirm { action, .. } = std::mem::replace(&mut self.overlay, Overlay::None) else { return Task::none() };
        match action {
            Irreversible::Purge(paths) => blocking(move || trashbin::purge(&paths).map(|n| format!("{} excluído(s) da Lixeira", count(n)))),
            Irreversible::Empty => blocking(|| trashbin::empty().map(|n| format!("Lixeira esvaziada ({})", count(n)))),
            Irreversible::DeleteNow(paths) => {
                blocking(move || trashbin::delete_now(&paths).map(|n| format!("{} excluído(s) permanentemente", count(n))))
            }
        }
    }

    pub(super) fn on_trash_done(&mut self, result: Result<String, String>) -> Task<Message> {
        let note = match result {
            Ok(text) => self.notify(text, false),
            Err(e) => self.notify(format!("Lixeira: {e}"), true),
        };
        // A Lixeira não tem observador de disco: relista as abas abertas nela.
        let reload: Vec<usize> = (0..2).filter(|&p| trashbin::is(&self.panes[p].tab().path)).collect();
        let tasks: Vec<_> = reload.into_iter().map(|p| self.load(p, trashbin::path(), true)).collect();
        Task::batch(tasks.into_iter().chain([note]))
    }
}
