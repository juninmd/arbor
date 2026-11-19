//! Painel de detalhes à direita (Alt+Shift+P), como no Explorer: preview e propriedades do item
//! sob o cursor, lidos fora da interface sempre que o cursor muda de item.

use super::{App, Message};
use crate::media::{self, Preview};
use crate::props;
use iced::Task;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Details {
    pub path: PathBuf,
    pub preview: Option<Preview>,
    pub props: Vec<(String, String)>,
    /// `None` antes de pedir; texto "Calculando…" enquanto lê o arquivo.
    pub hash: Option<String>,
}

impl App {
    /// Chamado depois de toda mensagem; só lê o disco quando o item sob o cursor mudou.
    pub(super) fn refresh_details(&mut self) -> Task<Message> {
        if !self.details_on {
            return Task::none();
        }
        // Na Lixeira o arquivo não está no caminho original: mostra o que a própria lista sabe.
        if crate::trashbin::is(&self.tab().path) {
            let Some(e) = self.tab().current() else {
                self.details = None;
                return Task::none();
            };
            if self.details.as_ref().is_none_or(|d| d.path != e.path) {
                let parent = e.path.parent().map(|p| p.display().to_string()).unwrap_or_default();
                let mut props = vec![("Local original".to_string(), parent), ("Excluído em".into(), arbor_core::human_time(e.modified))];
                if !e.is_dir {
                    props.push(("Tamanho".into(), arbor_core::human_size(e.size)));
                }
                self.details = Some(Details { path: e.path.clone(), preview: Some(Preview::Other { info: vec![] }), props, hash: None });
            }
            return Task::none();
        }
        let want = self.tab().current().map(|e| e.path.clone());
        match (want, &self.details) {
            (Some(w), Some(d)) if d.path == w => Task::none(),
            (Some(w), _) => {
                self.details = Some(Details { path: w.clone(), preview: None, props: vec![], hash: None });
                let path = w.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || (media::load_preview(&w), props::props(&w)))
                            .await
                            .unwrap_or_else(|_| (Preview::Error("falhou".into()), vec![]))
                    },
                    move |(preview, props)| Message::DetailsLoaded { path: path.clone(), preview, props },
                )
            }
            (None, _) => {
                self.details = None;
                Task::none()
            }
        }
    }

    pub(super) fn on_details(&mut self, path: PathBuf, preview: Preview, props: Vec<(String, String)>) -> Task<Message> {
        if let Some(d) = self.details.as_mut().filter(|d| d.path == path) {
            d.preview = Some(preview);
            d.props = props;
        }
        Task::none()
    }

    /// Hash sob demanda: ler um arquivo de vários GB a cada troca de cursor seria caro demais.
    pub(super) fn hash_file(&mut self, path: PathBuf) -> Task<Message> {
        let Some(d) = self.details.as_mut().filter(|d| d.path == path) else { return Task::none() };
        d.hash = Some("Calculando…".into());
        let p = path.clone();
        Task::perform(
            async move { tokio::task::spawn_blocking(move || props::sha256(&p)).await.unwrap_or_else(|e| Err(e.to_string())) },
            move |result| Message::Hashed { path: path.clone(), result },
        )
    }

    pub(super) fn on_hashed(&mut self, path: PathBuf, result: Result<String, String>) -> Task<Message> {
        if let Some(d) = self.details.as_mut().filter(|d| d.path == path) {
            d.hash = Some(result.unwrap_or_else(|e| format!("Não foi possível ler: {e}")));
        }
        Task::none()
    }
}
