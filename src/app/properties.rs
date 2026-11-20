//! Janela "Propriedades" (Alt+Enter ou menu de contexto), como a do Explorer: tipo, local, tamanho,
//! datas e atributos do item clicado. Lida fora da interface; pasta grande mostra o tamanho ao chegar.

use super::{App, Message, Overlay};
use crate::media::{self, Preview};
use crate::{computer, props};
use iced::Task;
use std::path::PathBuf;

impl App {
    /// Alvo: o item do menu de contexto; senão o único selecionado, o sob o cursor ou a própria pasta.
    fn properties_target(&self) -> Option<PathBuf> {
        if let Some(t) = &self.menu_target {
            return Some(t.clone());
        }
        let tab = self.tab();
        if tab.selected.len() == 1 {
            return tab.selected.iter().next().cloned();
        }
        tab.current().map(|e| e.path.clone()).or_else(|| Some(tab.path.clone()))
    }

    pub(super) fn open_properties(&mut self) -> Task<Message> {
        let Some(path) = self.properties_target().filter(|p| !computer::is_virtual(p)) else {
            return self.notify("Este local não tem propriedades", false);
        };
        self.overlay = Overlay::Properties { path: path.clone(), props: None, preview: None };
        let mut extra = vec![];
        if let Some((free, total)) = self.drives.iter().find(|d| d.path == path).and_then(|d| d.space) {
            extra.push(("Capacidade".to_string(), arbor_core::human_size(total)));
            extra.push(("Espaço livre".to_string(), arbor_core::human_size(free)));
        }
        let p = path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || (media::load_preview(&p), props::props(&p)))
                    .await
                    .unwrap_or_else(|_| (Preview::Error("falhou".into()), vec![]))
            },
            move |(preview, mut props)| {
                props.extend(extra.clone());
                Message::PropsLoaded { path: path.clone(), preview, props }
            },
        )
    }

    pub(super) fn on_props(&mut self, path: PathBuf, loaded: Preview, list: Vec<(String, String)>) -> Task<Message> {
        if let Overlay::Properties { path: p, props, preview } = &mut self.overlay {
            if *p == path {
                *props = Some(list);
                *preview = Some(loaded);
            }
        }
        Task::none()
    }
}
