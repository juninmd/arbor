//! Interface. A lista é virtualizada: só as linhas visíveis (mais uma margem) viram widgets,
//! então uma pasta com 200 mil arquivos custa o mesmo que uma com 50.
//!
//! O visual segue o protótipo "arbor Glass" (tokens em `theme.rs`): aurora ao fundo, cartões de vidro,
//! ficha de cor por tipo, pílula de seleção e velocidade visível ("N itens em X ms").

use crate::app::{App, Message, Overlay, Pane, Tab, UpdateState, CELL_H, CELL_W, PALETTE_INPUT, PROMPT_INPUT, ROW_H, SIDEBAR_W};
use crate::commands::Cmd;
use crate::computer::{self, Drive};
use crate::media::Preview;
use crate::theme::{self, icon, Icon};
use crate::update::{self, Progress};
use arbor_core::{human_count, human_size, human_time, Entry, SortKey};
use iced::widget::text::Wrapping;
use iced::widget::{
    button, center, column, container, image, mouse_area, opaque, progress_bar, row, scrollable, space, stack, text, text_input, tooltip,
    Column, Row,
};
use iced::{Alignment, Color, ContentFit, Element, Fill, Length, Padding, Theme};
use std::path::{Component, Path, PathBuf};

mod commands_view;
mod computer_view;

const OVERSCAN: usize = 8;
const GAP: f32 = 10.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let panes: Element<_> = if app.dual { row![pane(app, 0), pane(app, 1)].spacing(GAP).into() } else { pane(app, app.active) };
    let body = row![sidebar(app), container(panes).width(Fill).height(Fill)].spacing(GAP).height(Fill);
    let main = container(column![body, dock(app)].spacing(GAP)).padding(GAP).width(Fill).height(Fill).style(theme::backdrop);
    match overlay(app) {
        Some(o) => stack![main, o].into(),
        None => main.into(),
    }
}

pub fn theme(app: &App) -> Theme {
    theme::theme(app.dark)
}

// ---------- barra lateral ----------

fn sidebar(app: &App) -> Element<'_, Message> {
    let current = &app.tab().path;
    let computer_path = computer::path();
    let item = |label: &str, path: &PathBuf, ic: Icon, color: Color| -> Element<'_, Message> {
        let active = current == path;
        let content =
            row![icon(ic, 16.0, color), text(label.to_string()).size(13).wrapping(Wrapping::None)].spacing(10).align_y(Alignment::Center);
        let b = button(content)
            .on_press(Message::Navigate(path.clone()))
            .width(Fill)
            .padding([7, 10])
            .style(move |t: &Theme, s| theme::place(t, s, active));
        mouse_area(b).on_right_press(Message::SidebarMenu(path.clone())).into()
    };
    let brand = row![
        icon(Icon::Logo, 26.0, theme::ICE),
        text("arbor").font(theme::DISPLAY).size(19),
        container(text(update::CURRENT).font(theme::MONO).size(10).style(theme::ink3)).padding([1, 6]).style(theme::kbd),
    ]
    .spacing(9)
    .align_y(Alignment::Center);

    let mut col = Column::new()
        .spacing(2)
        .push(item(computer::TITLE, &computer_path, Icon::Drive, theme::ICE))
        .push(space().height(12))
        .push(heading("Locais"));
    for (label, path) in &app.places {
        let (ic, color) = place_icon(label);
        col = col.push(item(label, path, ic, color));
    }
    if !app.pins.is_empty() {
        col = col.push(space().height(12)).push(heading("Fixados"));
        for path in &app.pins {
            let label = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
            col = col.push(item(&label, path, Icon::Pin, theme::ICE));
        }
    }
    if !app.drives.is_empty() {
        col = col.push(space().height(12)).push(heading("Discos"));
        for d in &app.drives {
            col = col.push(item(&d.label, &d.path, Icon::Drive, theme::STEEL));
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

/// O cartão "motor" do protótipo: o que o arbor acabou de fazer, em números.
fn engine(app: &App) -> Element<'_, Message> {
    let tab = app.tab();
    let line = match tab.listed {
        Some((n, ms)) => format!("{} itens em {}", human_count(n), fmt_ms(ms.as_secs_f64() * 1000.0)),
        None if tab.loading => "lendo a pasta…".into(),
        None => "pronto".into(),
    };
    let title = row![
        container(space()).width(7).height(7).style(|t: &Theme| container::Style {
            background: Some(theme::tokens(t).good.into()),
            border: iced::border::rounded(4),
            ..Default::default()
        }),
        text("Motor").font(theme::UI_BOLD).size(12)
    ]
    .spacing(7)
    .align_y(Alignment::Center);
    container(column![title, text(line).font(theme::MONO).size(11).style(theme::ink2)].spacing(4))
        .padding(10)
        .width(Fill)
        .style(|t: &Theme| {
            let k = theme::tokens(t);
            container::Style {
                background: Some(k.glass_hi.into()),
                border: iced::Border { color: k.stroke, width: 1.0, radius: 12.0.into() },
                ..Default::default()
            }
        })
        .into()
}

fn place_icon(label: &str) -> (Icon, Color) {
    match label {
        "Início" => (Icon::Home, theme::ICE),
        "Imagens" => (Icon::Image, theme::ROSE),
        "Documentos" => (Icon::Doc, theme::IRIS),
        "Downloads" => (Icon::Download, theme::STEEL),
        "Músicas" | "Música" => (Icon::Audio, theme::AMBER),
        "Vídeos" => (Icon::Video, theme::CORAL),
        "Mesa" | "Área de trabalho" => (Icon::Grid, theme::MINT),
        _ => (Icon::Folder, theme::ICE),
    }
}

fn heading(label: &str) -> Element<'_, Message> {
    container(text(label.to_uppercase()).font(theme::UI_BOLD).size(10).style(theme::ink3)).padding(Padding::new(4.0).left(10.0)).into()
}

