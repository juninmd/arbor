//! Menus flutuantes: botão direito num item, na área vazia e na barra lateral, e os botões
//! "Classificar" e "Visualizar" da barra de comandos. Itens são `Cmd`s; `None` é um separador.

use super::{App, Message, Overlay};
use crate::commands::Cmd;
use crate::pointer;
use crate::{computer, launch};
use iced::keyboard::key::Named;
use iced::{Point, Task};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Item {
        dir: bool,
    },
    Background,
    Sidebar,
    Sort,
    View,
    New,
    /// Botão direito no cabeçalho da lista.
    Columns,
    /// Botão de tema na barra de título.
    Themes,
    /// Botão direito numa aba.
    Tab,
}

pub struct Menu {
    pub at: Point,
    pub items: Vec<Option<Cmd>>,
    /// Pasta ou item sobre o qual o menu foi aberto (fixar, abrir em nova aba).
    pub target: Option<PathBuf>,
    pub selected: Option<usize>,
}

pub fn items(kind: MenuKind, here_virtual: bool) -> Vec<Option<Cmd>> {
    use Cmd::*;
    let v: Vec<Option<Cmd>> = match kind {
        MenuKind::Item { dir } => {
            let mut v = vec![Some(Open)];
            if dir {
                v.push(Some(OpenInNewTab));
            }
            v.extend([
                Some(OpenDefault),
                Some(OpenWith),
                Some(RunAsAdmin),
                Some(Preview),
                None,
                Some(Cut),
                Some(Copy),
                Some(Rename),
                Some(Trash),
                Some(DeleteNow),
                None,
                Some(CopyPath),
                Some(CreateShortcut),
                Some(AddToShelf),
            ]);
            if dir {
                v.push(Some(Pin));
            }
            if dir {
                v.push(Some(Terminal));
            }
            v.extend([Some(Reveal), None, Some(CopyToOther), Some(MoveToOther), None, Some(Eject), Some(Properties)]);
            v
        }
        MenuKind::Background => vec![
            Some(ToggleGrid),
            Some(ToggleHidden),
            None,
            Some(SortName),
            Some(SortSize),
            Some(SortModified),
            Some(SortKind),
            Some(GroupKind),
            Some(GroupModified),
            Some(GroupNone),
            None,
            Some(Refresh),
            Some(Paste),
            Some(Undo),
            Some(SelectAll),
            Some(InvertSelection),
            Some(SelectMask),
            None,
            Some(NewFolder),
            Some(NewTextFile),
            Some(NewZip),
            None,
            Some(Terminal),
            Some(CopyPath),
            Some(CopyListing),
            Some(Pin),
            Some(MapDrive),
            None,
            Some(Details),
        ],
        MenuKind::Sidebar => vec![Some(OpenInNewTab), Some(Pin)],
        MenuKind::Tab => vec![
            Some(DuplicateTab),
            Some(LockTab),
            None,
            Some(CloseTab),
            Some(CloseOtherTabs),
            Some(CloseTabsLeft),
            Some(CloseTabsRight),
            None,
            Some(ReopenTab),
        ],
        MenuKind::New => vec![Some(NewFolder), Some(NewTextFile), Some(NewZip)],
        MenuKind::Sort => vec![
            Some(SortName),
            Some(SortSize),
            Some(SortModified),
            Some(SortKind),
            None,
            Some(GroupNone),
            Some(GroupKind),
            Some(GroupModified),
            Some(GroupSize),
        ],
        MenuKind::View => vec![
            Some(ToggleGrid),
            Some(ZoomIn),
            Some(ZoomOut),
            Some(ToggleHidden),
            Some(ToggleExtensions),
            None,
            Some(ColKind),
            Some(ColSize),
            Some(ColModified),
            None,
            Some(Details),
            Some(ToggleDual),
            Some(VerticalTabs),
            Some(ToggleTheme),
        ],
        MenuKind::Themes => {
            let mut v: Vec<Option<Cmd>> = crate::palettes::ALL.iter().map(|p| Some(p.cmd())).collect();
            v.extend([None, Some(VerticalTabs)]);
            v
        }
        MenuKind::Columns => vec![Some(ColKind), Some(ColSize), Some(ColModified)],
    };
    // Em "Este Computador" não há pasta real para fixar, colar ou abrir terminal.
    let hide = |c: &Cmd| {
        // Mapear unidade é do Windows e, como no Explorer, aparece em "Este Computador".
        (*c == Cmd::MapDrive && (!cfg!(windows) || !here_virtual))
            || here_virtual && matches!(kind, MenuKind::Background) && matches!(c, Cmd::Pin | Cmd::Terminal | Cmd::CopyPath)
    };
    let mut out: Vec<Option<Cmd>> = Vec::new();
    for c in v.into_iter().filter(|c| !c.as_ref().is_some_and(hide)) {
        // Sem separador no começo, no fim ou repetido depois de esconder itens.
        if c.is_none() && out.last().is_none_or(Option::is_none) {
            continue;
        }
        out.push(c);
    }
    if out.last() == Some(&None) {
        out.pop();
    }
    out
}

impl App {
    /// Comandos que não fariam nada agora aparecem apagados, como no Explorer.
    /// Itens liga/desliga do menu que estão ligados (mostram ✓).
    pub fn checked(&self, cmd: Cmd) -> bool {
        if let Some(p) = crate::palettes::Palette::of_cmd(cmd) {
            return self.palette == p;
        }
        if cmd == Cmd::VerticalTabs {
            return self.vertical_tabs;
        }
        if let Some(by) = super::group_of(cmd) {
            return self.tab().group == by;
        }
        super::column_of(cmd).is_some_and(|c| self.columns.shown(c))
    }

