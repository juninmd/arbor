//! Comandos "Criar soma SHA-256" e "Conferir somas SHA-256", rodando fora da interface.

use super::{App, Message};
use crate::checksums;
use iced::Task;

fn blocking(f: impl FnOnce() -> Result<String, String> + Send + 'static) -> Task<Message> {
    Task::perform(async move { tokio::task::spawn_blocking(f).await.unwrap_or_else(|e| Err(e.to_string())) }, Message::Notice)
}

impl App {
    pub(super) fn create_checksums(&mut self) -> Task<Message> {
        let dir = self.tab().path.clone();
        let files: Vec<_> = self.tab().targets().into_iter().filter(|p| p.is_file()).collect();
        if files.is_empty() {
            return self.notify("Selecione os arquivos para somar", false);
        }
        self.tab_mut().focus_on_load = vec![checksums::target(&dir, &files)];
        blocking(move || checksums::create(&dir, &files).map(|p| format!("Somas gravadas em {}", arbor_core::ops::name(&p))))
    }

    pub(super) fn verify_checksums(&mut self) -> Task<Message> {
        let Some(sums) = self.tab().current().filter(|e| !e.is_dir).map(|e| e.path.clone()) else {
            return self.notify("Escolha um arquivo .sha256", false);
        };
        blocking(move || checksums::verify(&sums))
    }
}
