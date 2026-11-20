//! Visão plana (Ctrl+B do Total Commander, Flat View do Directory Opus): a aba mostra todos os arquivos da
//! pasta e das subpastas numa lista só, ordenável e filtrável. Usa a busca recursiva em streaming; Ctrl+B de
//! novo volta para a pasta normal. Aqui também mora a aba "Duplicados", outra lista especial de subpastas.

use super::{App, Message};
use arbor_core::dupes::QUERY as DUPES;
use arbor_core::search::FLAT;
use arbor_core::{Sort, SortKey};
use iced::Task;

pub fn is_flat(search: &Option<(String, bool)>) -> bool {
    search.as_ref().is_some_and(|(q, _)| q == FLAT)
}

pub fn is_dupes(search: &Option<(String, bool)>) -> bool {
    search.as_ref().is_some_and(|(q, _)| q == DUPES)
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

    /// Abre uma aba com os arquivos duplicados abaixo da pasta atual, ordenada por tamanho.
    pub(super) fn find_duplicates(&mut self) -> Task<Message> {
        let pane = self.active;
        let root = self.tab().path.clone();
        if self.tab().search.is_some() || super::computer::is_virtual(&root) || arbor_core::archive::split(&root).is_some() {
            return self.notify("Procure duplicados numa pasta comum", false);
        }
        let _ = self.new_tab(pane);
        let hidden = self.show_hidden;
        let tab = self.panes[pane].tab_mut();
        let _ = Self::load_tab(tab, root.clone(), false);
        tab.search = Some((DUPES.to_string(), hidden));
        tab.sort = Sort { key: SortKey::Size, desc: true };
        let task = Self::load_tab(tab, root, true);
        tab.refreshing = false;
        Task::batch([task, self.notify("Procurando duplicados (tamanho, depois conteúdo)…", false)])
    }
}
