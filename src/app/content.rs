//! Comandos de conteúdo de arquivo: somas SHA-256 e comparar dois arquivos, rodando fora da interface.

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

impl App {
    /// Dois arquivos: os dois selecionados, ou o do cursor em cada painel.
    fn compare_pair(&self) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
        let sel = self.tab().targets();
        if let [a, b] = sel.as_slice() {
            return Some((a.clone(), b.clone()));
        }
        if self.dual {
            let a = self.panes[0].tab().current()?.path.clone();
            let b = self.panes[1].tab().current()?.path.clone();
            return Some((a, b));
        }
        None
    }

    pub(super) fn compare_files(&mut self) -> Task<Message> {
        let Some((a, b)) = self.compare_pair().filter(|(a, b)| a.is_file() && b.is_file()) else {
            return self.notify("Selecione dois arquivos (ou um em cada painel)", false);
        };
        blocking(move || match crate::diff::first_difference(&a, &b) {
            Ok(None) => Ok("Arquivos idênticos".into()),
            Ok(Some(at)) => crate::diff::open_tool(&a, &b)
                .map(|tool| format!("Diferem a partir do byte {at}; abrindo no {tool}"))
                .map_err(|e| format!("Diferem a partir do byte {at}; {e}")),
            Err(e) => Err(e.to_string()),
        })
    }
}
