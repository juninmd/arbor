//! Interface. A lista é virtualizada: só as linhas visíveis (mais uma margem) viram widgets,
//! então uma pasta com 200 mil arquivos custa o mesmo que uma com 50.

use crate::app::{App, Message, Overlay, Pane, Tab, CELL_H, CELL_W, PALETTE_INPUT, PROMPT_INPUT, ROW_H, SIDEBAR_W};
use crate::commands::Cmd;
use crate::media::Preview;
use crate::model::{human_size, human_time, Entry, SortKey};
use iced::border;
use iced::widget::text::Wrapping;
use iced::widget::{
    button, center, column, container, image, mouse_area, opaque, progress_bar, row, scrollable, space, stack, text, text_input, tooltip,
    Column, Row,
};
use iced::{Alignment, Background, Color, ContentFit, Element, Fill, Font, Length, Padding, Shadow, Theme, Vector};
use std::path::{Component, Path, PathBuf};

const OVERSCAN: usize = 8;

pub fn view(app: &App) -> Element<'_, Message> {
    let panes: Element<_> = if app.dual { row![pane(app, 0), pane(app, 1)].spacing(6).into() } else { pane(app, app.active) };
    let body = row![sidebar(app), container(panes).padding(Padding::new(6.0).left(0.0)).width(Fill).height(Fill)].height(Fill);
    let main = container(column![body, bottom_bar(app)]).width(Fill).height(Fill).style(|t: &Theme| container::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        text_color: Some(t.extended_palette().background.base.text),
        ..Default::default()
    });
    match overlay(app) {
        Some(o) => stack![main, o].into(),
        None => main.into(),
    }
}

// ---------- barra lateral ----------

fn sidebar(app: &App) -> Element<'_, Message> {
    let current = &app.tab().path;
    let item = |label: &str, path: &PathBuf| -> Element<'_, Message> {
        let active = current == path;
        button(text(label.to_string()).size(13).wrapping(Wrapping::None))
            .on_press(Message::Navigate(path.clone()))
            .width(Fill)
            .padding([5, 10])
            .style(move |t: &Theme, s| side_button(t, s, active))
            .into()
    };
    let mut col = Column::new().spacing(2).push(heading("Locais"));
    for (label, path) in &app.places {
        col = col.push(item(label, path));
    }
    if !app.drives.is_empty() {
        col = col.push(space().height(10)).push(heading("Discos"));
        for (label, path) in &app.drives {
            col = col.push(item(label, path));
        }
    }
    let footer = button(text("⌘  Comandos").size(12))
        .on_press(Message::Run(Cmd::Palette))
        .width(Fill)
        .padding([6, 10])
        .style(|t, s| side_button(t, s, false));
    let footer = tooltip(footer, text(Cmd::Palette.shortcut()).size(12), tooltip::Position::Top).padding(6).style(tip);
    container(column![scrollable(col.padding([10, 8])).height(Fill), container(footer).padding(8)])
        .width(SIDEBAR_W)
        .height(Fill)
        .style(|t: &Theme| container::Style {
            background: Some(t.extended_palette().background.weakest.color.into()),
            ..Default::default()
        })
        .into()
}

fn heading(label: &str) -> Element<'_, Message> {
    container(text(label.to_uppercase()).size(11).style(muted)).padding([4, 10]).into()
}

// ---------- painel ----------

fn pane(app: &App, index: usize) -> Element<'_, Message> {
    let p = &app.panes[index];
    let tab = p.tab();
    let is_active = app.active == index;
    let content = column![tabs_bar(p, index, is_active), toolbar(tab), body(app, tab), status_line(tab, app.show_hidden)].spacing(0);
    let framed = container(content).width(Fill).height(Fill).clip(true).style(move |t: &Theme| {
        let pal = t.extended_palette();
        container::Style {
            background: Some(pal.background.weakest.color.into()),
            border: border::rounded(8).width(1).color(if is_active { pal.primary.base.color } else { pal.background.strong.color }),
            ..Default::default()
        }
    });
    mouse_area(framed).on_press(Message::FocusPane(index)).into()
}

