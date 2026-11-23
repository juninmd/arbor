//! Moldura da janela: barra de título própria com as abas (estilo Explorer do Windows 11), abas
//! verticais, bordas de redimensionar e o cartão de avisos que substituiu a doca de baixo.

use super::{kbd, shorten};
use crate::app::{App, MenuKind, Message, Pane, WindowAction};
use crate::commands::Cmd;
use crate::theme::{self, icon, Icon};
use iced::mouse::Interaction;
use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text, tooltip, Column, Row};
use iced::window::Direction;
use iced::{Alignment, Element, Fill, Length, Padding, Theme};
use std::time::Instant;

/// Faz o item crescer até o tamanho inteiro: `frac` de 0 a 1 revela o conteúdo sem refluxo dele.
pub fn reveal<'a>(e: impl Into<Element<'a, Message>>, frac: f32, full: f32, horizontal: bool) -> Element<'a, Message> {
    if frac >= 1.0 {
        return e.into();
    }
    let c = container(e).clip(true);
    if horizontal {
        c.width((full * frac).max(0.1)).into()
    } else {
        c.height((full * frac).max(0.1)).into()
    }
}

pub const TITLE_H: f32 = 40.0;
const TAB_W: f32 = 210.0;
const EDGE: f32 = 5.0;

fn tab_icon<'a>(p: &Pane, i: usize) -> Element<'a, Message> {
    let path = &p.tabs[i].path;
    // travada mostra o cadeado; dentro de um repositório git, o ramo em coral como a pílula da barra
    if !p.tabs[i].locked && p.tabs[i].git.is_some() {
        return icon(Icon::Branch, 15.0, theme::CORAL);
    }
    let ic = if p.tabs[i].locked {
        Icon::Lock
    } else if crate::computer::is(path) {
        Icon::DriveSystem
    } else if crate::home::is(path) {
        Icon::Home
    } else if crate::trashbin::is(path) {
        Icon::Trash
    } else {
        Icon::Folder
    };
    icon(ic, 16.0, theme::now().hues.ice)
}

/// Abas na barra de título: a ativa tem a cor da faixa de baixo e se funde com ela.
fn title_tabs(app: &App) -> Element<'_, Message> {
    let pane = app.active;
    let p = &app.panes[pane];
    let mut r = Row::new().align_y(Alignment::End);
    for (i, tab) in p.tabs.iter().enumerate() {
        let selected = i == p.active;
        let close = button(icon(Icon::WinClose, 12.0, theme::now().ink2))
            .padding(6)
            .on_press(Message::CloseTabAt { pane, index: i })
            .style(theme::ghost);
        let label = row![tab_icon(p, i), text(shorten(&tab.title(), 26)).size(12.5).width(Fill).wrapping(Wrapping::None), close]
            .spacing(9)
            .align_y(Alignment::Center);
        let b = button(label)
            .width(TAB_W)
            .height(34)
            .padding(Padding::from([0, 4]).left(12.0))
            .on_press(Message::SelectTab { pane, index: i })
            .style(move |t: &Theme, s| theme::win_tab(t, s, selected));
        let item =
            mouse_area(b).on_middle_press(Message::CloseTabAt { pane, index: i }).on_right_press(Message::TabMenu { pane, index: i });
        r = r.push(reveal(item, app.motion.tab_open(tab.id, Instant::now()), TAB_W, true));
        // filete entre abas inativas, como no Explorer
        let next_selected = i + 1 == p.active;
        if !selected && !next_selected {
            r = r.push(container(space().width(1).height(16)).padding(Padding::new(0.0).bottom(9.0)).style(|t: &Theme| container::Style {
                background: Some(theme::alpha(theme::tokens(t).ink, 0.14).into()),
                ..Default::default()
            }));
        }
    }
    let add = button(container(icon(Icon::Plus, 16.0, theme::now().ink2)).center(Fill))
        .width(32)
        .height(30)
        .on_press(Message::NewTabIn(pane))
        .style(theme::ghost);
    r = r.push(container(add).padding(Padding::new(0.0).left(6.0).bottom(3.0)));
    scrollable(r).direction(super::thin_horizontal()).style(theme::scrollbar).into()
}

