//! Barra de comandos (Novo, recortar, copiar, colar, renomear, excluir, Classificar, Visualizar, …) e o
//! menu flutuante usado pelo botão direito e pelos botões da barra.

use super::{separator, separator_v};
use crate::app::{App, Jump, Menu, MenuKind, Message};
use crate::commands::Cmd;
use crate::theme::{self, icon, Icon};
use iced::widget::text::Wrapping;
use iced::widget::{button, container, mouse_area, opaque, row, scrollable, space, stack, text, tooltip, Column};
use iced::{Alignment, Element, Fill, Padding, Theme};

const MENU_W: f32 = 300.0;
const ITEM_H: f32 = 30.0;
const SEP_H: f32 = 9.0;

pub fn icon_of(cmd: Cmd) -> Option<Icon> {
    Some(match cmd {
        Cmd::NewFolder => Icon::Folder,
        Cmd::NewTextFile => Icon::Doc,
        Cmd::NewZip => Icon::Archive,
        Cmd::CreateShortcut => Icon::Right,
        Cmd::Cut => Icon::Cut,
        Cmd::Copy | Cmd::CopyPath => Icon::Copy,
        Cmd::Paste => Icon::Paste,
        Cmd::Rename => Icon::Rename,
        Cmd::Trash | Cmd::DeleteNow | Cmd::Purge | Cmd::EmptyTrash => Icon::Trash,
        Cmd::Restore => Icon::Up,
        Cmd::SortName | Cmd::SortSize | Cmd::SortModified | Cmd::SortKind => Icon::Sort,
        Cmd::GroupNone | Cmd::GroupKind | Cmd::GroupModified | Cmd::GroupSize => Icon::Sort,
        Cmd::ToggleGrid => Icon::Grid,
        Cmd::ToggleHidden | Cmd::Preview => Icon::Eye,
        Cmd::Details | Cmd::Properties => Icon::Doc,
        Cmd::Pin => Icon::Pin,
        Cmd::Open | Cmd::OpenInNewTab => Icon::Folder,
        Cmd::Up => Icon::Up,
        Cmd::Refresh => Icon::Bolt,
        Cmd::Palette => Icon::Search,
        _ => return None,
    })
}

pub fn bar(app: &App) -> Element<'_, Message> {
    if crate::trashbin::is(&app.tab().path) {
        return trash_bar(app);
    }
    let action = |cmd: Cmd| -> Element<'_, Message> {
        let ic = icon_of(cmd).expect("ícone do comando");
        let on = app.enabled(cmd);
        let color = if on { theme::DARK.ink2 } else { theme::alpha(theme::DARK.ink3, 0.3) };
        let b = button(icon(ic, 15.0, color)).padding([5, 8]).on_press_maybe(on.then_some(Message::Run(cmd))).style(theme::ghost);
        let sc = cmd.shortcut();
        let tip = if sc.is_empty() { cmd.label().to_string() } else { format!("{}  {sc}", cmd.label()) };
        tooltip(b, text(tip).size(12), tooltip::Position::Bottom).padding(8).style(theme::tooltip).into()
    };
    let labeled = |ic: Icon, label: &'static str, msg: Option<Message>| -> Element<'_, Message> {
        let content = row![icon(ic, 14.0, theme::DARK.ink2), text(label).size(12.5)].spacing(6).align_y(Alignment::Center);
        button(content).padding([5, 9]).on_press_maybe(msg).style(theme::ghost).into()
    };
    let new = labeled(Icon::Plus, "Novo", app.enabled(Cmd::NewFolder).then_some(Message::OpenMenu(MenuKind::New)));
    row![
        new,
        separator_v(),
        action(Cmd::Cut),
        action(Cmd::Copy),
        action(Cmd::Paste),
        action(Cmd::Rename),
        action(Cmd::Trash),
        separator_v(),
        labeled(Icon::Sort, "Classificar", Some(Message::OpenMenu(MenuKind::Sort))),
        labeled(Icon::Eye, "Visualizar", Some(Message::OpenMenu(MenuKind::View))),
        separator_v(),
        button(icon(Icon::More, 15.0, theme::DARK.ink2))
            .padding([5, 8])
            .on_press(Message::OpenMenu(MenuKind::Background))
            .style(theme::ghost),
        space().width(Fill),
        labeled(Icon::Doc, "Detalhes", Some(Message::Run(Cmd::Details))),
    ]
    .spacing(2)
    .align_y(Alignment::Center)
    .padding(Padding::from([0, 10]).bottom(4.0))
    .into()
}

