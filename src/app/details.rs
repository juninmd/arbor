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
}

impl App {
    /// Chamado depois de toda mensagem; só lê o disco quando o item sob o cursor mudou.
    pub(super) fn refresh_details(&mut self) -> Task<Message> {
        if !self.details_on {
            return Task::none();
        }
        let want = self.tab().current().map(|e| e.path.clone());
        match (want, &self.details) {
            (Some(w), Some(d)) if d.path == w => Task::none(),
            (Some(w), _) => {
                self.details = Some(Details { path: w.clone(), preview: None, props: vec![] });
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
}