fn tabs_bar(p: &Pane, pane: usize, pane_active: bool) -> Element<'_, Message> {
    let mut r = Row::new().spacing(2).align_y(Alignment::Center);
    for (i, tab) in p.tabs.iter().enumerate() {
        let selected = i == p.active;
        let mut label = row![text(shorten(&tab.title(), 24)).size(12).wrapping(Wrapping::None)].spacing(6).align_y(Alignment::Center);
        if p.tabs.len() > 1 {
            label = label.push(button(text("×").size(12)).padding([0, 4]).on_press(Message::CloseTabAt { pane, index: i }).style(ghost));
        }
        r = r.push(
            button(label)
                .padding([4, 10])
                .on_press(Message::SelectTab { pane, index: i })
                .style(move |t: &Theme, s| tab_button(t, s, selected, pane_active)),
        );
    }
    r = r.push(tip_button("+", Cmd::NewTab, Message::NewTabIn(pane)));
    container(scrollable(r).direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(2).scroller_width(2))))
        .padding(Padding::new(6.0).bottom(0.0))
        .into()
}

fn toolbar(tab: &Tab) -> Element<'_, Message> {
    let nav = row![
        tip_button("←", Cmd::Back, Message::Run(Cmd::Back)),
        tip_button("→", Cmd::Forward, Message::Run(Cmd::Forward)),
        tip_button("↑", Cmd::Up, Message::Run(Cmd::Up)),
    ]
    .spacing(2);
    let id = tab.id;
    let filter = text_input("Filtrar…", &tab.filter)
        .id(tab.filter_id())
        .on_input(move |value| Message::FilterChanged { tab: id, value })
        .on_submit(Message::FilterSubmit)
        .size(13)
        .padding([4, 8])
        .width(200);
    row![nav, breadcrumbs(tab), filter].spacing(8).padding([6, 8]).align_y(Alignment::Center).into()
}

fn breadcrumbs(tab: &Tab) -> Element<'_, Message> {
    let mut acc = PathBuf::new();
    let mut r = Row::new().spacing(0).align_y(Alignment::Center);
    let parts: Vec<_> = tab.path.components().collect();
    for (i, c) in parts.iter().enumerate() {
        acc.push(c.as_os_str());
        let label = match c {
            Component::RootDir if i == 0 => "/".to_string(),
            Component::RootDir => continue,
            Component::Prefix(p) => p.as_os_str().to_string_lossy().into_owned(),
            other => other.as_os_str().to_string_lossy().into_owned(),
        };
        if i > 0 && !matches!(parts[i - 1], Component::RootDir | Component::Prefix(_)) {
            r = r.push(text("›").size(13).style(muted));
        }
        let last = i == parts.len() - 1;
        r = r.push(
            button(text(label).size(13).wrapping(Wrapping::None))
                .padding([3, 6])
                .on_press(Message::NavigateIn { tab: tab.id, path: acc.clone() })
                .style(move |t: &Theme, s| crumb(t, s, last)),
        );
    }
    let bar = scrollable(r)
        .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(2).scroller_width(2)))
        .anchor_right()
        .width(Fill);
    let goto = mouse_area(container(bar).width(Fill).padding([2, 4]).style(|t: &Theme| container::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        border: border::rounded(6),
        ..Default::default()
    }))
    .on_double_click(Message::Run(Cmd::GoTo));
    goto.into()
}

fn body<'a>(app: &'a App, tab: &'a Tab) -> Element<'a, Message> {
    if let Some(err) = &tab.error {
        return center(text(format!("Não foi possível abrir a pasta:\n{err}")).style(danger_text)).into();
    }
    if tab.view.is_empty() {
        let msg = if tab.loading {
            "Carregando…"
        } else if !tab.filter.is_empty() {
            "Nada casa com o filtro"
        } else {
            "Pasta vazia"
        };
        return column![if tab.grid { space().into() } else { header(tab) }, center(text(msg).style(muted))].into();
    }
    if tab.grid {
        grid(app, tab)
    } else {
        column![header(tab), list(tab)].into()
    }
}

