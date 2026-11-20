//! Visão plana (Ctrl+B do Total Commander, Flat View do Directory Opus): a aba mostra todos os arquivos da
//! pasta e das subpastas numa lista só, ordenável e filtrável. Usa a busca recursiva em streaming; Ctrl+B de
//! novo volta para a pasta normal.

use super::{App, Message};
use arbor_core::search::FLAT;
use iced::Task;

pub fn is_flat(search: &Option<(String, bool)>) -> bool {
    search.as_ref().is_some_and(|(q, _)| q == FLAT)
}

impl App {
    pub(super) fn toggle_flat(&mut self) -> Task<Message> {
        let pane = self.active;
        let hidden = self.show_hidden;
        let tab = self.panes[pane].tab_mut();
        let path = tab.path.clone();
        if is_flat(&tab.search) {
            tab.search = None;
            return Self::load_tab(tab, path, false);
        }
        if tab.search.is_some() || super::computer::is_virtual(&path) || arbor_core::archive::split(&path).is_some() {
            return self.notify("A visão plana funciona numa pasta comum", false);
        }
        let tab = self.panes[pane].tab_mut();
        // Zera a lista e roda a busca no lugar dela, mostrando cada lote, como a busca recursiva.
        let _ = Self::load_tab(tab, path.clone(), false);
        tab.search = Some((FLAT.to_string(), hidden));
        let task = Self::load_tab(tab, path, true);
        tab.refreshing = false;
        task
    }
}