// ---------- painel ----------

fn pane(app: &App, index: usize) -> Element<'_, Message> {
    let p = &app.panes[index];
    let tab = p.tab();
    let is_active = app.active == index;
    let bar: Element<_> = if is_active { commands_view::bar(app) } else { space().height(0).into() };
    let body = mouse_area(body(app, tab, is_active)).on_right_press(Message::ContextMenu { tab: tab.id, index: None });
    let content = column![tabs_bar(p, index, is_active), toolbar(tab), bar, body, status_line(tab, app.show_hidden)];
    let framed = container(content).width(Fill).height(Fill).clip(true).style(move |t: &Theme| theme::glass(t, is_active && app.dual));
    mouse_area(framed).on_press(Message::FocusPane(index)).into()
}

fn tabs_bar(p: &Pane, pane: usize, pane_active: bool) -> Element<'_, Message> {
    let mut r = Row::new().spacing(4).align_y(Alignment::Center);
    for (i, tab) in p.tabs.iter().enumerate() {
        let selected = i == p.active;
        let color = if selected { theme::ICE } else { theme::DARK.ink3 };
        let mut label = row![icon(Icon::Folder, 14.0, color), text(shorten(&tab.title(), 24)).size(12).wrapping(Wrapping::None)]
            .spacing(7)
            .align_y(Alignment::Center);
        if p.tabs.len() > 1 {
            label =
                label.push(button(text("×").size(13)).padding([0, 5]).on_press(Message::CloseTabAt { pane, index: i }).style(theme::ghost));
        }
        r = r.push(
            button(label)
                .padding([6, 11])
                .on_press(Message::SelectTab { pane, index: i })
                .style(move |t: &Theme, s| theme::tab(t, s, selected, pane_active)),
        );
    }
    r = r.push(icon_button(Icon::Plus, Cmd::NewTab, Message::NewTabIn(pane)));
    container(scrollable(r).direction(thin_horizontal()).style(theme::scrollbar)).padding(Padding::new(8.0).bottom(0.0)).into()
}

fn toolbar(tab: &Tab) -> Element<'_, Message> {
    let nav = row![
        icon_button(Icon::Left, Cmd::Back, Message::Run(Cmd::Back)),
        icon_button(Icon::Right, Cmd::Forward, Message::Run(Cmd::Forward)),
        icon_button(Icon::Up, Cmd::Up, Message::Run(Cmd::Up)),
    ]
    .spacing(2);
    let id = tab.id;
    let filter = text_input("Filtrar", &tab.filter)
        .id(tab.filter_id())
        .on_input(move |value| Message::FilterChanged { tab: id, value })
        .on_submit(Message::FilterSubmit)
        .size(12.5)
        .padding(Padding::from([6, 10]).left(30.0))
        .width(if tab.filter.is_empty() { 150 } else { 210 })
        .style(theme::input);
    let filter = stack![filter, container(icon(Icon::Search, 13.0, theme::DARK.ink3)).padding(Padding::new(0.0).left(10.0)).center_y(Fill)];
    let filter = container(filter).height(30);
    let mut bar = row![nav, breadcrumbs(tab)].spacing(8).align_y(Alignment::Center);
    if let Some(badge) = latency_badge(tab) {
        bar = bar.push(badge);
    }
    bar.push(filter).push(view_switch(tab)).padding([6, 10]).into()
}

/// "100.000 itens · 38 ms": quanto custou listar e ordenar esta pasta. Fica âmbar acima de 150 ms.
fn latency_badge(tab: &Tab) -> Option<Element<'_, Message>> {
    let (n, elapsed) = tab.listed?;
    let ms = elapsed.as_secs_f64() * 1000.0;
    let slow = ms > 150.0;
    let label = format!("{} · {}", human_count(n), fmt_ms(ms));
    let badge = container(text(label).font(theme::MONO).size(10.5)).padding([3, 7]).style(move |t: &Theme| theme::latency(t, slow));
    Some(
        tooltip(badge, text("Tempo para listar e ordenar esta pasta").size(12), tooltip::Position::Bottom)
            .padding(8)
            .style(theme::tooltip)
            .into(),
    )
}