fn header(tab: &Tab) -> Element<'_, Message> {
    let id = tab.id;
    let col = |label: &'static str, key: SortKey, width: Length| -> Element<'_, Message> {
        let arrow = if tab.sort.key == key {
            if tab.sort.desc {
                " ↓"
            } else {
                " ↑"
            }
        } else {
            ""
        };
        button(text(format!("{label}{arrow}")).size(12).style(muted))
            .padding([2, 0])
            .width(width)
            .on_press(Message::SortBy { tab: id, key })
            .style(ghost)
            .into()
    };
    container(
        row![
            space().width(34),
            col("Nome", SortKey::Name, Fill),
            col("Tipo", SortKey::Kind, Length::Fixed(60.0)),
            col("Tamanho", SortKey::Size, Length::Fixed(84.0)),
            col("Modificado", SortKey::Modified, Length::Fixed(124.0)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([2, 12])
    .into()
}

fn list(tab: &Tab) -> Element<'_, Message> {
    let total = tab.view.len();
    let first = ((tab.scroll_y / ROW_H).floor() as usize).saturating_sub(OVERSCAN).min(total);
    let end = (first + (tab.viewport_h / ROW_H).ceil() as usize + 2 * OVERSCAN).min(total);
    let mut col = Column::new().push(space().height(first as f32 * ROW_H));
    for i in first..end {
        col = col.push(list_row(tab, i));
    }
    col = col.push(space().height((total - end) as f32 * ROW_H));
    let id = tab.id;
    scrollable(col.padding([0, 6]))
        .id(tab.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled { tab: id, viewport })
        .height(Fill)
        .into()
}

fn list_row(tab: &Tab, i: usize) -> Element<'_, Message> {
    let e = tab.entry_at(i).expect("índice válido");
    let selected = tab.selected.contains(&e.path);
    let cursor = i == tab.cursor;
    let dim = e.hidden;
    let line = row![
        badge(e),
        text(e.name.as_str()).size(13).wrapping(Wrapping::None).width(Fill).style(move |t: &Theme| name_style(t, dim)),
        text(kind_label(e)).size(12).width(60).wrapping(Wrapping::None).style(muted),
        text(if e.is_dir { String::new() } else { human_size(e.size) }).size(12).width(84).style(muted),
        text(human_time(e.modified)).size(12).width(124).wrapping(Wrapping::None).style(muted),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let cell = container(line)
        .height(ROW_H)
        .padding([0, 6])
        .align_y(Alignment::Center)
        .clip(true)
        .style(move |t: &Theme| row_style(t, selected, cursor));
    mouse_area(cell).on_press(Message::Click { tab: tab.id, index: i }).on_double_click(Message::Activate { tab: tab.id, index: i }).into()
}

fn grid<'a>(app: &'a App, tab: &'a Tab) -> Element<'a, Message> {
    let cols = app.grid_cols();
    let total = tab.view.len();
    let rows_total = total.div_ceil(cols);
    let first_row = ((tab.scroll_y / CELL_H).floor() as usize).saturating_sub(2).min(rows_total);
    let end_row = (first_row + (tab.viewport_h / CELL_H).ceil() as usize + 4).min(rows_total);
    let mut col = Column::new().push(space().height(first_row as f32 * CELL_H));
    for r in first_row..end_row {
        let mut line = Row::new();
        for i in r * cols..((r + 1) * cols).min(total) {
            line = line.push(grid_cell(app, tab, i));
        }
        col = col.push(line);
    }
    col = col.push(space().height((rows_total - end_row) as f32 * CELL_H));
    let id = tab.id;
    scrollable(col.padding([4, 8]))
        .id(tab.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled { tab: id, viewport })
        .height(Fill)
        .into()
}

fn grid_cell<'a>(app: &'a App, tab: &'a Tab, i: usize) -> Element<'a, Message> {
    let e = tab.entry_at(i).expect("índice válido");
    let selected = tab.selected.contains(&e.path);
    let cursor = i == tab.cursor;
    let thumb: Element<_> = match app.thumbs.get(&e.path) {
        Some(Some(h)) => image(h.clone()).content_fit(ContentFit::Contain).width(112).height(100).into(),
        _ => center(big_badge(e)).width(112).height(100).into(),
    };
    let label = text(shorten(&e.name, 34)).size(12).align_x(Alignment::Center).width(Fill);
    let cell = container(column![thumb, label].spacing(4).align_x(Alignment::Center))
        .width(CELL_W - 4.0)
        .height(CELL_H - 4.0)
        .padding(6)
        .clip(true)
        .style(move |t: &Theme| row_style(t, selected, cursor));
    let cell = container(cell).padding(2);
    mouse_area(cell).on_press(Message::Click { tab: tab.id, index: i }).on_double_click(Message::Activate { tab: tab.id, index: i }).into()
}

fn status_line(tab: &Tab, show_hidden: bool) -> Element<'_, Message> {
    let mut parts = vec![format!("{} itens", tab.view.len())];
    if !tab.filter.is_empty() {
        let total = tab.entries.iter().filter(|e| show_hidden || !e.hidden).count();
        parts[0] = format!("{} de {} itens", tab.view.len(), total);
    }
    if !tab.selected.is_empty() {
        let bytes: u64 = tab.entries.iter().filter(|e| tab.selected.contains(&e.path)).map(|e| e.size).sum();
        parts.push(format!("{} selecionado(s), {}", tab.selected.len(), human_size(bytes)));
    }
    if tab.loading {
        parts.push(format!("carregando… {}", tab.entries.len()));
    }
    if tab.grid {
        parts.push("miniaturas".into());
    }
    container(text(parts.join("  ·  ")).size(12).style(muted)).padding([4, 12]).into()
}