fn chrome_button<'a>(ic: Icon, msg: Message, tip: &'a str) -> Element<'a, Message> {
    let b = button(container(icon(ic, 15.0, theme::now().ink2)).center(Fill)).width(38).height(32).on_press(msg).style(theme::ghost);
    tooltip(b, text(tip).size(12), tooltip::Position::Bottom).padding(8).style(theme::tooltip).into()
}

fn window_button<'a>(ic: Icon, action: WindowAction, close: bool) -> Element<'a, Message> {
    button(container(icon(ic, 11.0, theme::now().ink2)).center(Fill))
        .width(46)
        .height(TITLE_H)
        .on_press(Message::Window(action))
        .style(move |t: &Theme, s| theme::window_button(t, s, close))
        .into()
}

/// Abas verticais e um painel só: endereço e comandos de navegação dividem a barra de título.
pub fn merged(app: &App) -> bool {
    app.vertical_tabs && !app.dual
}

/// Espaço que logo, busca e botões da janela tiram do endereço quando ele mora na barra de título.
pub const MERGED_CHROME_W: f32 = 230.0;

pub fn titlebar(app: &App) -> Element<'_, Message> {
    // o logo abre o menu de tema e abas; arrastar a janela fica com o vão livre
    let logo = tooltip(
        button(container(icon(Icon::Logo, 20.0, theme::now().hues.ice)).center_x(46).center_y(TITLE_H))
            .padding(0)
            .on_press(Message::OpenMenu(MenuKind::Themes))
            .style(theme::ghost),
        text("Tema e abas").size(12),
        tooltip::Position::Bottom,
    )
    .padding(8)
    .style(theme::tooltip);
    // abas verticais com um painel só: a barra de endereço sobe para a barra de título e o espaço de cima volta para os arquivos
    let tabs: Element<_> = if merged(app) {
        super::toolbar(app, app.tab())
    } else if app.vertical_tabs || app.dual {
        container(text(app.title()).size(12.5).style(theme::ink2).wrapping(Wrapping::None)).center_y(TITLE_H).into()
    } else {
        container(title_tabs(app)).height(TITLE_H).align_bottom(TITLE_H).into()
    };
    // o vão livre arrasta a janela; duplo clique maximiza, como na barra do sistema
    let grip_w = if merged(app) { Length::Fixed(28.0) } else { Fill };
    let grip = mouse_area(container(space()).width(grip_w).height(TITLE_H))
        .on_press(Message::Window(WindowAction::Drag))
        .on_double_click(Message::Window(WindowAction::ToggleMaximize));
    let tabs_w = if merged(app) { Length::Fill } else { Length::Shrink };
    let mut bar = row![logo, container(tabs).width(tabs_w).max_width(app.window.width - 330.0), grip]
        .align_y(Alignment::Center)
        .push(chrome_button(Icon::Search, Message::Run(Cmd::Palette), "Comandos  Ctrl+Shift+P"))
        .push(space().width(6));
    if crate::window_chrome::custom() {
        bar = bar
            .push(window_button(Icon::WinMin, WindowAction::Minimize, false))
            .push(window_button(Icon::WinMax, WindowAction::ToggleMaximize, false))
            .push(window_button(Icon::WinClose, WindowAction::Close, true));
    }
    container(bar).height(TITLE_H).width(Fill).style(theme::titlebar).into()
}

