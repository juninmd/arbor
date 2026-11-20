//! Interface. A lista é virtualizada: só as linhas visíveis (mais uma margem) viram widgets,
//! então uma pasta com 200 mil arquivos custa o mesmo que uma com 50.
//!
//! O visual segue o protótipo "arbor Glass" (tokens em `theme.rs`): aurora ao fundo, cartões de vidro,
//! ficha de cor por tipo, pílula de seleção e velocidade visível ("N itens em X ms").

use crate::app::{App, JumpKind, Message, Overlay, Pane, PromptKind, Tab, UpdateState, JUMP_INPUT, PALETTE_INPUT, PROMPT_INPUT, ROW_H};
use crate::columns::Col;
use crate::commands::Cmd;
use crate::computer::{self, Drive, Media};
use crate::media::Preview;
use crate::theme::{self, icon, Icon};
use crate::update::{self, Progress};
use crate::view_modes::Shape;
use arbor_core::{human_count, human_size, human_time, Entry, SortKey};
use iced::widget::text::Wrapping;
use iced::widget::{
    button, center, column, container, image, mouse_area, opaque, progress_bar, row, scrollable, space, stack, text, text_input, tooltip,
    Column, MouseArea, Row,
};
use iced::{Alignment, Color, ContentFit, Element, Fill, Length, Padding, Theme};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

mod chrome_view;
mod commands_view;
mod computer_view;
mod crumbs;
mod details_view;
mod home_view;
mod sidebar_view;

mod props_view;

const OVERSCAN: usize = 8;
/// Alça entre a área de arquivos e o painel de detalhes: 1 px visível, 7 px para pegar com o mouse.
pub const SPLIT_W: f32 = 7.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let panes: Element<_> = if app.dual { row![pane(app, 0), separator_v(), pane(app, 1)].into() } else { pane(app, app.active) };
    let mut body = Row::new().height(Fill);
    if app.vertical_tabs {
        body = body.push(chrome_view::vertical_tabs(app)).push(separator_v());
    }
    body = body.push(sidebar_view::sidebar(app)).push(separator_v()).push(container(panes).width(Fill).height(Fill));
    // o painel desliza: largura cresce de 0 até a escolhida, com o conteúdo já na largura final (sem refluxo)
    let open = app.motion.details(Instant::now());
    if app.details_on || open > 0.0 {
        let w = (app.details_w * open).round();
        body = body.push(details_split()).push(container(details_view::panel(app)).width(w).height(Fill).clip(true));
    }
    // faixa da cor da aba ativa: endereço (quando há um painel só) e a barra de comandos
    let mut band = Column::new();
    if !app.dual {
        band = band.push(toolbar(app, app.tab()));
    }
    let band = container(band.push(commands_view::bar(app))).width(Fill).style(theme::surface);
    let main: Element<_> = column![chrome_view::titlebar(app), band, separator(), body].width(Fill).height(Fill).into();
    let main: Element<_> = match notices(app) {
        Some(n) => {
            // sobe 12 px ao aparecer
            let rise = 12.0 * (1.0 - app.motion.notices(Instant::now()));
            stack![main, container(n).align_right(Fill).align_bottom(Fill).padding(Padding::new(14.0).bottom(14.0 - rise).top(14.0 + rise))]
                .into()
        }
        None => main,
    };
    // a aurora fica atrás de tudo; os cartões de vidro deixam ela (e o vidro do sistema) aparecer
    let aurora = image(app.aurora.clone()).width(Fill).height(Fill).content_fit(ContentFit::Fill);
    let mut layers = stack![aurora, main];
    if crate::window_chrome::custom() {
        if !app.maximized {
            layers = layers.push(chrome_view::window_outline());
        }
        layers = layers.push(chrome_view::resize_edges());
    }
    if let Some(o) = overlay(app) {
        layers = layers.push(o);
    }
    if let Some(g) = drag_ghost(app) {
        layers = layers.push(g);
    }
    layers.into()
}

pub fn theme(app: &App) -> Theme {
    theme::theme(app.palette)
}

/// Com vidro nativo o fundo da janela é transparente: quem pinta é a aurora translúcida sobre o sistema.
pub fn style(app: &App, t: &Theme) -> iced::theme::Style {
    let k = theme::tokens(t);
    iced::theme::Style { background_color: if app.glass { Color::TRANSPARENT } else { k.void }, text_color: k.ink }
}

// ---------- barra lateral ----------

fn place_icon(label: &str) -> (Icon, Color) {
    match label {
        "Pasta pessoal" => (Icon::Folder, theme::ICE),
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
    let mut body = mouse_area(body(app, tab, is_active)).on_right_press(Message::ContextMenu { tab: tab.id, index: None });
    if app.dragging().is_some() {
        body = body.on_enter(Message::DragOver(Some(tab.path.clone())));
    }
    // com dois painéis cada um tem as próprias abas (se não estiverem na coluna) e o próprio endereço
    let mut content = Column::new();
    if app.dual {
        if !app.vertical_tabs {
            content = content.push(tabs_bar(p, index, is_active));
        }
        content = content.push(toolbar(app, tab));
    }
    let framed =
        container(content.push(body)).width(Fill).height(Fill).clip(true).style(move |t: &Theme| theme::glass(t, is_active && app.dual));
    mouse_area(framed).on_press(Message::FocusPane(index)).into()
}

fn tabs_bar(p: &Pane, pane: usize, pane_active: bool) -> Element<'_, Message> {
    let mut r = Row::new().spacing(4).align_y(Alignment::Center);
    for (i, tab) in p.tabs.iter().enumerate() {
        let selected = i == p.active;
        let color = if selected { theme::ICE } else { theme::now().ink3 };
        let ic = if tab.locked { Icon::Lock } else { Icon::Folder };
        let mut label = row![icon(ic, 14.0, color), text(shorten(&tab.title(), 24)).size(12).wrapping(Wrapping::None)]
            .spacing(7)
            .align_y(Alignment::Center);
        if p.tabs.len() > 1 {
            label =
                label.push(button(text("×").size(13)).padding([0, 5]).on_press(Message::CloseTabAt { pane, index: i }).style(theme::ghost));
        }
        let b = button(label)
            .padding([6, 11])
            .on_press(Message::SelectTab { pane, index: i })
            .style(move |t: &Theme, s| theme::tab(t, s, selected, pane_active));
        r = r.push(mouse_area(b).on_right_press(Message::TabMenu { pane, index: i }));
    }
    r = r.push(icon_button(Icon::Plus, Cmd::NewTab, Message::NewTabIn(pane)));
    container(scrollable(r).direction(thin_horizontal()).style(theme::scrollbar)).padding(Padding::new(8.0).bottom(0.0)).into()
}