// ---------- rodapé: operações, avisos e dicas ----------

fn bottom_bar(app: &App) -> Element<'_, Message> {
    let mut items = Column::new().spacing(4);
    for job in &app.jobs {
        let pct = if job.total > 0 { job.done as f32 / job.total as f32 } else { 0.0 };
        let detail = if job.total > 1000 {
            format!("{} de {}  {}", human_size(job.done), human_size(job.total), job.current)
        } else {
            job.current.clone()
        };
        items = items.push(
            row![
                text(job.label.as_str()).size(12).width(Length::FillPortion(2)).wrapping(Wrapping::None),
                progress_bar(0.0..=1.0, pct).length(Fill).girth(6),
                text(detail).size(11).style(muted).width(Length::FillPortion(2)).wrapping(Wrapping::None),
                button(text("Cancelar").size(11)).padding([2, 8]).on_press(Message::CancelJob(job.id)).style(ghost),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
    }
    let hint = if let Some(t) = &app.toast {
        let err = t.error;
        text(t.text.as_str()).size(12).style(move |th: &Theme| if err { danger_text(th) } else { text::Style { color: None } })
    } else {
        text(format!(
            "Espaço preview · {} paleta · F3 painel duplo · F5 copiar · F6 mover · F2 renomear · Delete lixeira · {} desfazer · digite para filtrar",
            Cmd::Palette.shortcut(),
            Cmd::Undo.shortcut()
        ))
        .size(11)
        .style(muted)
    };
    items = items.push(hint);
    container(items).padding([6, 12]).width(Fill).into()
}

// ---------- sobreposições ----------

fn overlay(app: &App) -> Option<Element<'_, Message>> {
    let card: Element<_> = match &app.overlay {
        Overlay::None => return None,
        Overlay::Palette { query, selected } => {
            let card = palette(app, query, *selected);
            let backdrop = dim_backdrop();
            return Some(stack![backdrop, container(opaque(card)).center_x(Fill).padding(Padding::new(0.0).top(90.0))].into());
        }
        Overlay::Prompt { kind, value } => {
            let input = text_input("", value)
                .id(PROMPT_INPUT)
                .on_input(Message::PromptChanged)
                .on_submit(Message::PromptSubmit)
                .padding(8)
                .size(14);
            panel(
                column![text(kind.title()).size(14), input, text("Enter confirma · Esc cancela").size(11).style(muted)].spacing(10),
                460.0,
            )
        }
        Overlay::Preview { path, content } => preview(app, path, content.as_ref()),
    };
    Some(stack![dim_backdrop(), center(opaque(card)).padding(40)].into())
}