    pub fn enabled(&self, cmd: Cmd) -> bool {
        let here = computer::is_virtual(&self.tab().path);
        let in_trash = crate::trashbin::is(&self.tab().path);
        let has_target = self.tab().current().is_some();
        match cmd {
            // A área do sistema pode ter arquivos de outro app; lê-la a cada quadro custaria caro.
            Cmd::Paste => !here,
            Cmd::MapDrive => cfg!(windows),
            Cmd::OpenRemote => self.tab().git.as_ref().is_some_and(|g| g.remote.is_some()),
            Cmd::Undo => !self.undo.is_empty(),
            Cmd::Restore | Cmd::Purge => in_trash && has_target,
            Cmd::Eject => self.tab().current().is_some_and(|e| self.drives.iter().any(|d| d.path == e.path && d.removable())),
            Cmd::DeleteNow => has_target && (!here || in_trash),
            Cmd::Copy | Cmd::CopyToOther | Cmd::Open | Cmd::OpenDefault | Cmd::OpenWith | Cmd::Preview if in_trash => false,
            Cmd::Cut | Cmd::Rename | Cmd::Trash | Cmd::MoveToOther => has_target && !here,
            Cmd::NewFolder | Cmd::NewTextFile | Cmd::NewZip => !here,
            Cmd::CreateShortcut => has_target && !here,
            Cmd::RunAsAdmin => cfg!(windows) && self.tab().current().is_some_and(|e| launch::elevatable(&e.path)),
            Cmd::Copy | Cmd::CopyToOther | Cmd::Open | Cmd::OpenDefault | Cmd::Preview => has_target,
            Cmd::OpenWith => self.tab().current().is_some_and(|e| !e.is_dir && !computer::is_virtual(&e.path)),
            _ => true,
        }
    }

    pub(super) fn open_menu(&mut self, kind: MenuKind, target: Option<PathBuf>) -> Task<Message> {
        let here = computer::is_virtual(&self.tab().path);
        let items = if crate::trashbin::is(&self.tab().path) {
            match kind {
                MenuKind::Item { .. } => vec![Some(Cmd::Restore), Some(Cmd::Purge), None, Some(Cmd::Properties)],
                MenuKind::Background => vec![Some(Cmd::EmptyTrash), Some(Cmd::Refresh), Some(Cmd::SelectAll)],
                k => items(k, here),
            }
        } else {
            items(kind, here)
        };
        let items = items.into_iter().filter(|c| *c != Some(Cmd::Eject) || self.enabled(Cmd::Eject)).collect();
        self.overlay = Overlay::Menu(Menu { at: pointer::get(), items, target, selected: None });
        Task::none()
    }

    pub(super) fn run_from_menu(&mut self, cmd: Cmd) -> Task<Message> {
        let target = match std::mem::replace(&mut self.overlay, Overlay::None) {
            Overlay::Menu(m) => m.target,
            _ => None,
        };
        if !self.enabled(cmd) {
            return Task::none();
        }
        self.menu_target = target;
        let task = self.run(cmd);
        self.menu_target = None;
        task
    }

    /// Tecla Menu ou Shift+F10: menu do item sob o cursor (ou da pasta, se estiver vazia).
    pub(super) fn keyboard_menu(&mut self) -> Task<Message> {
        match self.tab().current().map(|e| (e.is_dir, e.path.clone())) {
            Some((dir, path)) => self.open_menu(MenuKind::Item { dir }, Some(path)),
            None => {
                let path = self.tab().path.clone();
                self.open_menu(MenuKind::Background, Some(path))
            }
        }
    }

    /// Setas e Enter navegam no menu aberto; Esc fecha.
    pub(super) fn menu_key(&mut self, named: Option<Named>) -> Task<Message> {
        let enabled: Vec<bool> = match &self.overlay {
            Overlay::Menu(m) => m.items.iter().map(|c| c.is_some_and(|c| self.enabled(c))).collect(),
            _ => return Task::none(),
        };
        let Overlay::Menu(m) = &mut self.overlay else { return Task::none() };
        let n = m.items.len();
        let step = |from: Option<usize>, down: bool| -> Option<usize> {
            let mut i = from.unwrap_or(if down { n - 1 } else { 0 });
            for _ in 0..n {
                i = if down { (i + 1) % n } else { (i + n - 1) % n };
                if enabled[i] {
                    return Some(i);
                }
            }
            None
        };
        match named {
            Some(Named::Escape) => self.overlay = Overlay::None,
            Some(Named::ArrowDown) => m.selected = step(m.selected, true),
            Some(Named::ArrowUp) => m.selected = step(m.selected, false),
            Some(Named::Enter) => {
                if let Some(cmd) = m.selected.and_then(|i| m.items[i]) {
                    return self.run_from_menu(cmd);
                }
            }
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_de_arquivo_nao_oferece_fixar_nem_nova_aba() {
        let file = items(MenuKind::Item { dir: false }, false);
        assert!(!file.contains(&Some(Cmd::Pin)) && !file.contains(&Some(Cmd::OpenInNewTab)));
        let dir = items(MenuKind::Item { dir: true }, false);
        assert!(dir.contains(&Some(Cmd::Pin)) && dir.contains(&Some(Cmd::OpenInNewTab)));
    }

    #[test]
    fn este_computador_esconde_acoes_de_pasta_real() {
        let bg = items(MenuKind::Background, true);
        assert!(!bg.contains(&Some(Cmd::Pin)) && !bg.contains(&Some(Cmd::Terminal)));
        assert!(items(MenuKind::Background, false).contains(&Some(Cmd::Pin)));
        assert_ne!(bg.last(), Some(&None), "sem separador sobrando no fim");
    }
}