fn view_switch(tab: &Tab) -> Element<'_, Message> {
    let seg = |ic: Icon, on: bool| -> Element<'_, Message> {
        button(icon(ic, 14.0, if on { theme::DARK.ink } else { theme::DARK.ink3 }))
            .padding([5, 7])
            .on_press_maybe((!on).then_some(Message::Run(Cmd::ToggleGrid)))
            .style(move |t: &Theme, s| {
                let mut st = theme::ghost(t, s);
                if on {
                    st.background = Some(theme::alpha(theme::tokens(t).ink, 0.14).into());
                }
                st
            })
            .into()
    };
    container(row![seg(Icon::List, !tab.grid), seg(Icon::Grid, tab.grid)].spacing(2)).padding(2).style(|t| theme::field(t, false)).into()
}

fn breadcrumbs(tab: &Tab) -> Element<'_, Message> {
    let mut acc = PathBuf::new();
    let mut r = Row::new().spacing(1).align_y(Alignment::Center);
    let in_computer = computer::is(&tab.path);
    // Como no Explorer: todo caminho começa em "Este Computador".
    r = r.push(crumb(tab.id, computer::TITLE.into(), computer::path(), in_computer));
    let parts: Vec<_> = if in_computer { vec![] } else { tab.path.components().collect() };
    if !parts.is_empty() {
        r = r.push(text("›").size(12).style(theme::ink3));
    }
    for (i, c) in parts.iter().enumerate() {
        acc.push(c.as_os_str());
        let label = match c {
            Component::RootDir if i == 0 => "/".to_string(),
            Component::RootDir => continue,
            Component::Prefix(p) => p.as_os_str().to_string_lossy().into_owned(),
            other => other.as_os_str().to_string_lossy().into_owned(),
        };
        if i > 0 && !matches!(parts[i - 1], Component::RootDir | Component::Prefix(_)) {
            r = r.push(text("›").size(12).style(theme::ink3));
        }
        r = r.push(crumb(tab.id, label, acc.clone(), i == parts.len() - 1));
    }
    let bar = scrollable(r).direction(thin_horizontal()).anchor_right().width(Fill).style(theme::scrollbar);
    mouse_area(container(bar).width(Fill)).on_double_click(Message::Run(Cmd::GoTo)).into()
}

fn body<'a>(app: &'a App, tab: &'a Tab, pane_active: bool) -> Element<'a, Message> {
    if let Some(err) = &tab.error {
        return center(
            column![text("Não foi possível abrir a pasta").font(theme::DISPLAY).size(15), text(err.as_str()).size(12).style(theme::danger)]
                .spacing(6)
                .align_x(Alignment::Center),
        )
        .into();
    }
    if tab.view.is_empty() {
        let (title, hint) = if tab.loading {
            ("Lendo a pasta…", "")
        } else if !tab.filter.is_empty() {
            ("Nada corresponde ao filtro", "Esc limpa o filtro")
        } else {
            ("Pasta vazia", "F7 cria uma nova pasta aqui")
        };
        let empty = center(
            column![text(title).font(theme::DISPLAY).size(14).style(theme::ink2), text(hint).size(12).style(theme::ink3)]
                .spacing(6)
                .align_x(Alignment::Center),
        );
        return column![if tab.grid { space().into() } else { header(tab) }, empty].into();
    }
    if tab.grid {
        grid(app, tab, pane_active)
    } else if computer::is(&tab.path) {
        computer_view::body(app, tab, pane_active)
    } else {
        column![header(tab), list(app, tab, pane_active)].into()
    }
}