pub fn menu<'a>(app: &'a App, m: &'a Menu) -> Element<'a, Message> {
    let mut col = Column::new().spacing(0);
    for (i, item) in m.items.iter().enumerate() {
        let Some(cmd) = *item else {
            col = col.push(container(separator()).padding([4, 6]));
            continue;
        };
        let on = app.enabled(cmd);
        let hot = m.selected == Some(i);
        let ic: Element<_> = match icon_of(cmd) {
            None if app.checked(cmd) => text("✓").size(13).width(14).style(theme::accent).into(),
            Some(ic) => icon(ic, 14.0, if on { theme::DARK.ink2 } else { theme::alpha(theme::DARK.ink3, 0.5) }),
            None => space().width(14).into(),
        };
        let label = text(cmd.label()).size(12.5).width(Fill).wrapping(Wrapping::None).style(move |t: &Theme| {
            if on {
                text::Style { color: Some(theme::tokens(t).ink) }
            } else {
                theme::ink3(t)
            }
        });
        let sc = text(cmd.shortcut()).font(theme::MONO).size(10.5).style(theme::ink3);
        let content = row![ic, label, sc].spacing(10).align_y(Alignment::Center);
        let b = button(content)
            .height(ITEM_H)
            .width(Fill)
            .padding([6, 10])
            .on_press_maybe(on.then_some(Message::MenuRun(cmd)))
            .style(move |t: &Theme, s| theme::place(t, if hot { button::Status::Hovered } else { s }, false));
        col = col.push(mouse_area(b).on_enter(Message::MenuHover(i)));
    }
    let seps = m.items.iter().filter(|c| c.is_none()).count() as f32;
    let h = (m.items.len() as f32 - seps) * ITEM_H + seps * SEP_H + 12.0;
    floating(app, col.into(), m.at, h)
}

/// Cartão flutuante na posição do mouse, sem passar da borda da janela; clique fora fecha.
fn floating<'a>(app: &App, content: Element<'a, Message>, at: iced::Point, h: f32) -> Element<'a, Message> {
    let card = container(content).width(MENU_W).padding(5).style(theme::tooltip);
    let x = at.x.min(app.window.width - MENU_W - 8.0).max(0.0);
    let y = if at.y + h > app.window.height - 8.0 { (at.y - h).max(0.0) } else { at.y };
    let catcher =
        mouse_area(container(space()).width(Fill).height(Fill)).on_press(Message::CloseOverlay).on_right_press(Message::CloseOverlay);
    stack![catcher, container(opaque(card)).padding(Padding::new(0.0).left(x).top(y))].into()
}

/// Até onde a lista de salto cresce antes de rolar.
const JUMP_ROWS: usize = 14;

/// Subpastas do "›" ou histórico de Voltar/Avançar.
pub fn jump<'a>(app: &'a App, j: &'a Jump) -> Element<'a, Message> {
    let Some(items) = &j.items else {
        return floating(app, container(text("Lendo…").size(12.5).style(theme::ink3)).padding([6, 10]).into(), j.at, ITEM_H + 12.0);
    };
    if items.is_empty() {
        return floating(app, container(text("Sem subpastas").size(12.5).style(theme::ink3)).padding([6, 10]).into(), j.at, ITEM_H + 12.0);
    }
    let current = &app.tab().path;
    let mut col = Column::new().spacing(0);
    for (i, (label, path)) in items.iter().enumerate() {
        let hot = j.selected == Some(i);
        let here = path == current;
        let content = row![icon(Icon::Folder, 14.0, theme::ICE), text(label.as_str()).size(12.5).width(Fill).wrapping(Wrapping::None)]
            .spacing(10)
            .align_y(Alignment::Center);
        let b = button(content)
            .height(ITEM_H)
            .width(Fill)
            .padding([6, 10])
            .on_press(Message::JumpPick(i))
            .style(move |t: &Theme, s| theme::place(t, if hot { button::Status::Hovered } else { s }, here));
        col = col.push(mouse_area(b).on_enter(Message::JumpHover(i)));
    }
    let rows = items.len().min(JUMP_ROWS) as f32;
    let list = scrollable(col).height(rows * ITEM_H).style(theme::scrollbar);
    floating(app, list.into(), j.at, rows * ITEM_H + 12.0)
}

/// Na Lixeira a barra troca para o que faz sentido ali, como no Explorer.
fn trash_bar(app: &App) -> Element<'_, Message> {
    let b = |ic: Icon, cmd: Cmd| -> Element<'_, Message> {
        let content = row![icon(ic, 14.0, theme::DARK.ink2), text(cmd.label()).size(12.5)].spacing(6).align_y(Alignment::Center);
        button(content).padding([5, 9]).on_press_maybe(app.enabled(cmd).then_some(Message::Run(cmd))).style(theme::ghost).into()
    };
    row![b(Icon::Up, Cmd::Restore), b(Icon::Trash, Cmd::Purge), separator_v(), b(Icon::Trash, Cmd::EmptyTrash)]
        .spacing(2)
        .align_y(Alignment::Center)
        .padding(Padding::from([0, 10]).bottom(4.0))
        .into()
}