fn toolbar<'a>(app: &'a App, tab: &'a Tab) -> Element<'a, Message> {
    let nav = row![
        mouse_area(icon_button(Icon::Left, Cmd::Back, Message::Run(Cmd::Back)))
            .on_right_press(Message::JumpOpen { kind: JumpKind::Back, dir: None }),
        mouse_area(icon_button(Icon::Right, Cmd::Forward, Message::Run(Cmd::Forward)))
            .on_right_press(Message::JumpOpen { kind: JumpKind::Forward, dir: None }),
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
    let filter =
        stack![filter, container(icon(Icon::Search, 13.0, theme::now().ink3)).padding(Padding::new(0.0).left(10.0)).center_y(Fill)];
    let filter = container(filter).height(30);
    let bar = row![nav, breadcrumbs(app, tab)].spacing(8).align_y(Alignment::Center);
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
        button(icon(ic, 14.0, if on { theme::now().ink } else { theme::now().ink3 }))
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

fn breadcrumbs<'a>(app: &App, tab: &'a Tab) -> Element<'a, Message> {
    let mut acc = PathBuf::new();
    let mut segments: Vec<(String, PathBuf)> = Vec::new();
    let mut r = Row::new().spacing(1).align_y(Alignment::Center);
    let in_computer = computer::is(&tab.path);
    // Como no Explorer: todo caminho começa em "Este Computador".
    if let Some(title) = [
        (crate::trashbin::is(&tab.path), crate::trashbin::TITLE),
        (crate::home::is(&tab.path), crate::home::TITLE),
        (crate::discovery::is(&tab.path), crate::discovery::TITLE),
    ]
    .into_iter()
    .find_map(|(hit, t)| hit.then_some(t))
    {
        r = r.push(crumb(tab.id, title.into(), tab.path.clone(), true));
        return container(r).width(Fill).into();
    }
    // Caminhos \\servidor começam em "Rede", como no Explorer.
    let unc = crate::network::server_root(&tab.path).is_some() || crate::network::share_parent(&tab.path).is_some();
    let (root_title, root) = if unc { (crate::discovery::TITLE, crate::discovery::path()) } else { (computer::TITLE, computer::path()) };
    r = r.push(crumb(tab.id, root_title.into(), root.clone(), in_computer));
    let parts: Vec<_> = if in_computer { vec![] } else { tab.path.components().collect() };
    r = r.push(chevron(root));
    for (i, c) in parts.iter().enumerate() {
        acc.push(c.as_os_str());
        let label = match c {
            Component::RootDir if i == 0 => "/".to_string(),
            Component::RootDir => continue,
            Component::Prefix(p) => p.as_os_str().to_string_lossy().into_owned(),
            other => other.as_os_str().to_string_lossy().into_owned(),
        };
        // No Windows "C:" e "\\" formam um só degrau: o "›" vem depois da raiz.
        if matches!(c, Component::Prefix(_)) && matches!(parts.get(i + 1), Some(Component::RootDir)) {
            acc.push(parts[i + 1].as_os_str());
        }
        segments.push((label, acc.clone()));
    }
    // Largura que sobra para os degraus: painel menos botões, filtro, selo de tempo e "Este Computador".
    // Um painel: a faixa ocupa a janela toda. Dois: cada painel tem a sua metade da área de arquivos.
    let width = if app.dual {
        let vtabs = if app.vertical_tabs { crate::app::VTABS_W + 1.0 } else { 0.0 };
        let details = if app.details_on { app.details_w + SPLIT_W } else { 0.0 };
        (app.window.width - crate::app::SIDEBAR_W - 2.0 - vtabs - details) / 2.0
    } else {
        app.window.width
    };
    let filter_w = if tab.filter.is_empty() { 150.0 } else { 210.0 };
    let budget = width - 230.0 - filter_w - crumbs::crumb_width(computer::TITLE);
    let widths: Vec<f32> = segments.iter().map(|(l, _)| crumbs::crumb_width(l)).collect();
    let first = crumbs::first_visible(&widths, budget - 30.0);
    if first > 0 {
        let hidden = segments[first - 1].1.clone();
        r = r.push(
            button(text("«").size(13).style(theme::ink3))
                .padding([4, 6])
                .on_press(Message::JumpOpen { kind: JumpKind::Ancestors, dir: Some(hidden) })
                .style(theme::ghost),
        );
    }
    let last = segments.len().saturating_sub(1);
    for (i, (label, path)) in segments.into_iter().enumerate().skip(first) {
        r = r.push(crumb(tab.id, label, path.clone(), i == last));
        r = r.push(chevron(path));
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
    if tab.view.is_empty() && !crate::home::is(&tab.path) {
        let (title, hint) = if tab.loading {
            ("Lendo a pasta…", "")
        } else if !tab.filter.is_empty() {
            ("Nada corresponde ao filtro", "Esc limpa o filtro")
        } else if crate::discovery::is(&tab.path) {
            ("Nenhum computador encontrado", "Digite \\\\nome na barra de endereço (Ctrl+L) para abrir um servidor")
        } else {
            ("Pasta vazia", "F7 cria uma nova pasta aqui")
        };
        let empty = center(
            column![text(title).font(theme::DISPLAY).size(14).style(theme::ink2), text(hint).size(12).style(theme::ink3)]
                .spacing(6)
                .align_x(Alignment::Center),
        );
        return column![if tab.grid { space().into() } else { header(app, tab) }, empty].into();
    }
    if crate::home::is(&tab.path) {
        home_view::body(app, tab, pane_active)
    } else if tab.grid {
        grid(app, tab, pane_active)
    } else if computer::is(&tab.path) {
        computer_view::body(app, tab, pane_active)
    } else {
        column![header(app, tab), list(app, tab, pane_active)].into()
    }
}

fn header<'a>(app: &'a App, tab: &'a Tab) -> Element<'a, Message> {
    let id = tab.id;
    let disks = computer::is(&tab.path);
    let cols = &app.columns;
    let col = |label: &'static str, key: SortKey, width: Length, right: bool| -> Element<'a, Message> {
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
    let modified = if disks {
        "Livre"
    } else if crate::trashbin::is(&tab.path) {
        "Excluído em"
    } else {
        "Modificado"
    };
    let specs = [
        (Col::Kind, "Tipo", SortKey::Kind, false),
        (Col::Size, if disks { "Total" } else { "Tamanho" }, SortKey::Size, true),
        (Col::Modified, modified, SortKey::Modified, false),
    ];
    let mut r = row![space().width(34), col("Nome", SortKey::Name, Fill, false)].align_y(Alignment::Center);
    for (c, label, key, right) in specs.into_iter().filter(|s| cols.shown(s.0)) {
        // Alça de 10 px entre as colunas (o espaçamento): arrastar muda a largura da coluna à direita.
        let grip = mouse_area(container(text("│").size(10).style(theme::ink3)).width(10).center_x(10))
            .interaction(iced::mouse::Interaction::ResizingHorizontally)
            .on_press(Message::ColResizeStart(c));
        r = r.push(grip).push(col(label, key, Length::Fixed(cols.width(c)), right));
    }
    mouse_area(container(r).padding([2, 18])).on_right_press(Message::HeaderMenu).into()
}

fn list<'a>(app: &'a App, tab: &'a Tab, pane_active: bool) -> Element<'a, Message> {
    let total = tab.row_count();
    let first = ((tab.scroll_y / ROW_H).floor() as usize).saturating_sub(OVERSCAN).min(total);
    let end = (first + (tab.viewport_h / ROW_H).ceil() as usize + 2 * OVERSCAN).min(total);
    let mut col = Column::new().push(space().height(first as f32 * ROW_H));
    for r in first..end {
        col = col.push(match tab.item_at_row(r) {
            Some(i) => list_row(app, tab, i, pane_active),
            None => group_header(tab, r),
        });
    }
    col = col.push(space().height((total - end) as f32 * ROW_H));
    let id = tab.id;
    let content = surface(app, tab, col.padding(Padding::from([0, 8]).bottom(8.0)), total as f32 * ROW_H + 8.0);
    scrollable(content)
        .id(tab.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled { tab: id, viewport })
        .direction(thin_vertical())
        .height(Fill)
        .style(theme::scrollbar)
        .into()
}

/// Cabeçalho de grupo como no Explorer: rótulo, contagem e um fio até a borda.
fn group_header<'a>(tab: &'a Tab, row: usize) -> Element<'a, Message> {
    let Some(crate::app::groups::Row::Header { label, count }) = tab.grouped.rows.get(row) else { return space().height(ROW_H).into() };
    let rule = container(space().height(1))
        .width(Fill)
        .style(|t: &Theme| container::Style { background: Some(theme::alpha(theme::tokens(t).ink, 0.12).into()), ..Default::default() });
    let line = row![
        text(label.as_str()).size(12.5).font(theme::UI_BOLD).style(theme::accent),
        text(format!("({count})")).size(11).style(theme::ink3),
        rule
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    container(line).height(ROW_H).padding([0, 12]).align_y(Alignment::Center).into()
}

fn list_row<'a>(app: &'a App, tab: &'a Tab, i: usize, pane_active: bool) -> Element<'a, Message> {
    let e = tab.entry_at(i).expect("índice válido");
    let drop_here = e.is_dir && app.dragging().is_some_and(|(_, _, over)| over == Some(e.path.as_path()));
    let drive = drive_of(app, tab, e);
    let (size, when) = match drive {
        Some(d) => d.space.map(|(free, total)| (human_size(total), human_size(free))).unwrap_or_default(),
        None => (if e.is_dir && !tab.dir_sizes { String::new() } else { human_size(e.size) }, human_time(e.modified)),
    };
    let selected = tab.selected.contains(&e.path);
    let cursor = i == tab.cursor;
    let dim = e.hidden;
    let strong = selected || cursor;
    let tint = crate::colors::of(&e.name);
    let name =
        text(shown_name(app, e)).size(13).wrapping(Wrapping::None).width(Fill).font(if strong { theme::UI_BOLD } else { theme::UI }).style(
            move |t: &Theme| text::Style {
                color: Some(theme::alpha(tint.unwrap_or(theme::tokens(t).ink), if dim { theme::HIDDEN_ALPHA } else { 1.0 })),
            },
        );
    // Na busca, a pasta de cada resultado (relativa à pasta buscada) vai embaixo do nome.
    // Na busca no computador o resultado pode estar em qualquer lugar: vai a pasta inteira.
    let global = tab.search.as_ref().is_some_and(|(q, _)| crate::global_search::query_of(q).is_some());
    let place = e.path.parent().and_then(|p| if global { Some(p) } else { p.strip_prefix(&tab.path).ok() });
    let name: Element<_> = match (&tab.search, place) {
        (Some(_), Some(rel)) => {
            let rel = if rel.as_os_str().is_empty() { ".".to_string() } else { rel.to_string_lossy().into_owned() };
            column![name, text(rel).size(10.5).font(theme::MONO).wrapping(Wrapping::None).style(theme::ink3)].width(Fill).into()
        }
        _ => name.into(),
    };
    let cols = &app.columns;
    let repo = e.is_dir && tab.repos.contains(&e.path);
    let mut line = row![entry_chip(entry_kind(e, drive), 24.0, dim, repo), name].spacing(10).align_y(Alignment::Center);
    if cols.shown(Col::Kind) {
        let kind = if drive.is_some() { "Disco".into() } else { kind_label(e) };
        line = line.push(text(kind).size(11.5).width(cols.width(Col::Kind)).wrapping(Wrapping::None).style(theme::ink3));
    }
    if cols.shown(Col::Size) {
        line = line.push(text(size).font(theme::MONO).size(11).width(cols.width(Col::Size)).align_x(Alignment::End).style(theme::ink3));
    }
    if cols.shown(Col::Modified) {
        line =
            line.push(text(when).font(theme::MONO).size(11).width(cols.width(Col::Modified)).wrapping(Wrapping::None).style(theme::ink3));
    }
    let cell = container(line)
        .height(ROW_H)
        .padding([0, 10])
        .align_y(Alignment::Center)
        .clip(true)
        .style(move |t: &Theme| theme::row(t, selected || drop_here, cursor || drop_here, pane_active));
    drop_zone(app, tab, e, mouse_area(cell))
        .on_press(Message::PressItem { tab: tab.id, index: i })
        .on_double_click(Message::Activate { tab: tab.id, index: i })
        .on_right_press(Message::ContextMenu { tab: tab.id, index: Some(i) })
        .into()
}

fn grid<'a>(app: &'a App, tab: &'a Tab, pane_active: bool) -> Element<'a, Message> {
    let cols = app.grid_cols();
    let total = tab.view.len();
    let rows_total = total.div_ceil(cols);
    let cell_h = app.cell_h();
    let first_row = ((tab.scroll_y / cell_h).floor() as usize).saturating_sub(2).min(rows_total);
    let end_row = (first_row + (tab.viewport_h / cell_h).ceil() as usize + 4).min(rows_total);
    let mut col = Column::new().push(space().height(first_row as f32 * cell_h));
    for r in first_row..end_row {
        let mut line = Row::new();
        for i in r * cols..((r + 1) * cols).min(total) {
            line = line.push(grid_cell(app, tab, i, pane_active));
        }
        col = col.push(line);
    }
    col = col.push(space().height((rows_total - end_row) as f32 * cell_h));
    let id = tab.id;
    let content = surface(app, tab, col.padding([4, 10]), rows_total as f32 * cell_h + 8.0);
    scrollable(content)
        .id(tab.scroll_id())
        .on_scroll(move |viewport| Message::Scrolled { tab: id, viewport })
        .direction(thin_vertical())
        .height(Fill)
        .style(theme::scrollbar)
        .into()
}

/// Conteúdo rolável da lista/grade: área vazia que inicia o retângulo de seleção (preenchendo até o
/// fim da janela, para dar onde clicar) e o retângulo desenhado por cima.
fn surface<'a>(app: &'a App, tab: &'a Tab, col: Column<'a, Message>, height: f32) -> Element<'a, Message> {
    let filler = (tab.viewport_h - height).max(0.0);
    let col = col.push(space().height(filler));
    let id = tab.id;
    let mut layer: Element<_> = col.into();
    if let Some(r) = app.band_rect(id) {
        let band = container(space().width(r.width).height(r.height)).style(theme::band);
        layer = stack![layer, container(band).padding(Padding::new(0.0).left(r.x).top(r.y))].into();
    }
    let area = mouse_area(layer).on_press(Message::PressEmpty(id));
    if app.band_rect(id).is_some() || matches!(app.gesture, crate::app::Gesture::Band { tab, .. } if tab == id) {
        area.on_move(move |at| Message::BandMove { tab: id, at }).into()
    } else {
        area.into()
    }
}

