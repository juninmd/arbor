//! Barra lateral no jeito do painel de navegação do Explorer: Início, locais e fixados, Este Computador
//! com os discos dentro (e o quanto estão cheios) e a Lixeira. Toda pasta se expande pela seta.

use super::place_icon;
use crate::app::{App, Message, SIDEBAR_W};
use crate::computer::{self, Drive};
use crate::theme::{self, icon, Icon};
use arbor_core::human_size;
use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, mouse_area, progress_bar, row, scrollable, space, text, Column};
use iced::{Alignment, Color, Element, Fill, Padding, Theme};
use std::path::{Path, PathBuf};

const INDENT: f32 = 14.0;
/// Limite de profundidade desenhada: protege contra laços de links simbólicos abertos à mão.
const MAX_DEPTH: usize = 24;

fn node<'a>(app: &'a App, label: String, path: &Path, ic: Icon, color: Color, depth: usize) -> Element<'a, Message> {
    let active = app.tab().path == path;
    let open = app.tree.open.contains(path);
    // Este Computador é virtual mas abre: dentro dele ficam os discos
    let chevron: Element<_> = if computer::is_virtual(path) && !computer::is(path) {
        space().width(16).into()
    } else {
        button(icon(if open { Icon::Down } else { Icon::Right }, 12.0, theme::now().ink3))
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
    let mark = container(space().width(3).height(16)).style(move |t: &Theme| container::Style {
        background: active.then(|| theme::tokens(t).accent.into()),
        border: iced::border::rounded(2),
        ..Default::default()
    });
    let line = row![mark, space().width(depth as f32 * INDENT), chevron, b].spacing(2).align_y(Alignment::Center);
    let p = path.to_path_buf();
    let area = mouse_area(line).on_right_press(Message::SidebarMenu(p.clone()));
    if app.dragging().is_some() {
        area.on_enter(Message::DragOver(Some(p.clone()))).on_exit(Message::DragLeave { from: p, back: None }).into()
    } else {
        area.into()
    }
}

/// Área que aparece durante um arrasto: soltar pastas aqui as fixa.
fn pin_drop<'a>(hot: bool) -> Element<'a, Message> {
    let content =
        row![icon(Icon::Pin, 14.0, theme::now().hues.ice), text("Solte para fixar").size(12)].spacing(10).align_y(Alignment::Center);
    let zone = container(content).width(Fill).padding([7, 10]).style(move |t: &Theme| theme::drop_zone(t, hot));
    let p = crate::pins::drop_target();
    mouse_area(zone).on_enter(Message::DragOver(Some(p.clone()))).on_exit(Message::DragLeave { from: p, back: None }).into()
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
                col = branch(col, app, name, child, Icon::Folder, theme::now().hues.ice, depth + 1);
            }
            col
        }
    }
}

/// Disco dentro de Este Computador: nome e, quando o volume responde, a barra de uso.
fn drive<'a>(col: Column<'a, Message>, app: &'a App, d: &'a Drive) -> Column<'a, Message> {
    let (ic, color) = super::drive_look(d.media);
    let col = branch(col, app, d.label.clone(), &d.path, ic, color, 1);
    let Some((free, total)) = d.space.filter(|(_, t)| *t > 0) else { return col };
    let used = 1.0 - free as f32 / total as f32;
    let full = used > 0.9;
    let usage = column![
        progress_bar(0.0..=1.0, used).length(Fill).girth(3).style(if full { theme::progress_full } else { theme::progress }),
        text(format!("{} livres de {}", human_size(free), human_size(total))).size(10.5).style(theme::ink3),
    ]
    .spacing(3);
    col.push(container(usage).padding(Padding::new(0.0).left(INDENT + 46.0).right(10.0).bottom(4.0)))
}

fn rule<'a>() -> Element<'a, Message> {
    container(super::separator()).padding([6, 10]).into()
}

/// Locais, fixados, Este Computador e Lixeira, em coluna (a barra comum e a unificada embrulham isto).
pub fn tree(app: &App) -> Column<'_, Message> {
    let mut col = Column::new().spacing(1).push(node(app, crate::home::TITLE.into(), &app.home, Icon::Home, theme::now().hues.ice, 0));
    for (label, path) in &app.places {
        let (ic, color) = place_icon(label);
        col = branch(col, app, label.clone(), path, ic, color, 0);
    }
    for path in &app.pins {
        let label = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        col = branch(col, app, label, path, Icon::Pin, theme::now().hues.ice, 0);
    }
    // arrastando uma pasta: alvo para fixá-la logo abaixo dos fixados
    if let Some((_, _, over)) = app.dragging() {
        col = col.push(pin_drop(over.is_some_and(crate::pins::is_drop)));
    }
    col = col.push(rule()).push(node(app, computer::TITLE.into(), &app.computer, Icon::Drive, theme::now().hues.ice, 0));
    if app.tree.open.contains(&app.computer) {
        for d in &app.drives {
            col = drive(col, app, d);
        }
    }
    col = col.push(node(app, crate::discovery::TITLE.into(), &app.network, Icon::Network, theme::now().hues.iris, 0));
    if crate::trashbin::SUPPORTED {
        col = col.push(rule()).push(node(app, crate::trashbin::TITLE.into(), &app.trash, Icon::Trash, theme::now().hues.steel, 0));
    }
    col
}

pub fn sidebar(app: &App) -> Element<'_, Message> {
    let content = scrollable(tree(app)).height(Fill).style(theme::scrollbar);
    container(content).padding([10, 6]).width(SIDEBAR_W).height(Fill).style(|t| theme::glass(t, false)).into()
}