fn dim_backdrop<'a>() -> Element<'a, Message> {
    mouse_area(
        container(space())
            .width(Fill)
            .height(Fill)
            .style(|_| container::Style { background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.45).into()), ..Default::default() }),
    )
    .on_press(Message::CloseOverlay)
    .into()
}

fn palette<'a>(app: &'a App, query: &'a str, selected: usize) -> Element<'a, Message> {
    let input = text_input("Digite um comando…", query)
        .id(PALETTE_INPUT)
        .on_input(Message::PaletteQuery)
        .on_submit(Message::PaletteRun(usize::MAX))
        .padding(10)
        .size(15);
    let mut list = Column::new().spacing(1);
    let start = selected.saturating_sub(13);
    for (i, cmd) in app.palette_view().into_iter().enumerate().skip(start).take(14) {
        let active = i == selected;
        let line = row![text(cmd.label()).size(13).width(Fill), text(cmd.shortcut()).size(12).style(muted)].align_y(Alignment::Center);
        list = list.push(
            button(line).width(Fill).padding([6, 10]).on_press(Message::PaletteRun(i)).style(move |t: &Theme, s| side_button(t, s, active)),
        );
    }
    panel(column![input, list].spacing(8), 560.0)
}

fn preview<'a>(app: &'a App, path: &'a Path, content: Option<&'a Preview>) -> Element<'a, Message> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let max_h = (app.window.height - 160.0).max(200.0);
    let max_w = (app.window.width - 160.0).clamp(300.0, 1400.0);
    let info_table = |info: &'a [(String, String)]| -> Element<'a, Message> {
        let mut c = Column::new().spacing(3);
        for (k, v) in info {
            c = c.push(row![text(k.as_str()).size(12).style(muted).width(110), text(v.as_str()).size(12)].spacing(8));
        }
        scrollable(c).width(260).into()
    };
    let inner: Element<_> = match content {
        None => center(text("Carregando…").style(muted)).height(240).into(),
        Some(Preview::Image { handle, info, .. }) => {
            row![image(handle.clone()).content_fit(ContentFit::ScaleDown).width(Fill).height(Length::Fixed(max_h - 60.0)), info_table(info)]
                .spacing(16)
                .into()
        }
        Some(Preview::Text { content, truncated }) => {
            let mut c = column![text(content.as_str()).font(Font::MONOSPACE).size(12)];
            if *truncated {
                c = c.push(text("… (mostrando os primeiros 64 KB)").size(11).style(muted));
            }
            scrollable(c.padding(8)).height(Length::Fixed(max_h - 60.0)).width(Fill).into()
        }
        Some(Preview::Folder { dirs, files, bytes, more }) => {
            let plus = if *more { "+" } else { "" };
            center(text(format!("{dirs}{plus} pastas · {files}{plus} arquivos · {}{plus} nesta pasta", human_size(*bytes))).size(14))
                .height(160)
                .into()
        }
        Some(Preview::Other { info }) => {
            column![center(text("Sem preview para este tipo").style(muted)).height(120), info_table(info)].into()
        }
        Some(Preview::Error(e)) => center(text(format!("Não foi possível mostrar: {e}")).style(danger_text)).height(160).into(),
    };
    let title = row![text(name).size(14).width(Fill), text("↑↓ navega · Enter abre · Espaço fecha").size(11).style(muted)]
        .align_y(Alignment::Center);
    panel(column![title, inner].spacing(10), max_w)
}