/// Durante um arraste, passar por cima de uma pasta a marca como destino; arquivo vale a pasta atual.
fn drop_zone<'a>(app: &App, tab: &Tab, e: &Entry, area: MouseArea<'a, Message>) -> MouseArea<'a, Message> {
    if app.dragging().is_none() {
        return area;
    }
    let target = if e.is_dir { e.path.clone() } else { tab.path.clone() };
    // Ao sair, só volta para a pasta atual se este item ainda era o destino: a próxima linha pode já ter
    // marcado o dela na mesma passada de eventos.
    area.on_enter(Message::DragOver(Some(target.clone()))).on_exit(Message::DragLeave { from: target, back: Some(tab.path.clone()) })
}

/// Nome como aparece na lista: sem a extensão quando o usuário as ocultou.
fn shown_name<'a>(app: &App, e: &'a Entry) -> &'a str {
    if app.hide_ext && !e.is_dir {
        stem(&e.name)
    } else {
        &e.name
    }
}

/// "foto.jpg" vira "foto"; ".bashrc" e "arquivo." ficam inteiros, como no Explorer.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => &name[..i],
        _ => name,
    }
}

fn grid_cell<'a>(app: &'a App, tab: &'a Tab, i: usize, pane_active: bool) -> Element<'a, Message> {
    let e = tab.entry_at(i).expect("índice válido");
    let selected = tab.selected.contains(&e.path);
    let cursor = i == tab.cursor;
    let drive = drive_of(app, tab, e);
    let k = entry_kind(e, drive);
    let color = kind_color(k);
    let (dim, repo) = (e.hidden, e.is_dir && tab.repos.contains(&e.path));
    let mode = app.mode();
    let art = mode.art;
    let thumb: Element<_> = match app.thumbs.get(&e.path) {
        // Ícone de programa aparece inteiro; foto e vídeo preenchem o quadro.
        Some(Some(h)) if mode.shape != Shape::Line => image(h.clone())
            .content_fit(if crate::thumbs::program(&e.path) { ContentFit::Contain } else { ContentFit::Cover })
            .width(if mode.shape == Shape::Icon { Fill } else { art.into() })
            .height(art)
            .border_radius(10)
            .opacity(if dim { theme::HIDDEN_ALPHA } else { 1.0 })
            .into(),
        _ if mode.shape == Shape::Line => entry_chip(k, art, dim, repo),
        _ => container(entry_chip(k, art * 0.48, dim, repo))
            .center_x(if mode.shape == Shape::Icon { Fill } else { art.into() })
            .center_y(art)
            .style(move |_| container::Style {
                background: Some(theme::alpha(color, if dim { 0.04 } else { 0.08 }).into()),
                border: iced::border::rounded(10),
                ..Default::default()
            })
            .into(),
    };
    let tint = crate::colors::of(&e.name);
    let label = text(shorten(shown_name(app, e), 30))
        .size(12)
        .width(Fill)
        .wrapping(Wrapping::None)
        .font(if selected { theme::UI_BOLD } else { theme::UI })
        .style(move |t: &Theme| text::Style {
            color: Some(theme::alpha(tint.unwrap_or(theme::tokens(t).ink), if dim { theme::HIDDEN_ALPHA } else { 1.0 })),
        });
    let meta = match drive.and_then(|d| d.space) {
        Some((free, total)) => format!("{} livres de {}", human_size(free), human_size(total)),
        None if e.is_dir => "Pasta".to_string(),
        None => human_size(e.size),
    };
    let meta = text(meta).font(theme::MONO).size(10).style(theme::ink3);
    let body: Element<_> = match mode.shape {
        Shape::Line => row![thumb, label].spacing(8).align_y(Alignment::Center).into(),
        Shape::Tile => {
            // Pasta já diz o tipo no lugar do tamanho; repetir "Pasta" duas vezes só ocupa a linha.
            let mut info = column![label].spacing(3);
            if !e.is_dir || drive.is_some() {
                info = info.push(text(kind_label(e)).size(11).style(theme::ink3));
            }
            row![thumb, info.push(meta)].spacing(10).align_y(Alignment::Center).into()
        }
        Shape::Icon => column![thumb, label, meta].spacing(4).into(),
        Shape::Content => {
            let kind = text(if drive.is_some() { "Disco".into() } else { kind_label(e) }).size(11).style(theme::ink3);
            let when = text(human_time(e.modified)).font(theme::MONO).size(10.5).style(theme::ink3);
            row![thumb, column![label, kind].spacing(3).width(Fill), column![when, meta].spacing(3).align_x(Alignment::End)]
                .spacing(12)
                .align_y(Alignment::Center)
                .into()
        }
    };
    let (pad, radius) = if mode.shape == Shape::Line { ([2, 6], 9.0) } else { ([6, 6], 15.0) };
    let cell = container(body)
        .width(app.cell_w() - 10.0)
        .height(app.cell_h() - if mode.shape == Shape::Line { 4.0 } else { 10.0 })
        .padding(pad)
        .align_y(Alignment::Center)
        .clip(true)
        .style(move |t: &Theme| {
            let mut s = theme::row(t, selected, cursor, pane_active);
            if !selected && !cursor {
                s.background = Some(theme::tokens(t).glass_hi.into());
            }
            s.border.radius = radius.into();
            s
        });
    let cell = container(cell).padding(if mode.shape == Shape::Line { [2, 5] } else { [5, 5] });
    drop_zone(app, tab, e, mouse_area(cell))
        .on_press(Message::PressItem { tab: tab.id, index: i })
        .on_double_click(Message::Activate { tab: tab.id, index: i })
        .on_right_press(Message::ContextMenu { tab: tab.id, index: Some(i) })
        .into()
}