fn header(tab: &Tab) -> Element<'_, Message> {
    let id = tab.id;
    let disks = computer::is(&tab.path);
    let col = |label: &'static str, key: SortKey, width: Length, right: bool| -> Element<'_, Message> {
        let arrow = match (tab.sort.key == key, tab.sort.desc) {
            (true, true) => " ↓",
            (true, false) => " ↑",
            _ => "",
        };
        let on = tab.sort.key == key;
        let t = text(format!("{}{arrow}", label.to_uppercase())).font(theme::UI_BOLD).size(10).style(move |t: &Theme| {
            if on {
                theme::accent(t)
            } else {
                theme::ink3(t)
            }
        });
        let t = if right { t.width(Fill).align_x(Alignment::End) } else { t };
        button(t).padding([3, 0]).width(width).on_press(Message::SortBy { tab: id, key }).style(theme::ghost).into()
    };
    container(
        row![
            space().width(28),
            col("Nome", SortKey::Name, Fill, false),
            col("Tipo", SortKey::Kind, Length::Fixed(64.0), false),
            col(if disks { "Total" } else { "Tamanho" }, SortKey::Size, Length::Fixed(80.0), true),
            col(if disks { "Livre" } else { "Modificado" }, SortKey::Modified, Length::Fixed(118.0), false),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([2, 18])
    .into()
}

fn list<'a>(app: &'a App, tab: &'a Tab, pane_active: bool) -> Element<'a, Message> {
    let total = tab.view.len();
    let first = ((tab.scroll_y / ROW_H).floor() as usize).saturating_sub(OVERSCAN).min(total);
    let end = (first + (tab.viewport_h / ROW_H).ceil() as usize + 2 * OVERSCAN).min(total);
    let mut col = Column::new().push(space().height(first as f32 * ROW_H));
    for i in first..end {
        col = col.push(list_row(app, tab, i, pane_active));
    }
    col = col.push(space().height((total - end) as f32 * ROW_H));
    let id = tab.id;
    scrollable(col.padding(Padding::from([0, 8]).bottom(8.0)))
        .id(tab.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled { tab: id, viewport })
        .direction(thin_vertical())
        .height(Fill)
        .style(theme::scrollbar)
        .into()
}

fn list_row<'a>(app: &'a App, tab: &'a Tab, i: usize, pane_active: bool) -> Element<'a, Message> {
    let e = tab.entry_at(i).expect("índice válido");
    let drive = drive_of(app, tab, e);
    let (size, when) = match drive {
        Some(d) => d.space.map(|(free, total)| (human_size(total), human_size(free))).unwrap_or_default(),
        None => (if e.is_dir { String::new() } else { human_size(e.size) }, human_time(e.modified)),
    };
    let selected = tab.selected.contains(&e.path);
    let cursor = i == tab.cursor;
    let dim = e.hidden;
    let strong = selected || cursor;
    let name = text(e.name.as_str())
        .size(13)
        .wrapping(Wrapping::None)
        .width(Fill)
        .font(if strong { theme::UI_BOLD } else { theme::UI })
        .style(move |t: &Theme| if dim { theme::ink3(t) } else { text::Style { color: Some(theme::tokens(t).ink) } });
    let line = row![
        chip(entry_kind(e, drive), 24.0),
        name,
        text(if drive.is_some() { "Disco".into() } else { kind_label(e) }).size(11.5).width(64).wrapping(Wrapping::None).style(theme::ink3),
        text(size).font(theme::MONO).size(11).width(80).align_x(Alignment::End).style(theme::ink3),
        text(when).font(theme::MONO).size(11).width(118).wrapping(Wrapping::None).style(theme::ink3),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let cell = container(line)
        .height(ROW_H)
        .padding([0, 10])
        .align_y(Alignment::Center)
        .clip(true)
        .style(move |t: &Theme| theme::row(t, selected, cursor, pane_active));
    mouse_area(cell)
        .on_press(Message::Click { tab: tab.id, index: i })
        .on_double_click(Message::Activate { tab: tab.id, index: i })
        .on_right_press(Message::ContextMenu { tab: tab.id, index: Some(i) })
        .into()
}

fn grid<'a>(app: &'a App, tab: &'a Tab, pane_active: bool) -> Element<'a, Message> {
    let cols = app.grid_cols();
    let total = tab.view.len();
    let rows_total = total.div_ceil(cols);
    let first_row = ((tab.scroll_y / CELL_H).floor() as usize).saturating_sub(2).min(rows_total);
    let end_row = (first_row + (tab.viewport_h / CELL_H).ceil() as usize + 4).min(rows_total);
    let mut col = Column::new().push(space().height(first_row as f32 * CELL_H));
    for r in first_row..end_row {
        let mut line = Row::new();
        for i in r * cols..((r + 1) * cols).min(total) {
            line = line.push(grid_cell(app, tab, i, pane_active));
        }
        col = col.push(line);
    }
    col = col.push(space().height((rows_total - end_row) as f32 * CELL_H));
    let id = tab.id;
    scrollable(col.padding([4, 10]))
        .id(tab.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled { tab: id, viewport })
        .direction(thin_vertical())
        .height(Fill)
        .style(theme::scrollbar)
        .into()
}

fn grid_cell<'a>(app: &'a App, tab: &'a Tab, i: usize, pane_active: bool) -> Element<'a, Message> {
    let e = tab.entry_at(i).expect("índice válido");
    let selected = tab.selected.contains(&e.path);
    let cursor = i == tab.cursor;
    let drive = drive_of(app, tab, e);
    let k = entry_kind(e, drive);
    let color = kind_color(k);
    let thumb: Element<_> = match app.thumbs.get(&e.path) {
        Some(Some(h)) => image(h.clone()).content_fit(ContentFit::Cover).width(Fill).height(96).border_radius(10).into(),
        _ => container(chip(k, 46.0))
            .center_x(Fill)
            .center_y(96)
            .style(move |_| container::Style {
                background: Some(theme::alpha(color, 0.08).into()),
                border: iced::border::rounded(10),
                ..Default::default()
            })
            .into(),
    };
    let label =
        text(shorten(&e.name, 30)).size(12).width(Fill).wrapping(Wrapping::None).font(if selected { theme::UI_BOLD } else { theme::UI });
    let meta = match drive.and_then(|d| d.space) {
        Some((free, total)) => format!("{} livres de {}", human_size(free), human_size(total)),
        None if e.is_dir => "Pasta".to_string(),
        None => human_size(e.size),
    };
    let meta = text(meta).font(theme::MONO).size(10).style(theme::ink3);
    let cell = container(column![thumb, label, meta].spacing(4)).width(CELL_W - 10.0).height(CELL_H - 10.0).padding(6).clip(true).style(
        move |t: &Theme| {
            let mut s = theme::row(t, selected, cursor, pane_active);
            if !selected && !cursor {
                s.background = Some(theme::tokens(t).glass_hi.into());
            }
            s.border.radius = 15.0.into();
            s
        },
    );
    let cell = container(cell).padding(5);
    mouse_area(cell)
        .on_press(Message::Click { tab: tab.id, index: i })
        .on_double_click(Message::Activate { tab: tab.id, index: i })
        .on_right_press(Message::ContextMenu { tab: tab.id, index: Some(i) })
        .into()
}

fn status_line(tab: &Tab, show_hidden: bool) -> Element<'_, Message> {
    let mut parts = vec![format!("{} itens", human_count(tab.view.len()))];
    if !tab.filter.is_empty() {
        let total = tab.entries.iter().filter(|e| show_hidden || !e.hidden).count();
        parts[0] = format!("{} de {} itens", human_count(tab.view.len()), human_count(total));
    }
    if !tab.selected.is_empty() {
        let bytes: u64 = tab.entries.iter().filter(|e| tab.selected.contains(&e.path)).map(|e| e.size).sum();
        parts.push(format!("{} selecionado(s), {}", tab.selected.len(), human_size(bytes)));
    }
    if tab.loading {
        parts.push(format!("lendo… {}", human_count(tab.entries.len())));
    }
    container(text(parts.join("  ·  ")).size(11.5).style(theme::ink3)).padding([6, 16]).into()
}

fn crumb<'a>(tab: u64, label: String, path: PathBuf, last: bool) -> Element<'a, Message> {
    let label = text(label).size(12.5).wrapping(Wrapping::None).font(if last { theme::UI_BOLD } else { theme::UI });
    button(label)
        .padding([4, 7])
        .on_press(Message::NavigateIn { tab, path })
        .style(move |t: &Theme, s| {
            let mut st = theme::ghost(t, s);
            if last {
                st.text_color = theme::tokens(t).ink;
            } else if s == button::Status::Active {
                st.text_color = theme::tokens(t).ink3;
            }
            st
        })
        .into()
}

// ---------- doca: atalhos, operações, avisos, fps e atualização ----------

fn dock(app: &App) -> Element<'_, Message> {
    let mut col = Column::new().spacing(6);
    for job in &app.jobs {
        let pct = if job.total > 0 { job.done as f32 / job.total as f32 } else { 0.0 };
        let detail = if job.total > 1000 {
            format!("{} de {}  {}", human_size(job.done), human_size(job.total), job.current)
        } else {
            job.current.clone()
        };
        col = col.push(
            row![
                text(job.label.as_str()).font(theme::UI_BOLD).size(12).width(Length::FillPortion(2)).wrapping(Wrapping::None),
                progress_bar(0.0..=1.0, pct).length(Fill).girth(5).style(theme::progress),
                text(detail).font(theme::MONO).size(10.5).style(theme::ink3).width(Length::FillPortion(2)).wrapping(Wrapping::None),
                button(text("Cancelar").size(11)).padding([3, 9]).on_press(Message::CancelJob(job.id)).style(theme::ghost),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
    }
    if let Some(u) = update_banner(app) {
        col = col.push(u);
    }

    let status: Element<_> = if let Some(t) = &app.toast {
        let err = t.error;
        text(t.text.as_str())
            .size(12)
            .width(Fill)
            .wrapping(Wrapping::None)
            .style(move |th: &Theme| if err { theme::danger(th) } else { theme::ink2(th) })
            .into()
    } else {
        let tab = app.tab();
        let where_ = tab.title();
        row![
            text(where_).font(theme::UI_BOLD).size(12),
            text(format!("·  {} itens", human_count(tab.view.len()))).size(12).style(theme::ink3)
        ]
        .spacing(8)
        .width(Fill)
        .into()
    };
    let mut bottom = row![status].spacing(14).align_y(Alignment::Center);
    if let Some(fps) = &app.fps {
        let (frames, worst) = fps.stats();
        let slow = worst > 16.7;
        let label = format!("{frames} fps · pior {worst:.1} ms");
        bottom = bottom
            .push(container(text(label).font(theme::MONO).size(10.5)).padding([3, 8]).style(move |t: &Theme| theme::latency(t, slow)));
    }
    let keys = [
        ("Espaço", "Preview", Cmd::Preview),
        ("F2", "Renomear", Cmd::Rename),
        ("F3", "Duplo", Cmd::ToggleDual),
        ("F5", "Copiar", Cmd::CopyToOther),
        ("F6", "Mover", Cmd::MoveToOther),
        ("F7", "Pasta", Cmd::NewFolder),
        ("Del", "Lixeira", Cmd::Trash),
    ];
    let mut dock_keys = Row::new().spacing(4);
    for (k, label, cmd) in keys {
        dock_keys = dock_keys.push(
            button(row![kbd(k.to_string()), text(label).size(11.5)].spacing(6).align_y(Alignment::Center))
                .padding([4, 8])
                .on_press(Message::Run(cmd))
                .style(theme::ghost),
        );
    }
    bottom = bottom.push(dock_keys);
    col = col.push(bottom);
    container(col).padding([7, 12]).width(Fill).style(|t| theme::glass(t, false)).into()
}

fn update_banner(app: &App) -> Option<Element<'_, Message>> {
    let content: Element<_> = match &app.update {
        UpdateState::Idle => return None,
        UpdateState::Available(u) => row![
            icon(Icon::Bolt, 15.0, theme::ICE),
            text(format!("arbor {} disponível", u.version)).font(theme::UI_BOLD).size(12.5),
            text(format!("você usa a {}", update::CURRENT)).size(11.5).style(theme::ink3).width(Fill),
            button(text("Atualizar e reiniciar").font(theme::UI_BOLD).size(12))
                .padding([5, 12])
                .on_press(Message::UpdateInstall)
                .style(theme::primary),
            button(text("Depois").size(12)).padding([5, 10]).on_press(Message::UpdateDismiss).style(theme::ghost),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
        UpdateState::Working(p) => {
            let (label, pct) = match p {
                Progress::Downloading { received, total: Some(t) } => {
                    (format!("Baixando {} de {}", human_size(*received), human_size(*t)), *received as f32 / (*t).max(1) as f32)
                }
                Progress::Downloading { received, total: None } => (format!("Baixando {}", human_size(*received)), 0.0),
                Progress::Verifying => ("Conferindo SHA-256…".into(), 1.0),
                Progress::Ready => ("Atualização instalada".into(), 1.0),
                Progress::Failed(e) => (e.clone(), 0.0),
            };
            let ready = matches!(p, Progress::Ready);
            let action = if ready {
                button(text("Reiniciar agora").font(theme::UI_BOLD).size(12))
                    .padding([5, 12])
                    .on_press(Message::UpdateRestart)
                    .style(theme::primary)
            } else {
                button(text("Cancelar").size(12)).padding([5, 10]).on_press(Message::UpdateDismiss).style(theme::ghost)
            };
            row![
                icon(Icon::Download, 15.0, theme::ICE),
                text(label).size(12).width(Length::FillPortion(2)),
                progress_bar(0.0..=1.0, pct).length(Length::FillPortion(3)).girth(5).style(theme::progress),
                action
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        }
    };
    Some(
        container(content)
            .padding([6, 10])
            .style(|t: &Theme| {
                let k = theme::tokens(t);
                iced::widget::container::Style {
                    background: Some(theme::alpha(k.accent, 0.07).into()),
                    border: iced::Border { color: theme::alpha(k.accent, 0.3), width: 1.0, radius: 12.0.into() },
                    ..Default::default()
                }
            })
            .into(),
    )
}

// ---------- sobreposições ----------

fn overlay(app: &App) -> Option<Element<'_, Message>> {
    let card: Element<_> = match &app.overlay {
        Overlay::None => return None,
        Overlay::Palette { query, selected } => {
            let card = palette(app, query, *selected);
            return Some(stack![scrim(), container(opaque(card)).center_x(Fill).padding(Padding::new(0.0).top(90.0))].into());
        }
        Overlay::Prompt { kind, value } => {
            let input = text_input("", value)
                .id(PROMPT_INPUT)
                .on_input(Message::PromptChanged)
                .on_submit(Message::PromptSubmit)
                .padding(10)
                .size(14)
                .style(theme::input);
            sheet(
                column![
                    text(kind.title()).font(theme::DISPLAY).size(16),
                    input,
                    row![
                        kbd("Enter".into()),
                        text("confirma").size(11.5).style(theme::ink3),
                        kbd("Esc".into()),
                        text("cancela").size(11.5).style(theme::ink3)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center)
                ]
                .spacing(12),
                460.0,
            )
        }
        Overlay::Preview { path, content } => preview(app, path, content.as_ref()),
        Overlay::Menu(m) => return Some(commands_view::menu(app, m)),
    };
    Some(stack![scrim(), center(opaque(card)).padding(40)].into())
}

fn scrim<'a>() -> Element<'a, Message> {
    mouse_area(container(space()).width(Fill).height(Fill).style(theme::scrim)).on_press(Message::CloseOverlay).into()
}

fn palette<'a>(app: &'a App, query: &'a str, selected: usize) -> Element<'a, Message> {
    let input = text_input("Comando, ação ou atalho…", query)
        .id(PALETTE_INPUT)
        .on_input(Message::PaletteQuery)
        .on_submit(Message::PaletteRun(usize::MAX))
        .padding(Padding::from([12, 14]).left(42.0))
        .size(16)
        .style(|t, s| {
            let mut st = theme::input(t, s);
            st.border.width = 0.0;
            st
        });
    let input = stack![input, container(icon(Icon::Bolt, 18.0, theme::ICE)).padding(Padding::new(0.0).left(14.0)).center_y(Fill)];
    let mut list = Column::new().spacing(2);
    let start = selected.saturating_sub(11);
    for (i, cmd) in app.palette_view().into_iter().enumerate().skip(start).take(12) {
        let active = i == selected;
        let sc = cmd.shortcut();
        let mut line = row![text(cmd.label()).size(13.5).width(Fill)].align_y(Alignment::Center);
        if !sc.is_empty() {
            line = line.push(kbd(sc));
        }
        list = list.push(
            button(line)
                .width(Fill)
                .padding([8, 12])
                .on_press(Message::PaletteRun(i))
                .style(move |t: &Theme, s| theme::place(t, s, active)),
        );
    }
    let foot = row![
        kbd("↑".into()),
        kbd("↓".into()),
        text("navegar").size(11).style(theme::ink3),
        kbd("Enter".into()),
        text("executar").size(11).style(theme::ink3),
        kbd("Esc".into()),
        text("fechar").size(11).style(theme::ink3)
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    sheet(column![container(input).height(48), separator(), list, separator(), foot].spacing(10), 640.0)
}

fn preview<'a>(app: &'a App, path: &'a Path, content: Option<&'a Preview>) -> Element<'a, Message> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let max_h = (app.window.height - 160.0).max(200.0);
    let max_w = (app.window.width - 160.0).clamp(300.0, 1400.0);
    let info_table = |info: &'a [(String, String)]| -> Element<'a, Message> {
        let mut c = Column::new().spacing(8);
        for (k, v) in info {
            c = c.push(
                row![text(k.as_str()).size(12).style(theme::ink3).width(Fill), text(v.as_str()).font(theme::MONO).size(11.5)].spacing(10),
            );
        }
        scrollable(c).width(270).style(theme::scrollbar).into()
    };
    let inner: Element<_> = match content {
        None => center(text("Carregando…").style(theme::ink3)).height(240).into(),
        Some(Preview::Image { handle, info, .. }) => row![
            image(handle.clone()).content_fit(ContentFit::ScaleDown).width(Fill).height(Length::Fixed(max_h - 70.0)).border_radius(12),
            info_table(info)
        ]
        .spacing(18)
        .into(),
        Some(Preview::Text { content, truncated }) => {
            let mut c = column![text(content.as_str()).font(theme::MONO).size(12.5)];
            if *truncated {
                c = c.push(text("… (mostrando os primeiros 64 KB)").size(11).style(theme::ink3));
            }
            container(scrollable(c.padding(14)).height(Length::Fixed(max_h - 70.0)).width(Fill).style(theme::scrollbar))
                .style(|t: &Theme| iced::widget::container::Style {
                    background: Some(theme::alpha(theme::tokens(t).void, 0.45).into()),
                    border: iced::border::rounded(12),
                    ..Default::default()
                })
                .into()
        }
        Some(Preview::Folder { dirs, files, bytes, more }) => {
            let plus = if *more { "+" } else { "" };
            center(
                column![
                    icon(Icon::Folder, 64.0, theme::ICE),
                    text(format!("{dirs}{plus} pastas · {files}{plus} arquivos · {}{plus}", human_size(*bytes)))
                        .size(14)
                        .style(theme::ink2)
                ]
                .spacing(14)
                .align_x(Alignment::Center),
            )
            .height(200)
            .into()
        }
        Some(Preview::Other { info }) => {
            column![center(text("Sem preview para este tipo").style(theme::ink3)).height(120), info_table(info)].into()
        }
        Some(Preview::Error(e)) => center(text(format!("Não foi possível mostrar: {e}")).style(theme::danger)).height(160).into(),
    };
    let title = row![
        text(name).font(theme::DISPLAY).size(16).width(Fill).wrapping(Wrapping::None),
        kbd("↑↓".into()),
        text("navega").size(11).style(theme::ink3),
        kbd("Enter".into()),
        text("abre").size(11).style(theme::ink3),
        kbd("Espaço".into()),
        text("fecha").size(11).style(theme::ink3)
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    sheet(column![title, inner].spacing(14), max_w)
}

fn sheet<'a>(content: impl Into<Element<'a, Message>>, width: f32) -> Element<'a, Message> {
    container(content).padding(18).max_width(width).style(theme::sheet).into()
}