fn panel<'a>(content: impl Into<Element<'a, Message>>, width: f32) -> Element<'a, Message> {
    container(content)
        .padding(16)
        .max_width(width)
        .style(|t: &Theme| {
            let pal = t.extended_palette();
            container::Style {
                background: Some(pal.background.weak.color.into()),
                border: border::rounded(10).width(1).color(pal.background.strong.color),
                shadow: Shadow { color: Color::from_rgba(0.0, 0.0, 0.0, 0.4), offset: Vector::new(0.0, 8.0), blur_radius: 24.0 },
                ..Default::default()
            }
        })
        .into()
}

// ---------- peças pequenas ----------

fn tip_button<'a>(label: &'a str, cmd: Cmd, msg: Message) -> Element<'a, Message> {
    let b = button(text(label).size(13)).padding([3, 9]).on_press(msg).style(ghost);
    let tip_text = format!("{}  {}", cmd.label(), cmd.shortcut());
    tooltip(b, text(tip_text).size(12), tooltip::Position::Bottom).padding(6).style(tip).into()
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
    Image,
    Code,
    Archive,
    Doc,
    Media,
    Other,
}

fn kind(e: &Entry) -> Kind {
    if e.is_dir {
        return Kind::Folder;
    }
    match e.ext.as_str() {
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "c" | "h" | "cpp" | "hpp" | "cs" | "java" | "kt" | "swift" | "rb" | "php"
        | "sh" | "ps1" | "toml" | "json" | "yaml" | "yml" | "xml" | "html" | "css" | "lua" | "sql" | "md" => Kind::Code,
        "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" | "zst" | "iso" | "dmg" => Kind::Archive,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "txt" | "rtf" | "epub" => Kind::Doc,
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "mp4" | "mkv" | "mov" | "avi" | "webm" => Kind::Media,
        _ if crate::media::is_image(&e.path) => Kind::Image,
        _ => Kind::Other,
    }
}

fn kind_color(k: Kind) -> Color {
    match k {
        Kind::Folder => Color::from_rgb8(0xE8, 0xB3, 0x39),
        Kind::Image => Color::from_rgb8(0x4C, 0xB8, 0x7A),
        Kind::Code => Color::from_rgb8(0x5B, 0x9B, 0xF0),
        Kind::Archive => Color::from_rgb8(0xA3, 0x7B, 0xE0),
        Kind::Doc => Color::from_rgb8(0xE0, 0x6C, 0x5B),
        Kind::Media => Color::from_rgb8(0xE0, 0x7B, 0xB8),
        Kind::Other => Color::from_rgb8(0x8A, 0x93, 0xA0),
    }
}

fn badge(e: &Entry) -> Element<'_, Message> {
    let k = kind(e);
    let label = match k {
        Kind::Folder => String::new(),
        _ => e.ext.chars().take(3).collect::<String>().to_uppercase(),
    };
    let link = e.is_link;
    container(text(label).size(8).color(Color::WHITE))
        .width(26)
        .height(if matches!(k, Kind::Folder) { 16 } else { 18 })
        .center_x(26)
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(Background::Color(kind_color(k))),
            border: border::rounded(if matches!(k, Kind::Folder) { 3 } else { 4 }).width(if link { 1 } else { 0 }).color(Color::WHITE),
            ..Default::default()
        })
        .into()
}

fn big_badge(e: &Entry) -> Element<'_, Message> {
    let k = kind(e);
    let label = if e.is_dir { String::new() } else { e.ext.chars().take(4).collect::<String>().to_uppercase() };
    container(text(label).size(14).color(Color::WHITE))
        .width(64)
        .height(if e.is_dir { 50 } else { 72 })
        .center_x(64)
        .align_y(Alignment::Center)
        .style(move |_| container::Style { background: Some(kind_color(k).into()), border: border::rounded(8), ..Default::default() })
        .into()
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max - 1).collect();
        format!("{head}…")
    }
}

// ---------- estilos ----------

fn muted(t: &Theme) -> text::Style {
    let pal = t.extended_palette();
    text::Style { color: Some(Color { a: 0.6, ..pal.background.base.text }) }
}