/// Abas verticais: uma linha por aba do painel ativo, como nos navegadores; `full` mostra o título e o fechar.
pub fn tab_rows(app: &App, full: bool) -> Column<'_, Message> {
    let pane = app.active;
    let p = &app.panes[pane];
    let mut col = Column::new().spacing(2);
    for (i, tab) in p.tabs.iter().enumerate() {
        let selected = i == p.active;
        let b: Element<_> = if full {
            let close = button(icon(Icon::WinClose, 11.0, theme::now().ink3))
                .padding(5)
                .on_press(Message::CloseTabAt { pane, index: i })
                .style(theme::ghost);
            let label = row![tab_icon(p, i), text(tab.title()).size(12.5).width(Fill).wrapping(Wrapping::None), close]
                .spacing(9)
                .align_y(Alignment::Center);
            button(label)
                .width(Fill)
                .padding(Padding::from([4, 4]).left(10.0))
                .on_press(Message::SelectTab { pane, index: i })
                .style(move |t: &Theme, s| theme::side_tab(t, s, selected))
                .into()
        } else {
            let b = button(container(tab_icon(p, i)).center_x(Fill))
                .width(Fill)
                .padding([7, 0])
                .on_press(Message::SelectTab { pane, index: i })
                .style(move |t: &Theme, s| theme::side_tab(t, s, selected));
            tooltip(b, text(tab.title()).size(12), tooltip::Position::Right).padding(8).style(theme::tooltip).into()
        };
        let item =
            mouse_area(b).on_middle_press(Message::CloseTabAt { pane, index: i }).on_right_press(Message::TabMenu { pane, index: i });
        col = col.push(reveal(item, app.motion.tab_open(tab.id, Instant::now()), 32.0, false));
    }
    col
}

/// "Nova aba" da barra lateral (só o ícone na faixa recolhida).
pub fn new_tab_button(app: &App, full: bool) -> Element<'_, Message> {
    let pane = app.active;
    if !full {
        let b = button(container(icon(Icon::Plus, 14.0, theme::now().ink2)).center_x(Fill))
            .width(Fill)
            .padding([7, 0])
            .on_press(Message::NewTabIn(pane))
            .style(theme::ghost);
        return tooltip(b, text("Nova aba").size(12), tooltip::Position::Right).padding(8).style(theme::tooltip).into();
    }
    button(
        row![icon(Icon::Plus, 14.0, theme::now().ink2), text("Nova aba").size(12.5), space().width(Fill), kbd(Cmd::NewTab.shortcut())]
            .spacing(9)
            .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([7, 10])
    .on_press(Message::NewTabIn(pane))
    .style(theme::ghost)
    .into()
}

/// Contorno de 1 px da janela sem moldura: separa o arbor do que está atrás (outra janela escura,
/// o papel de parede). Fica por cima de tudo e não recebe cliques; maximizada, some.
pub fn window_outline<'a>() -> Element<'a, Message> {
    container(space().width(Fill).height(Fill))
        .style(|t: &Theme| container::Style { border: theme::window_border(t), ..Default::default() })
        .into()
}

/// Bordas finas por cima de tudo: puxar redimensiona a janela (sem moldura do sistema não há outra borda).
pub fn resize_edges<'a>() -> Element<'a, Message> {
    let edge = |dir: Direction, w: Length, h: Length, cursor: Interaction| -> Element<'a, Message> {
        mouse_area(container(space()).width(w).height(h)).on_press(Message::Window(WindowAction::Resize(dir))).interaction(cursor).into()
    };
    let (e, f) = (Length::Fixed(EDGE), Fill);
    column![
        row![
            edge(Direction::NorthWest, e, e, Interaction::ResizingDiagonallyDown),
            edge(Direction::North, f, e, Interaction::ResizingVertically),
            edge(Direction::NorthEast, e, e, Interaction::ResizingDiagonallyUp),
        ],
        row![
            edge(Direction::West, e, f, Interaction::ResizingHorizontally),
            space().width(Fill),
            edge(Direction::East, e, f, Interaction::ResizingHorizontally),
        ]
        .height(Fill),
        row![
            edge(Direction::SouthWest, e, e, Interaction::ResizingDiagonallyUp),
            edge(Direction::South, f, e, Interaction::ResizingVertically),
            edge(Direction::SouthEast, e, e, Interaction::ResizingDiagonallyDown),
        ],
    ]
    .into()
}
