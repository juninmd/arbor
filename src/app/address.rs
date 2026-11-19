//! Autocompletar da caixa "Ir para" (Ctrl+L): sugestões lidas fora da interface, setas escolhem,
//! Tab completa e Enter abre.

use super::{App, Message, Overlay, PromptKind, PROMPT_INPUT};
use crate::complete;
use iced::keyboard::key::Named;
use iced::widget::operation as op;
use iced::Task;
use std::path::PathBuf;

#[derive(Default)]
pub struct Suggest {
    generation: u64,
    pub items: Vec<PathBuf>,
    pub selected: Option<usize>,
}

impl App {
    pub(super) fn request_suggestions(&mut self, value: String) -> Task<Message> {
        self.suggest.generation += 1;
        self.suggest.selected = None;
        let generation = self.suggest.generation;
        let base = self.tab().path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || complete::suggestions(&value, &base, dirs::home_dir().as_deref()))
                    .await
                    .unwrap_or_default()
            },
            move |items| Message::Suggested { generation, items },
        )
    }

    pub(super) fn on_suggested(&mut self, generation: u64, items: Vec<PathBuf>) -> Task<Message> {
        if generation == self.suggest.generation && matches!(self.overlay, Overlay::Prompt { kind: PromptKind::GoTo, .. }) {
            self.suggest.items = items;
        }
        Task::none()
    }

    /// Tab completa com a sugestão escolhida (ou a primeira) e já pede as subpastas dela.
    pub(super) fn accept_suggestion(&mut self, index: Option<usize>) -> Task<Message> {
        let Some(path) = index.or(self.suggest.selected).or(Some(0)).and_then(|i| self.suggest.items.get(i)).cloned() else {
            return Task::none();
        };
        let text = complete::accept(&path);
        if let Overlay::Prompt { value, .. } = &mut self.overlay {
            *value = text.clone();
        }
        Task::batch([self.request_suggestions(text), op::focus(PROMPT_INPUT), op::move_cursor_to_end(PROMPT_INPUT)])
    }

    pub(super) fn prompt_key(&mut self, named: Option<Named>) -> Task<Message> {
        let n = self.suggest.items.len();
        match named {
            Some(Named::Escape) => self.overlay = Overlay::None,
            Some(Named::Tab) => return self.accept_suggestion(None),
            Some(Named::ArrowDown) if n > 0 => self.suggest.selected = Some(self.suggest.selected.map_or(0, |i| (i + 1) % n)),
            Some(Named::ArrowUp) if n > 0 => self.suggest.selected = Some(self.suggest.selected.map_or(n - 1, |i| (i + n - 1) % n)),
            _ => {}
        }
        Task::none()
    }

    /// Enter com uma sugestão marcada abre ela, não o texto digitado.
    pub(super) fn chosen_suggestion(&mut self) -> Option<String> {
        let s = std::mem::take(&mut self.suggest);
        self.suggest.generation = s.generation;
        s.selected.and_then(|i| s.items.get(i)).map(|p| p.to_string_lossy().into_owned())
    }
}
