//! Tela "Este Computador" no jeito do Explorer: blocos de pastas e de discos, com barra de uso e espaço livre.

use super::{chip, drive_of, entry_kind, heading, thin_vertical};
use crate::app::{App, Message, Tab};
use crate::computer::Drive;
use crate::theme;
use arbor_core::{human_size, Entry};
use iced::widget::text::Wrapping;
use iced::widget::{column, container, mouse_area, progress_bar, row, scrollable, space, text, Column, Row};
use iced::{Alignment, Element, Fill, Theme};

const TILE_W: f32 = 280.0;
/// Acima disto a barra fica vermelha, como no Explorer.
const FULL: f32 = 0.9;

pub fn body<'a>(app: &'a App, tab: &'a Tab, pane_active: bool) -> Element<'a, Message> {
    let (mut folders, mut drives) = (Row::new().spacing(6), Row::new().spacing(6));
    let (mut has_folders, mut has_drives) = (false, false);
    for i in 0..tab.view.len() {
        let e = tab.entry_at(i).expect("índice válido");
        let drive = drive_of(app, tab, e);
        let t = tile(tab, i, e, drive, pane_active);
        if drive.is_some() {
            drives = drives.push(t);
            has_drives = true;
        } else {
            folders = folders.push(t);
            has_folders = true;
        }
    }
    let mut col = Column::new().spacing(6);
    if has_folders {
        col = col.push(heading("Pastas")).push(folders.wrap().vertical_spacing(6)).push(space().height(10));
    }
    if has_drives {
        col = col.push(heading("Dispositivos e unidades")).push(drives.wrap().vertical_spacing(6));
    }
    let id = tab.id;
    scrollable(container(col).padding([8, 18]))
        .id(tab.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled { tab: id, viewport })
        .direction(thin_vertical())
        .height(Fill)
        .style(theme::scrollbar)
        .into()
}

fn tile<'a>(tab: &'a Tab, i: usize, e: &'a Entry, drive: Option<&'a Drive>, pane_active: bool) -> Element<'a, Message> {
    let selected = tab.selected.contains(&e.path);
    let cursor = i == tab.cursor;
    let name = text(e.name.as_str()).size(13).wrapping(Wrapping::None).font(if selected || cursor { theme::UI_BOLD } else { theme::UI });
    let mut info = column![name].spacing(4).width(Fill);
    if let Some((free, total)) = drive.and_then(|d| d.space) {
        let used = if total == 0 { 0.0 } else { 1.0 - free as f32 / total as f32 };
        let bar = progress_bar(0.0..=1.0, used).length(Fill).girth(6);
        let bar = if used >= FULL { bar.style(theme::progress_full) } else { bar.style(theme::progress) };
        let meta = text(format!("{} livres de {}", human_size(free), human_size(total))).font(theme::MONO).size(10.5).style(theme::ink3);
        info = info.push(bar).push(meta);
    }
    let content = row![chip(entry_kind(e, drive), 40.0), info].spacing(10).align_y(Alignment::Center);
    let cell = container(content).width(TILE_W).padding(8).clip(true).style(move |t: &Theme| theme::row(t, selected, cursor, pane_active));
    mouse_area(cell).on_press(Message::Click { tab: tab.id, index: i }).on_double_click(Message::Activate { tab: tab.id, index: i }).into()
}