/// Resumo à direita da barra de comandos: itens, seleção e o tempo de listagem (a barra de status de baixo saiu).
fn status<'a>(app: &App, tab: &'a Tab) -> Element<'a, Message> {
    let show_hidden = app.show_hidden;
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
    if let Some(b) = &tab.branch {
        parts.push(format!("⎇ {b}"));
    }
    if app.sync_nav && app.dual {
        parts.push("⇄ sincronizado".into());
    }
    // O modo aparece ao trocar com Ctrl+roda, como o Explorer mostra o botão de visualização ativo.
    if tab.grid {
        parts.push(app.mode().name.to_string());
    }
    let mut r =
        row![text(parts.join("  ·  ")).size(11.5).style(theme::ink3).wrapping(Wrapping::None)].spacing(8).align_y(Alignment::Center);
    if let Some(badge) = latency_badge(tab) {
        r = r.push(badge);
    }
    r.into()
}

/// "›" entre os degraus: clicar lista as subpastas daquele nível, como no Explorer.
fn chevron<'a>(dir: PathBuf) -> Element<'a, Message> {
    button(text("›").size(12).style(theme::ink3))
        .padding([4, 4])
        .on_press(Message::JumpOpen { kind: JumpKind::Children, dir: Some(dir) })
        .style(theme::ghost)
        .into()
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

/// Gráfico de velocidade da cópia: uma barra por meio segundo, escala pelo pico.
fn speed_graph(history: &std::collections::VecDeque<f64>) -> Option<Element<'_, Message>> {
    let peak = history.iter().copied().fold(0.0, f64::max);
    if history.len() < 2 || peak <= 0.0 {
        return None;
    }
    const H: f32 = 16.0;
    let bars = history.iter().map(|&v| {
        let h = (v / peak) as f32 * H;
        container(space()).width(2).height(h.max(1.0)).style(|t: &Theme| container::Style {
            background: Some(theme::alpha(theme::tokens(t).accent, 0.7).into()),
            ..Default::default()
        })
    });
    Some(Row::with_children(bars.map(Element::from)).spacing(1).height(H).align_y(Alignment::End).into())
}

