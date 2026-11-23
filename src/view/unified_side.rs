//! Barra lateral unificada das abas verticais (como no Firefox): abas em cima e locais embaixo, numa
//! coluna só. Com "ocultar automaticamente" ela vira uma faixa de ícones e abre por cima ao passar o mouse.

use super::{chrome_view, place_icon, sidebar_view};
use crate::app::{App, Message, RAIL_W, SIDEBAR_W};
use crate::commands::Cmd;
use crate::computer;
use crate::theme::{self, icon, Icon};
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text, tooltip, Column};
use iced::{Alignment, Color, Element, Fill, Length, Padding, Shadow, Theme, Vector};
use std::path::Path;

fn toggle(app: &App, full: bool) -> Element<'_, Message> {
    let on = app.sidebar_hide;
    let color = if on { theme::tokens(&super::theme(app)).accent } else { theme::now().ink2 };
    let b = button(container(icon(Icon::Sidebar, 15.0, color)).center(Fill))
        .width(if full { Length::Fixed(32.0) } else { Fill })
        .height(30)
        .on_press(Message::Run(Cmd::SidebarAutoHide))
        .style(theme::ghost);
    let tip = if on { "Fixar barra lateral" } else { "Ocultar barra lateral automaticamente" };
    tooltip(b, text(tip).size(12), tooltip::Position::Right).padding(8).style(theme::tooltip).into()
}

fn rail_place<'a>(app: &'a App, label: &str, path: &Path, ic: Icon, color: Color) -> Element<'a, Message> {
    let active = app.tab().path == path;
    let b = button(container(icon(ic, 16.0, color)).center_x(Fill))
        .width(Fill)
        .padding([7, 0])
        .on_press(Message::Navigate(path.to_path_buf()))
        .style(move |t: &Theme, s| theme::place(t, s, active));
    tooltip(b, text(label.to_owned()).size(12), tooltip::Position::Right).padding(8).style(theme::tooltip).into()
}

fn rail_places(app: &App) -> Column<'_, Message> {
    let hues = theme::now().hues;
    let mut col = Column::new().spacing(1).push(rail_place(app, crate::home::TITLE, &app.home, Icon::Home, hues.ice));
    for (label, path) in &app.places {
        let (ic, color) = place_icon(label);
        col = col.push(rail_place(app, label, path, ic, color));
    }
    col = col.push(rail_place(app, computer::TITLE, &app.computer, Icon::Drive, hues.ice));
    if crate::trashbin::SUPPORTED {
        col = col.push(rail_place(app, crate::trashbin::TITLE, &app.trash, Icon::Trash, hues.steel));
    }
    col
}

fn heading<'a>(label: &'a str) -> Element<'a, Message> {
    container(text(label).font(theme::UI_BOLD).size(10).style(theme::ink3)).padding(Padding::new(4.0).left(10.0)).into()
}

/// Coluna completa: cabeçalho com o botão de ocultar, abas, "Nova aba" e a árvore de locais.
fn full(app: &App) -> Element<'_, Message> {
    let head = row![heading("ABAS"), space().width(Fill), toggle(app, true)].align_y(Alignment::Center);
    let content = column![
        head,
        chrome_view::tab_rows(app, true),
        chrome_view::new_tab_button(app, true),
        container(super::separator()).padding([6, 10]),
        heading("LOCAIS"),
        sidebar_view::tree(app),
    ]
    .spacing(4);
    container(scrollable(content).height(Fill).style(theme::scrollbar)).padding([10, 6]).width(SIDEBAR_W).height(Fill).into()
}

fn rail(app: &App) -> Element<'_, Message> {
    let content = column![
        toggle(app, false),
        chrome_view::tab_rows(app, false),
        chrome_view::new_tab_button(app, false),
        container(super::separator()).padding([6, 8]),
        rail_places(app),
    ]
    .spacing(4);
    container(scrollable(content).height(Fill).style(theme::scrollbar)).padding([10, 6]).width(RAIL_W).height(Fill).into()
}

/// A barra que ocupa lugar no layout: inteira, ou a faixa de ícones (que abre ao entrar o mouse).
pub fn slot(app: &App) -> Element<'_, Message> {
    let glass = |e| container(e).height(Fill).style(|t| theme::glass(t, false));
    if !app.sidebar_hide {
        return glass(full(app)).into();
    }
    let r = glass(rail(app));
    if app.sidebar_hover {
        return r.into();
    }
    mouse_area(r).on_enter(Message::SidebarHover(true)).into()
}

/// Versão aberta por cima do conteúdo; some quando o mouse sai.
pub fn flyout(app: &App) -> Option<Element<'_, Message>> {
    let open = app.motion.sidebar(std::time::Instant::now());
    if !app.sidebar_hide || !(app.sidebar_hover || open > 0.0) {
        return None;
    }
    // abre revelando da esquerda para a direita, com o conteúdo já na largura final (sem refluxo)
    let card = container(super::chrome_view::reveal(full(app), open, SIDEBAR_W, true)).height(Fill).clip(true).style(|t: &Theme| {
        let mut s = theme::glass(t, false);
        s.background = Some(theme::tokens(t).void.into());
        s.shadow = Shadow { color: Color::from_rgba(0.0, 0.0, 0.0, 0.45), offset: Vector::new(6.0, 0.0), blur_radius: 18.0 };
        s
    });
    // só a parte visível recebe o mouse; enquanto fecha, sair já não reabre
    Some(mouse_area(card).on_exit(Message::SidebarHover(false)).into())
}
