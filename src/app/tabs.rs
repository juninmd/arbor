//! Abas travadas, duplicar aba e fechar várias de uma vez (Directory Opus, Total Commander, Dolphin).
//!
//! Uma aba travada não sai da pasta dela: navegar a partir dela abre uma aba nova, e ela não fecha
//! por Ctrl+W nem pelos "fechar outras / à esquerda / à direita". Destravar devolve o comportamento comum.

use super::{App, MenuKind, Message};
use iced::Task;

/// Quais abas um "fechar várias" pega, a partir da aba `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Close {
    Others,
    Left,
    Right,
}

/// Índices a fechar, do maior para o menor (para remover sem deslocar os outros), pulando as travadas.
pub fn to_close(locked: &[bool], at: usize, which: Close) -> Vec<usize> {
    (0..locked.len())
        .rev()
        .filter(|&i| match which {
            Close::Others => i != at,
            Close::Left => i < at,
            Close::Right => i > at,
        })
        .filter(|&i| !locked[i])
        .collect()
}

impl App {
    /// Botão direito numa aba: ela vira a ativa e o menu age sobre ela.
    pub(super) fn tab_menu(&mut self, pane: usize, index: usize) -> Task<Message> {
        self.active = pane;
        self.panes[pane].active = index.min(self.panes[pane].tabs.len() - 1);
        self.open_menu(MenuKind::Tab, None)
    }

    pub(super) fn toggle_lock(&mut self) -> Task<Message> {
        let tab = self.tab_mut();
        tab.locked = !tab.locked;
        let text = if tab.locked { "Aba travada: navegar a partir dela abre outra aba" } else { "Aba destravada" };
        self.notify(text, false)
    }

    pub(super) fn duplicate_tab(&mut self) -> Task<Message> {
        let pane = self.active;
        let src = self.tab();
        let (path, back, forward, keep) = (src.path.clone(), src.back.clone(), src.forward.clone(), src.current().map(|e| e.path.clone()));
        let search = src.search.clone();
        let task = self.new_tab_at(pane, path.clone());
        let tab = self.tab_mut();
        tab.back = back;
        tab.forward = forward;
        tab.focus_on_load = keep.into_iter().collect();
        if search.is_some() {
            // Busca e visão plana: refaz a mesma consulta na cópia.
            tab.search = search;
            let again = Self::load_tab(tab, path, true);
            tab.refreshing = false;
            return Task::batch([task, again]);
        }
        task
    }

    pub(super) fn close_many(&mut self, which: Close) -> Task<Message> {
        let pane = self.active;
        let p = &self.panes[pane];
        let locked: Vec<bool> = p.tabs.iter().map(|t| t.locked).collect();
        for i in to_close(&locked, p.active, which) {
            let _ = self.close_tab(pane, i);
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::{to_close, Close};

    #[test]
    fn fechar_varias_pula_as_travadas_e_vai_do_fim_para_o_comeco() {
        let locked = [false, true, false, false, false];
        assert_eq!(to_close(&locked, 2, Close::Others), [4, 3, 0]);
        assert_eq!(to_close(&locked, 2, Close::Left), [0]);
        assert_eq!(to_close(&locked, 2, Close::Right), [4, 3]);
        assert!(to_close(&locked, 4, Close::Right).is_empty());
    }
}