fn separator<'a>() -> Element<'a, Message> {
    container(space())
        .width(Fill)
        .height(1)
        .style(|t: &Theme| iced::widget::container::Style { background: Some(theme::tokens(t).stroke.into()), ..Default::default() })
        .into()
}

fn separator_v<'a>() -> Element<'a, Message> {
    container(space())
        .width(1)
        .height(18)
        .style(|t: &Theme| iced::widget::container::Style { background: Some(theme::tokens(t).stroke.into()), ..Default::default() })
        .into()
}

// ---------- peças pequenas ----------

fn thin_horizontal() -> scrollable::Direction {
    scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(2).scroller_width(2))
}

fn thin_vertical() -> scrollable::Direction {
    scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(6).scroller_width(6).margin(2))
}

fn kbd<'a>(label: String) -> Element<'a, Message> {
    container(text(label).font(theme::MONO).size(10)).padding([1, 6]).style(theme::kbd).into()
}

fn icon_button<'a>(ic: Icon, cmd: Cmd, msg: Message) -> Element<'a, Message> {
    let b = button(icon(ic, 15.0, theme::DARK.ink2)).padding([5, 7]).on_press(msg).style(theme::ghost);
    let sc = cmd.shortcut();
    let tip = if sc.is_empty() { cmd.label().to_string() } else { format!("{}  {sc}", cmd.label()) };
    tooltip(b, text(tip).size(12), tooltip::Position::Bottom).padding(8).style(theme::tooltip).into()
}