// ---------- avisos: operações, atualização, mensagens e fps, num cartão flutuante só quando há algo ----------

fn notices(app: &App) -> Option<Element<'_, Message>> {
    if app.jobs.is_empty() && app.toast.is_none() && app.fps.is_none() && matches!(app.update, UpdateState::Idle) {
        return None;
    }
    let mut col = Column::new().spacing(8);
    for job in &app.jobs {
        let pct = if job.total > 0 { job.done as f32 / job.total as f32 } else { 0.0 };
        let detail = if job.queued {
            "na fila".to_string()
        } else if job.paused {
            format!("pausado  {}", job.current)
        } else if job.total > 1000 {
            // Como o Explorer: velocidade e tempo restante junto do total.
            let speed = job.rate.per_sec().map(|v| format!("  {}/s", human_size(v as u64))).unwrap_or_default();
            let eta = job.rate.eta(job.done, job.total).map(|d| format!(" · {}", crate::app::rate::human_eta(d))).unwrap_or_default();
            format!("{} de {}{speed}{eta}  {}", human_size(job.done), human_size(job.total), job.current)
        } else {
            job.current.clone()
        };
        let actions = row![
            speed_graph(&job.rate.history),
            text(detail).font(theme::MONO).size(10.5).style(theme::ink3).width(Fill).wrapping(Wrapping::None),
            (!job.queued).then(|| {
                button(text(if job.paused { "Continuar" } else { "Pausar" }).size(11))
                    .padding([3, 9])
                    .on_press(Message::PauseJob(job.id))
                    .style(theme::ghost)
            }),
            button(text(if job.queued { "Tirar da fila" } else { "Cancelar" }).size(11))
                .padding([3, 9])
                .on_press(Message::CancelJob(job.id))
                .style(theme::ghost),
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        col = col.push(
            column![
                text(job.label.as_str()).font(theme::UI_BOLD).size(12).wrapping(Wrapping::None),
                progress_bar(0.0..=1.0, pct).length(Fill).girth(5).style(theme::progress),
                actions
            ]
            .spacing(5),
        );
    }
    if let Some(u) = update_banner(app) {
        col = col.push(u);
    }
    if let Some(t) = &app.toast {
        let err = t.error;
        col = col.push(text(t.text.as_str()).size(12).style(move |th: &Theme| if err { theme::danger(th) } else { theme::ink2(th) }));
    }
    if let Some(fps) = &app.fps {
        let (frames, worst) = fps.stats();
        let slow = worst > 16.7;
        let label = format!("{frames} fps · pior {worst:.1} ms");
        col = col.push(container(text(label).font(theme::MONO).size(10.5)).padding([3, 8]).style(move |t: &Theme| theme::latency(t, slow)));
    }
    Some(opaque(container(col).padding([10, 12]).width(380).style(|t: &Theme| {
        let mut s = theme::glass(t, false);
        s.background = Some(theme::tokens(t).surface.into());
        s.border = iced::Border { color: theme::tokens(t).stroke_hi, width: 1.0, radius: 12.0.into() };
        // cartão flutuante: sombra curta e difusa para destacar do conteúdo por trás
        s.shadow = iced::Shadow { color: Color::from_rgba(0.0, 0.0, 0.0, 0.35), offset: iced::Vector::new(0.0, 10.0), blur_radius: 30.0 };
        s
    })))
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
    // entrada: o véu escurece e o cartão sobe alguns pixels até o lugar
    let t = app.motion.overlay(Instant::now());
    let lift = 14.0 * (1.0 - t);
    let card: Element<_> = match &app.overlay {
        Overlay::None => return None,
        Overlay::Palette { query, selected } => {
            let card = palette(app, query, *selected);
            return Some(stack![scrim(t), container(opaque(card)).center_x(Fill).padding(Padding::new(0.0).top(90.0 + lift))].into());
        }
        Overlay::Frecent { query, selected } => {
            let card = jump(app, query, *selected);
            return Some(stack![scrim(t), container(opaque(card)).center_x(Fill).padding(Padding::new(0.0).top(90.0 + lift))].into());
        }
        Overlay::Properties { path, props, preview } => props_view::sheet(path, props.as_deref(), preview.as_ref()),
        Overlay::Prompt { kind, value } => {
            let input = text_input("", value)
                .id(PROMPT_INPUT)
                .on_input(Message::PromptChanged)
                .on_submit(Message::PromptSubmit)
                .padding(10)
                .size(14)
                .style(theme::input);
            let suggestions = suggestions(app, *kind);
            let hint: Option<Element<_>> = (*kind == PromptKind::BatchRename).then(|| batch_hint(app, value));
            sheet(
                column![
                    text(kind.title()).font(theme::DISPLAY).size(16),
                    input,
                    suggestions,
                    hint,
                    row![
                        kbd("Enter".into()),
                        text("confirma").size(11.5).style(theme::ink3),
                        kbd("Tab".into()),
                        text("completa").size(11.5).style(theme::ink3),
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
        Overlay::Conflict { clashes, .. } => conflict_sheet(clashes),
        Overlay::Menu(m) => return Some(commands_view::menu(app, m)),
        Overlay::Jump(j) => return Some(commands_view::jump(app, j)),
        Overlay::Confirm { text: question, action } => sheet(
            column![
                text("Confirmar").font(theme::DISPLAY).size(16),
                text(question.as_str()).size(13.5),
                row![
                    space().width(Fill),
                    button(text("Cancelar").size(13)).padding([7, 14]).on_press(Message::CloseOverlay).style(theme::ghost),
                    button(text(action.verb()).size(13)).padding([7, 14]).on_press(Message::Confirmed).style(theme::primary),
                ]
                .spacing(8)
            ]
            .spacing(14),
            420.0,
        ),
    };
    Some(stack![scrim(t), center(opaque(card)).padding(Padding::new(40.0).top(40.0 + 2.0 * lift))].into())
}

/// Já existe item com o mesmo nome no destino: compara os dois lados e o usuário escolhe uma regra para todos.
fn conflict_sheet(clashes: &[crate::clash::Clash]) -> Element<'_, Message> {
    let n = clashes.len();
    let title = match clashes {
        [one] => format!("\"{}\" já existe no destino", shorten(&one.name, 40)),
        _ => format!("{n} itens já existem no destino"),
    };
    let side = |label: &'static str, info: String| {
        row![text(label).size(11.5).style(theme::ink3).width(58), text(info).size(11.5).font(theme::MONO).style(theme::ink2)].spacing(6)
    };
    let shown: Vec<Element<_>> = clashes
        .iter()
        .take(4)
        .map(|c| {
            let newer = c.source_newer();
            let older = !newer && !c.same() && c.source.modified.is_some() && c.dest.modified.is_some();
            let same = c.same().then(|| text("mesmo tamanho e data").size(11).style(theme::ink3));
            column![
                row![text(shorten(&c.name, 52)).size(12.5), same].spacing(8).align_y(Alignment::Center),
                side("Origem", crate::clash::describe(&c.source, newer)),
                side("Destino", crate::clash::describe(&c.dest, older)),
            ]
            .spacing(1)
            .into()
        })
        .collect();
    let more = (n > 4).then(|| text(format!("… e mais {}", n - 4)).size(11.5).style(theme::ink3));
    let choice = |label: &'static str, key: &'static str, msg: Message| {
        button(row![kbd(key.to_string()), text(label).size(12.5)].spacing(8).align_y(Alignment::Center))
            .padding([6, 12])
            .on_press(msg)
            .style(theme::ghost)
    };
    use arbor_core::ops::Conflict;
    sheet(
        column![
            text(title).font(theme::DISPLAY).size(16),
            column(shown).spacing(8),
            more,
            text("Substituir manda o que já existe para a lixeira. Manter os dois cria \"nome (2)\". Só os mais novos substitui onde a origem é mais recente e pula o resto.")
                .size(11.5)
                .style(theme::ink3),
            row![
                choice("Substituir", "S", Message::ResolveConflict(Some(Conflict::Replace))),
                choice("Pular", "P", Message::ResolveConflict(Some(Conflict::Skip))),
                choice("Manter os dois", "M", Message::ResolveConflict(Some(Conflict::KeepBoth))),
                choice("Só os mais novos", "N", Message::KeepNewer),
                space().width(Fill),
                choice("Cancelar", "Esc", Message::ResolveConflict(None)),
            ]
            .spacing(4)
            .align_y(Alignment::Center)
        ]
        .spacing(12),
        660.0,
    )
}

/// Pastas sugeridas abaixo da caixa "Ir para"; clique ou Tab completa.
fn suggestions(app: &App, kind: PromptKind) -> Element<'_, Message> {
    if kind != PromptKind::GoTo || app.suggest.items.is_empty() {
        return space().height(0).into();
    }
    let mut col = Column::new().spacing(1);
    for (i, p) in app.suggest.items.iter().enumerate() {
        let hot = app.suggest.selected == Some(i);
        let label = row![icon(Icon::Folder, 14.0, theme::ICE), text(p.to_string_lossy().into_owned()).size(12.5).wrapping(Wrapping::None)]
            .spacing(8)
            .align_y(Alignment::Center);
        col = col.push(
            button(label)
                .width(Fill)
                .padding([5, 8])
                .on_press(Message::SuggestPick(i))
                .style(move |t: &Theme, s| theme::place(t, if hot { button::Status::Hovered } else { s }, hot)),
        );
    }
    col.into()
}

/// Etiqueta que segue o mouse no arraste: quantos itens e o que vai acontecer ao soltar.
fn drag_ghost(app: &App) -> Option<Element<'_, Message>> {
    let (paths, at, over) = app.dragging()?;
    let what = if paths.len() == 1 { shorten(&paths[0].file_name()?.to_string_lossy(), 28) } else { format!("{} itens", paths.len()) };
    let action = match over {
        Some(dest) if crate::pins::is_drop(dest) => "Fixar na barra lateral".to_string(),
        Some(dest) if crate::app::drag_valid(paths, dest) => {
            let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| dest.display().to_string());
            let verb = if app.drag_copies(paths, dest) { "Copiar para" } else { "Mover para" };
            format!("{verb} {name}")
        }
        _ => "Solte numa pasta".to_string(),
    };
    let tag = container(column![text(what).size(12.5).font(theme::UI_BOLD), text(action).size(11.5).style(theme::ink2)].spacing(2))
        .padding([6, 10])
        .style(theme::tooltip);
    Some(container(tag).padding(Padding::new(0.0).left(at.x + 14.0).top(at.y + 14.0)).into())
}