fn danger_text(t: &Theme) -> text::Style {
    text::Style { color: Some(t.extended_palette().danger.base.color) }
}

fn name_style(t: &Theme, dim: bool) -> text::Style {
    if dim {
        muted(t)
    } else {
        text::Style { color: None }
    }
}

fn row_style(t: &Theme, selected: bool, cursor: bool) -> container::Style {
    let pal = t.extended_palette();
    let bg = if selected {
        Some(Color { a: 0.35, ..pal.primary.base.color })
    } else if cursor {
        Some(pal.background.weak.color)
    } else {
        None
    };
    container::Style {
        background: bg.map(Background::Color),
        border: border::rounded(5).width(if cursor { 1 } else { 0 }).color(Color { a: 0.7, ..pal.primary.base.color }),
        ..Default::default()
    }
}

fn side_button(t: &Theme, status: button::Status, active: bool) -> button::Style {
    let pal = t.extended_palette();
    let bg = match (active, status) {
        (true, _) => Some(Color { a: 0.3, ..pal.primary.base.color }),
        (_, button::Status::Hovered) => Some(pal.background.weak.color),
        _ => None,
    };
    button::Style {
        background: bg.map(Background::Color),
        text_color: pal.background.base.text,
        border: border::rounded(6),
        ..Default::default()
    }
}

fn tab_button(t: &Theme, status: button::Status, selected: bool, pane_active: bool) -> button::Style {
    let pal = t.extended_palette();
    let bg = if selected {
        Some(pal.background.base.color)
    } else if status == button::Status::Hovered {
        Some(pal.background.weak.color)
    } else {
        None
    };
    let accent = if selected && pane_active { pal.primary.base.color } else { Color::TRANSPARENT };
    button::Style {
        background: bg.map(Background::Color),
        text_color: if selected { pal.background.base.text } else { Color { a: 0.7, ..pal.background.base.text } },
        border: border::rounded(6).width(1).color(accent),
        ..Default::default()
    }
}

fn ghost(t: &Theme, status: button::Status) -> button::Style {
    let pal = t.extended_palette();
    let bg = matches!(status, button::Status::Hovered | button::Status::Pressed).then_some(pal.background.weak.color);
    button::Style {
        background: bg.map(Background::Color),
        text_color: pal.background.base.text,
        border: border::rounded(5),
        ..Default::default()
    }
}

fn crumb(t: &Theme, status: button::Status, last: bool) -> button::Style {
    let mut s = ghost(t, status);
    if !last {
        s.text_color = Color { a: 0.75, ..s.text_color };
    }
    s
}

fn tip(t: &Theme) -> container::Style {
    let pal = t.extended_palette();
    container::Style {
        background: Some(pal.background.strong.color.into()),
        text_color: Some(pal.background.strong.text),
        border: border::rounded(5),
        ..Default::default()
    }
}

pub fn theme(app: &App) -> Theme {
    use iced::theme::Palette;
    if app.dark {
        Theme::custom(
            "Arbor Escuro",
            Palette {
                background: Color::from_rgb8(0x16, 0x18, 0x1D),
                text: Color::from_rgb8(0xE4, 0xE6, 0xEB),
                primary: Color::from_rgb8(0x4F, 0x9D, 0x7E),
                success: Color::from_rgb8(0x4C, 0xB8, 0x7A),
                warning: Color::from_rgb8(0xE8, 0xB3, 0x39),
                danger: Color::from_rgb8(0xE0, 0x6C, 0x5B),
            },
        )
    } else {
        Theme::custom(
            "Arbor Claro",
            Palette {
                background: Color::from_rgb8(0xF6, 0xF7, 0xF9),
                text: Color::from_rgb8(0x1E, 0x22, 0x28),
                primary: Color::from_rgb8(0x2F, 0x7D, 0x5E),
                success: Color::from_rgb8(0x2E, 0x9E, 0x5E),
                warning: Color::from_rgb8(0xC2, 0x8A, 0x10),
                danger: Color::from_rgb8(0xC0, 0x45, 0x35),
            },
        )
    }
}