fn fmt_ms(ms: f64) -> String {
    if ms < 10.0 {
        format!("{ms:.1} ms")
    } else {
        format!("{ms:.0} ms")
    }
}

/// Em "Este Computador", as entradas que são discos (e só elas) ganham ícone e colunas de disco.
fn drive_of<'a>(app: &'a App, tab: &Tab, e: &Entry) -> Option<&'a Drive> {
    computer::is(&tab.path).then(|| app.drives.iter().find(|d| d.path == e.path)).flatten()
}

fn entry_kind(e: &Entry, drive: Option<&Drive>) -> Kind {
    if drive.is_some() {
        Kind::Drive
    } else {
        kind(e)
    }
}

fn kind_label(e: &Entry) -> String {
    if e.is_dir {
        "Pasta".into()
    } else if e.ext.is_empty() {
        "Arquivo".into()
    } else {
        e.ext.to_uppercase()
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Folder,
    Drive,
    Image,
    Code,
    Archive,
    Doc,
    Audio,
    Video,
    Other,
}

fn kind(e: &Entry) -> Kind {
    if e.is_dir {
        return Kind::Folder;
    }
    match e.ext.as_str() {
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "c" | "h" | "cpp" | "hpp" | "cs" | "java" | "kt" | "swift" | "rb" | "php"
        | "sh" | "ps1" | "toml" | "json" | "yaml" | "yml" | "xml" | "html" | "css" | "lua" | "sql" | "lock" => Kind::Code,
        "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" | "zst" | "iso" | "dmg" => Kind::Archive,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "txt" | "rtf" | "epub" | "md" => Kind::Doc,
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "opus" | "aac" => Kind::Audio,
        "mp4" | "mkv" | "mov" | "avi" | "webm" => Kind::Video,
        _ if crate::media::is_image(&e.path) => Kind::Image,
        _ => Kind::Other,
    }
}

/// Uma cor por tipo, a mesma do protótipo.
fn kind_color(k: Kind) -> Color {
    match k {
        Kind::Folder => theme::ICE,
        Kind::Drive => theme::STEEL,
        Kind::Image => theme::ROSE,
        Kind::Code => theme::MINT,
        Kind::Doc => theme::IRIS,
        Kind::Audio => theme::AMBER,
        Kind::Video => theme::CORAL,
        Kind::Archive | Kind::Other => theme::STEEL,
    }
}

fn kind_icon(k: Kind) -> Icon {
    match k {
        Kind::Folder => Icon::Folder,
        Kind::Drive => Icon::Drive,
        Kind::Image => Icon::Image,
        Kind::Code => Icon::Code,
        Kind::Doc => Icon::Doc,
        Kind::Audio => Icon::Audio,
        Kind::Video => Icon::Video,
        Kind::Archive => Icon::Archive,
        Kind::Other => Icon::File,
    }
}

/// Ficha de ícone: quadrado arredondado tingido com a cor do tipo.
fn chip<'a>(k: Kind, size: f32) -> Element<'a, Message> {
    let color = kind_color(k);
    container(icon(kind_icon(k), size * 0.58, color)).center_x(size).center_y(size).style(theme::chip(color)).into()
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max - 1).collect();
        format!("{head}…")
    }
}