/// Ajuda e prévia ao vivo da renomeação em lote.
fn batch_hint<'a>(app: &'a App, value: &str) -> Element<'a, Message> {
    let help = text("{name} nome · {n} ou {n:3} contador · {ext} extensão · {date} data · {upper} {lower} · antigo => novo")
        .size(11.5)
        .style(theme::ink3);
    let preview: Element<_> = match app.batch_preview(value) {
        Ok((n, pairs)) => {
            let rows = pairs.into_iter().map(|(from, to)| {
                row![
                    text(shorten(&from, 28)).size(12).font(theme::MONO).style(theme::ink3).width(Fill),
                    text("→").size(12).style(theme::ink3),
                    text(shorten(&to, 28)).size(12).font(theme::MONO).width(Fill)
                ]
                .spacing(8)
                .into()
            });
            let more = (n > 6).then(|| text(format!("… e mais {}", n - 6)).size(11.5).style(theme::ink3));
            column![text(format!("{n} item(ns) mudam de nome")).size(12), column(rows).spacing(3), more].spacing(6).into()
        }
        Err(e) => text(e).size(12).style(theme::danger).into(),
    };
    column![help, preview].spacing(8).into()
}

fn scrim<'a>(t: f32) -> Element<'a, Message> {
    mouse_area(container(space()).width(Fill).height(Fill).style(move |th: &Theme| theme::scrim(th, t)))
        .on_press(Message::CloseOverlay)
        .into()
}

