//! Barra lateral: Este Computador, locais, fixados e discos, com a árvore de pastas que se expande
//! pela seta (como o painel de navegação do Explorer).

use super::{engine, heading, kbd, place_icon};
use crate::app::{App, Message, SIDEBAR_W};
use crate::commands::Cmd;
use crate::computer;
use crate::theme::{self, icon, Icon};
use crate::update;
use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text, Column};
use iced::{Alignment, Color, Element, Fill, Padding, Theme};
use std::path::{Path, PathBuf};

const INDENT: f32 = 14.0;
/// Limite de profundidade desenhada: protege contra laços de links simbólicos abertos à mão.
const MAX_DEPTH: usize = 24;

fn node<'a>(app: &'a App, label: String, path: &Path, ic: Icon, color: Color, depth: usize) -> Element<'a, Message> {
    let active = app.tab().path == path;
    let open = app.tree.open.contains(path);
    let chevron: Element<_> = if computer::is_virtual(path) {
        space().width(16).into()
    } else {
        button(icon(if open { Icon::Down } else { Icon::Right }, 12.0, theme::DARK.ink3))
            .padding(2)
            .on_press(Message::TreeToggle(path.to_path_buf()))
            .style(theme::ghost)
            .into()
    };
    let content = row![icon(ic, 16.0, color), text(label).size(13).wrapping(Wrapping::None)].spacing(10).align_y(Alignment::Center);
    let b = button(content)
        .on_press(Message::Navigate(path.to_path_buf()))
        .width(Fill)
        .padding([7, 8])
        .style(move |t: &Theme, s| theme::place(t, s, active));
    let line = row![space().width(depth as f32 * INDENT), chevron, b].spacing(2).align_y(Alignment::Center);
    let p = path.to_path_buf();
    let area = mouse_area(line).on_right_press(Message::SidebarMenu(p.clone()));
    if app.dragging().is_some() {
        area.on_enter(Message::DragOver(Some(p.clone()))).on_exit(Message::DragLeave { from: p, back: None }).into()
    } else {
        area.into()
    }
}

/// O nó e, se aberto, suas subpastas (recursivo).
fn branch<'a>(
    mut col: Column<'a, Message>,
    app: &'a App,
    label: String,
    path: &'a PathBuf,
    ic: Icon,
    color: Color,
    depth: usize,
) -> Column<'a, Message> {
    col = col.push(node(app, label, path, ic, color, depth));
    if depth >= MAX_DEPTH || !app.tree.open.contains(path) {
        return col;
    }
    match app.tree.children.get(path) {
        None => col.push(container(text("…").size(12).style(theme::ink3)).padding(Padding::new(2.0).left((depth + 2) as f32 * INDENT))),
        Some(children) => {
            for child in children {
                let name = child.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                col = branch(col, app, name, child, Icon::Folder, theme::ICE, depth + 1);
            }
            col
        }
    }
}

pub fn sidebar(app: &App) -> Element<'_, Message> {
    let brand = row![
        icon(Icon::Logo, 26.0, theme::ICE),
        text("arbor").font(theme::DISPLAY).size(19),
        container(text(update::CURRENT).font(theme::MONO).size(10).style(theme::ink3)).padding([1, 6]).style(theme::kbd),
    ]
    .spacing(9)
    .align_y(Alignment::Center);

    let mut col = Column::new().spacing(2).push(node(app, computer::TITLE.into(), &app.computer, Icon::Drive, theme::ICE, 0));
    if crate::trashbin::SUPPORTED {
        col = col.push(node(app, crate::trashbin::TITLE.into(), &app.trash, Icon::Trash, theme::STEEL, 0));
    }
    col = col.push(space().height(12));
    col = col.push(heading("Locais"));
    for (label, path) in &app.places {
        let (ic, color) = place_icon(label);
        col = branch(col, app, label.clone(), path, ic, color, 0);
    }
    if !app.pins.is_empty() {
        col = col.push(space().height(12)).push(heading("Fixados"));
        for path in &app.pins {
            let label = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
            col = branch(col, app, label, path, Icon::Pin, theme::ICE, 0);
        }
    }
    if !app.drives.is_empty() {
        col = col.push(space().height(12)).push(heading("Discos"));
        for d in &app.drives {
            col = branch(col, app, d.label.clone(), &d.path, Icon::Drive, theme::STEEL, 0);
        }
    }
    let palette = button(
        row![icon(Icon::Search, 14.0, theme::DARK.ink3), text("Comandos").size(12).width(Fill), kbd(Cmd::Palette.shortcut())]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .on_press(Message::Run(Cmd::Palette))
    .width(Fill)
    .padding([8, 10])
    .style(|t, s| theme::place(t, s, false));
    let content = column![
        container(brand).padding(Padding::new(6.0).bottom(14.0)),
        scrollable(col).height(Fill).style(theme::scrollbar),
        engine(app),
        palette
    ]
    .spacing(8);
    container(content).padding([14, 10]).width(SIDEBAR_W).height(Fill).style(|t| theme::glass(t, false)).into()
}
