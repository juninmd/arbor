//! Tela "Início" no jeito do Explorer: Acesso rápido em blocos e Recentes em lista.

use super::{computer_view, heading, list_row, thin_vertical};
use crate::app::{App, Message, Tab};
use crate::theme;
use iced::widget::{container, scrollable, space, text, Column, Row};
use iced::{Element, Fill};

pub fn body<'a>(app: &'a App, tab: &'a Tab, pane_active: bool) -> Element<'a, Message> {
    let (mut quick, mut recent) = (Row::new().spacing(6), Column::new());
    let (mut has_quick, mut has_recent) = (false, false);
    for i in 0..tab.view.len() {
        let e = tab.entry_at(i).expect("índice válido");
        if e.is_dir {
            quick = quick.push(computer_view::tile(tab, i, e, None, pane_active));
            has_quick = true;
        } else {
            recent = recent.push(list_row(app, tab, i, pane_active));
            has_recent = true;
        }
    }
    let hint = |s: &'static str| -> Element<'a, Message> { text(s).size(12).style(theme::ink3).into() };
    let mut col = Column::new().spacing(6).push(heading("Acesso rápido"));
    col = col.push(if has_quick {
        quick.wrap().vertical_spacing(6).into()
    } else {
        hint("Fixe pastas pelo botão direito ou visite algumas: as mais usadas aparecem aqui.")
    });
    col = col.push(space().height(10)).push(heading("Recentes"));
    col = col.push(if has_recent { recent.into() } else { hint("Os arquivos que você abrir pelo arbor aparecem aqui.") });
    let id = tab.id;
    scrollable(container(col).padding([8, 18]))
        .id(tab.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled { tab: id, viewport })
        .direction(thin_vertical())
        .height(Fill)
        .style(theme::scrollbar)
        .into()
}