/// Filete entre os arquivos e o painel de detalhes; puxar redimensiona o painel.
fn details_split<'a>() -> Element<'a, Message> {
    let line = container(space().width(1).height(Fill))
        .style(|t: &Theme| iced::widget::container::Style { background: Some(theme::tokens(t).stroke.into()), ..Default::default() });
    mouse_area(container(line).center_x(SPLIT_W).height(Fill))
        .on_press(Message::DetailsResizeStart)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .into()
}

/// Ctrl+K: pastas frequentes, filtradas pelo que se digita.
fn jump<'a>(app: &'a App, query: &'a str, selected: usize) -> Element<'a, Message> {
    let input = text_input("Parte do nome de uma pasta que você já visitou…", query)
        .id(JUMP_INPUT)
        .on_input(Message::JumpQuery)
        .on_submit(Message::JumpRun(usize::MAX))
        .padding(Padding::from([12, 14]).left(42.0))
        .size(16)
        .style(|t, s| {
            let mut st = theme::input(t, s);
            st.border.width = 0.0;
            st
        });
    let input = stack![input, container(icon(Icon::Folder, 18.0, theme::ICE)).padding(Padding::new(0.0).left(14.0)).center_y(Fill)];
    let items = app.jump_view();
    let mut list = Column::new().spacing(2);
    if items.is_empty() {
        list = list.push(container(text("Nenhuma pasta visitada combina ainda").size(12.5).style(theme::ink3)).padding([8, 12]));
    }
    for (i, dir) in items.into_iter().enumerate() {
        let active = i == selected;
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| dir.to_string_lossy().into_owned());
        let parent = dir.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let line = row![
            text(name).size(13.5),
            text(shorten(&parent, 60)).size(11).font(theme::MONO).style(theme::ink3).width(Fill).wrapping(Wrapping::None)
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        list = list.push(
            button(line).width(Fill).padding([8, 12]).on_press(Message::JumpRun(i)).style(move |t: &Theme, s| theme::place(t, s, active)),
        );
    }
    let foot = row![
        kbd("↑".into()),
        kbd("↓".into()),
        text("navegar").size(11).style(theme::ink3),
        kbd("Enter".into()),
        text("abrir").size(11).style(theme::ink3),
        kbd("Esc".into()),
        text("fechar").size(11).style(theme::ink3)
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    sheet(column![container(input).height(48), separator(), list, separator(), foot].spacing(10), 640.0)
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
        // Um destaque só: passar o mouse move a seleção (como as setas), em vez de pintar uma segunda linha.
        let b = button(line).width(Fill).padding([8, 12]).on_press(Message::PaletteRun(i)).style(move |t: &Theme, s| {
            let s = if s == button::Status::Hovered { button::Status::Active } else { s };
            theme::place(t, s, active)
        });
        list = list.push(mouse_area(b).on_enter(Message::PaletteHover(i)));
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
    let b = button(icon(ic, 15.0, theme::now().ink2)).padding([5, 7]).on_press(msg).style(theme::ghost);
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
    drive.map_or_else(|| kind(e), |d| Kind::Drive(d.media))
}

/// Ícone e cor de cada tipo de unidade, como o Explorer distingue sistema, USB, DVD e rede.
pub(crate) fn drive_look(m: Media) -> (Icon, Color) {
    match m {
        Media::System => (Icon::DriveSystem, theme::ICE),
        Media::Fixed => (Icon::Drive, theme::STEEL),
        Media::Usb => (Icon::Usb, theme::AMBER),
        Media::Optical => (Icon::Disc, theme::ROSE),
        Media::Network => (Icon::Network, theme::IRIS),
    }
}

pub fn kind_label(e: &Entry) -> String {
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
    Drive(Media),
    Image,
    Code,
    Archive,
    Doc,
    Audio,
    Video,
    Other,
}

fn kind(e: &Entry) -> Kind {
    kind_for(e.is_dir, e.ext.as_str(), &e.path)
}

fn kind_for(is_dir: bool, ext: &str, path: &Path) -> Kind {
    if is_dir {
        return Kind::Folder;
    }
    match ext {
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "c" | "h" | "cpp" | "hpp" | "cs" | "java" | "kt" | "swift" | "rb" | "php"
        | "sh" | "ps1" | "toml" | "json" | "yaml" | "yml" | "xml" | "html" | "css" | "lua" | "sql" | "lock" => Kind::Code,
        "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" | "zst" | "iso" | "dmg" => Kind::Archive,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "txt" | "rtf" | "epub" | "md" => Kind::Doc,
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "opus" | "aac" => Kind::Audio,
        "mp4" | "mkv" | "mov" | "avi" | "webm" => Kind::Video,
        _ if crate::media::is_image(path) => Kind::Image,
        _ => Kind::Other,
    }
}

/// Uma cor por tipo, a mesma do protótipo.
fn kind_color(k: Kind) -> Color {
    match k {
        Kind::Folder => theme::ICE,
        Kind::Drive(m) => drive_look(m).1,
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
        Kind::Drive(m) => drive_look(m).0,
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
    entry_chip(k, size, false, false)
}

/// Ficha de um item: oculto sai translúcido; pasta que é repositório git troca para a pasta com o
/// losango do git, em coral (legível até no tamanho da lista, onde um selo sumiria).
fn entry_chip<'a>(k: Kind, size: f32, hidden: bool, repo: bool) -> Element<'a, Message> {
    let strength = if hidden { theme::HIDDEN_ALPHA } else { 1.0 };
    let (ic, color) = if repo { (Icon::FolderGit, theme::CORAL) } else { (kind_icon(k), kind_color(k)) };
    container(icon(ic, size * 0.58, theme::alpha(color, strength)))
        .center_x(size)
        .center_y(size)
        .style(theme::chip_soft(color, strength))
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

#[cfg(test)]
mod tests {
    use super::stem;

    #[test]
    fn hiding_extensions_keeps_dotfiles_and_trailing_dots_whole() {
        assert_eq!(stem("foto.tar.gz"), "foto.tar");
        assert_eq!(stem("foto.jpg"), "foto");
        assert_eq!(stem(".bashrc"), ".bashrc");
        assert_eq!(stem("arquivo."), "arquivo.");
        assert_eq!(stem("LEIAME"), "LEIAME");
    }
}
