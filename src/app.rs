//! Estado e lógica do app (o `update` do iced). A interface fica em `view.rs`.

use crate::commands::{self, Cmd};
mod menu;

pub use menu::{Menu, MenuKind};

use crate::computer::{self, Drive};
use crate::media::{self, Preview};
use crate::{launch, listing, pins, pointer, update, watch};
use arbor_core::ops::{self, JobEvent, Op, Undo};
use arbor_core::{fuzzy, sort_entries, Entry, Snapshot, Sort, SortKey};
use iced::advanced::widget::operation::focusable;
use iced::keyboard::{key::Named, Key, Modifiers};
use iced::widget::image::Handle;
use iced::widget::operation::{self as op, AbsoluteOffset};
use iced::widget::scrollable::Viewport;
use iced::{event, keyboard, window, Event, Size, Subscription, Task};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const ROW_H: f32 = 34.0;
pub const CELL_W: f32 = 142.0;
pub const CELL_H: f32 = 168.0;
pub const SIDEBAR_W: f32 = 216.0;

pub struct App {
    pub panes: [Pane; 2],
    pub active: usize,
    pub dual: bool,
    pub overlay: Overlay,
    pub jobs: Vec<Job>,
    next_job: u64,
    undo: Vec<Undo>,
    clip: Option<(bool, Vec<PathBuf>)>,
    pub toast: Option<Toast>,
    next_toast: u64,
    pub dark: bool,
    pub show_hidden: bool,
    pub thumbs: HashMap<PathBuf, Option<Handle>>,
    modifiers: Modifiers,
    pub window: Size,
    pub places: Vec<(String, PathBuf)>,
    pub drives: Vec<Drive>,
    pub pins: Vec<PathBuf>,
    /// Alvo do menu de onde o comando atual veio (fixar ou abrir uma pasta da barra lateral).
    menu_target: Option<PathBuf>,
    next_tab: u64,
    pub update: UpdateState,
    pub fps: Option<Fps>,
}

/// Estado da atualização automática mostrado no rodapé.
#[derive(Debug, Clone)]
pub enum UpdateState {
    Idle,
    Available(update::Update),
    Working(update::Progress),
}

/// Medidor de quadros: instantes dos quadros do último segundo.
#[derive(Debug, Default)]
pub struct Fps {
    frames: std::collections::VecDeque<Instant>,
}

impl Fps {
    fn push(&mut self, at: Instant) {
        self.frames.push_back(at);
        while self.frames.front().is_some_and(|f| at.duration_since(*f) > Duration::from_secs(1)) {
            self.frames.pop_front();
        }
    }

    /// Quadros no último segundo e o pior intervalo entre dois quadros, em ms.
    pub fn stats(&self) -> (usize, f32) {
        let worst = self
            .frames
            .iter()
            .zip(self.frames.iter().skip(1))
            .map(|(a, b)| b.duration_since(*a).as_secs_f32() * 1000.0)
            .fold(0.0, f32::max);
        (self.frames.len(), worst)
    }
}

pub struct Pane {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl Pane {
    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }
    fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }
}

pub struct Tab {
    pub id: u64,
    pub path: PathBuf,
    /// Snapshot imutável vindo da listagem; só é copiado se a ordenação mudar enquanto ainda é compartilhado.
    pub entries: Arc<Vec<Entry>>,
    /// Itens e tempo da última listagem completa: o "N itens em X ms" do cabeçalho.
    pub listed: Option<(usize, Duration)>,
    /// Filtro que gerou `view`; se o novo filtro só acrescenta letras, filtra só o resultado anterior.
    view_filter: String,
    /// Índices de `entries` visíveis (após ocultos e filtro), na ordem de exibição.
    pub view: Vec<usize>,
    pub cursor: usize,
    pub selected: HashSet<PathBuf>,
    anchor: usize,
    pub filter: String,
    pub sort: Sort,
    pub loading: bool,
    refreshing: bool,
    generation: u64,
    pub error: Option<String>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    pub scroll_y: f32,
    pub viewport_h: f32,
    pub grid: bool,
    /// Selecionar este item quando ele aparecer na listagem (ao subir de pasta, depois de copiar...).
    focus_on_load: Vec<PathBuf>,
}

impl Tab {
    fn new(id: u64, path: PathBuf) -> Tab {
        Tab {
            id,
            path,
            entries: Arc::default(),
            listed: None,
            view_filter: String::new(),
            view: vec![],
            cursor: 0,
            selected: HashSet::new(),
            anchor: 0,
            filter: String::new(),
            sort: Sort::default(),
            loading: false,
            refreshing: false,
            generation: 0,
            error: None,
            back: vec![],
            forward: vec![],
            scroll_y: 0.0,
            viewport_h: 600.0,
            grid: false,
            focus_on_load: vec![],
        }
    }

    pub fn title(&self) -> String {
        if computer::is(&self.path) {
            return computer::TITLE.into();
        }
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

    pub fn current(&self) -> Option<&Entry> {
        self.view.get(self.cursor).map(|&i| &self.entries[i])
    }

    pub fn entry_at(&self, view_index: usize) -> Option<&Entry> {
        self.view.get(view_index).map(|&i| &self.entries[i])
    }

    /// Itens alvo de uma ação: a seleção, ou o item sob o cursor.
    pub fn targets(&self) -> Vec<PathBuf> {
        if self.selected.is_empty() {
            self.current().map(|e| vec![e.path.clone()]).unwrap_or_default()
        } else {
            self.view.iter().map(|&i| &self.entries[i].path).filter(|p| self.selected.contains(*p)).cloned().collect()
        }
    }

    pub fn scroll_id(&self) -> String {
        format!("list-{}", self.id)
    }

    pub fn filter_id(&self) -> String {
        format!("filter-{}", self.id)
    }

    /// Troca ou reordena `entries` mantendo o cursor no mesmo arquivo.
    fn edit_entries(&mut self, show_hidden: bool, f: impl FnOnce(&mut Arc<Vec<Entry>>)) {
        let keep = self.current().map(|e| e.path.clone());
        f(&mut self.entries);
        self.view.clear();
        self.view_filter.clear();
        self.rebuild(show_hidden);
        if let Some(i) = keep.and_then(|p| self.position(&p)) {
            self.cursor = i;
        }
    }

    fn rebuild(&mut self, show_hidden: bool) {
        let keep = self.current().map(|e| e.path.clone());
        // Digitar mais uma letra só pode reduzir o resultado: filtra o que já passou em vez da pasta toda.
        let narrowing = !self.view_filter.is_empty() && self.filter.starts_with(&self.view_filter) && !self.view.is_empty();
        let candidates: Vec<usize> = if narrowing {
            std::mem::take(&mut self.view)
        } else {
            self.entries.iter().enumerate().filter(|(_, e)| show_hidden || !e.hidden).map(|(i, _)| i).collect()
        };
        self.view = if self.filter.is_empty() {
            candidates
        } else {
            let entries = &self.entries;
            fuzzy::rank_slice(&self.filter, &candidates, |&i| entries[i].name.as_str()).into_iter().map(|k| candidates[k]).collect()
        };
        self.view_filter.clone_from(&self.filter);
        if !self.filter.is_empty() {
            self.cursor = 0;
        } else if let Some(p) = keep {
            self.cursor = self.position(&p).unwrap_or(self.cursor);
        }
        self.cursor = self.cursor.min(self.view.len().saturating_sub(1));
    }

    fn position(&self, path: &Path) -> Option<usize> {
        self.view.iter().position(|&i| self.entries[i].path == path)
    }
}

pub struct Job {
    pub id: u64,
    pub label: String,
    pub done: u64,
    pub total: u64,
    pub current: String,
    cancel: Arc<AtomicBool>,
}

pub struct Toast {
    pub id: u64,
    pub text: String,
    pub error: bool,
}

pub enum Overlay {
    None,
    Palette { query: String, selected: usize },
    Prompt { kind: PromptKind, value: String },
    Preview { path: PathBuf, content: Option<Preview> },
    Menu(Menu),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Rename,
    NewFolder,
    GoTo,
}

impl PromptKind {
    pub fn title(self) -> &'static str {
        match self {
            PromptKind::Rename => "Renomear",
            PromptKind::NewFolder => "Nova pasta",
            PromptKind::GoTo => "Ir para",
        }
    }
}

pub const PALETTE_INPUT: &str = "palette-input";
pub const PROMPT_INPUT: &str = "prompt-input";

#[derive(Debug, Clone)]
pub enum Message {
    Listed { tab: u64, generation: u64, snap: Snapshot },
    Key { key: Key, modifiers: Modifiers, text: Option<String>, captured: bool },
    Modifiers(Modifiers),
    Resized(Size),
    Dropped(PathBuf),
    Run(Cmd),
    ContextMenu { tab: u64, index: Option<usize> },
    SidebarMenu(PathBuf),
    OpenMenu(MenuKind),
    MenuRun(Cmd),
    MenuHover(usize),
    Click { tab: u64, index: usize },
    Activate { tab: u64, index: usize },
    Scrolled { tab: u64, viewport: Viewport },
    Navigate(PathBuf),
    NavigateIn { tab: u64, path: PathBuf },
    SelectTab { pane: usize, index: usize },
    CloseTabAt { pane: usize, index: usize },
    NewTabIn(usize),
    FocusPane(usize),
    SortBy { tab: u64, key: SortKey },
    FilterChanged { tab: u64, value: String },
    FilterSubmit,
    PaletteQuery(String),
    PaletteRun(usize),
    PromptChanged(String),
    PromptSubmit,
    CloseOverlay,
    PreviewLoaded { path: PathBuf, content: Preview },
    Thumb { path: PathBuf, handle: Option<Handle> },
    Job { id: u64, event: JobEvent },
    CancelJob(u64),
    ToastExpired(u64),
    DirChanged(PathBuf),
    Places(Vec<Drive>),
    Frame(Instant),
    UpdateCheck { manual: bool },
    UpdateChecked { manual: bool, result: Result<Option<update::Update>, String> },
    UpdateInstall,
    UpdateProgress(update::Progress),
    UpdateRestart,
    UpdateDismiss,
}

impl App {
    pub fn boot() -> (App, Task<Message>) {
        let start = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .and_then(|p| std::fs::canonicalize(p).ok())
            .and_then(start_dir)
            .unwrap_or_else(computer::path);
        let second = dirs::download_dir().filter(|d| d.is_dir()).unwrap_or_else(|| start.clone());
        let mut app = App {
            panes: [
                Pane { tabs: vec![Tab::new(0, start.clone())], active: 0 },
                Pane { tabs: vec![Tab::new(1, second.clone())], active: 0 },
            ],
            active: 0,
            dual: false,
            overlay: Overlay::None,
            jobs: vec![],
            next_job: 0,
            undo: vec![],
            clip: None,
            toast: None,
            next_toast: 0,
            dark: true,
            show_hidden: false,
            thumbs: HashMap::new(),
            modifiers: Modifiers::default(),
            window: Size::new(1280.0, 800.0),
            places: computer::places(),
            drives: vec![],
            pins: pins::load(pins::file().as_deref()),
            menu_target: None,
            next_tab: 2,
            update: UpdateState::Idle,
            fps: None,
        };
        let tasks = Task::batch([
            app.load(0, start, false),
            app.load(1, second, false),
            scan_drives(),
            if update::auto_enabled() {
                Task::perform(tokio::time::sleep(update::FIRST_CHECK), |_| Message::UpdateCheck { manual: false })
            } else {
                Task::none()
            },
        ]);
        (app, tasks)
    }

    pub fn title(&self) -> String {
        format!("{} — arbor", if computer::is(&self.tab().path) { computer::TITLE.into() } else { self.tab().path.display().to_string() })
    }

    pub fn tab(&self) -> &Tab {
        self.panes[self.active].tab()
    }

    fn tab_mut(&mut self) -> &mut Tab {
        self.panes[self.active].tab_mut()
    }

    fn other_pane(&self) -> usize {
        1 - self.active
    }

    fn find_tab(&mut self, id: u64) -> Option<&mut Tab> {
        self.panes.iter_mut().flat_map(|p| p.tabs.iter_mut()).find(|t| t.id == id)
    }

    fn pane_of(&self, id: u64) -> Option<usize> {
        self.panes.iter().position(|p| p.tabs.iter().any(|t| t.id == id))
    }

    pub fn grid_cols(&self) -> usize {
        let panes = if self.dual { 2.0 } else { 1.0 };
        // margens da janela, espaço entre os cartões e o recuo da grade
        let width = (self.window.width - SIDEBAR_W - 30.0 - 10.0 * (panes - 1.0)) / panes - 20.0;
        ((width / CELL_W).floor() as usize).max(1)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keys = event::listen_with(|event, status, _| match event {
            Event::Mouse(_) => {
                pointer::track(&event);
                None
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, text, .. }) => {
                Some(Message::Key { key, modifiers, text: text.map(|t| t.to_string()), captured: status == event::Status::Captured })
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => Some(Message::Modifiers(m)),
            Event::Window(window::Event::Resized(size)) => Some(Message::Resized(size)),
            Event::Window(window::Event::FileDropped(path)) => Some(Message::Dropped(path)),
            _ => None,
        });
        let visible = if self.dual { &self.panes[..] } else { &self.panes[self.active..=self.active] };
        let watchers = visible
            .iter()
            .filter(|p| !computer::is(&p.tab().path))
            .map(|p| Subscription::run_with(p.tab().path.clone(), watch::watch).map(Message::DirChanged));
        let fps = if self.fps.is_some() { window::frames().map(Message::Frame) } else { Subscription::none() };
        let updates = if update::auto_enabled() {
            iced::time::every(update::EVERY).map(|_| Message::UpdateCheck { manual: false })
        } else {
            Subscription::none()
        };
        Subscription::batch(std::iter::once(keys).chain(watchers).chain([fps, updates]))
    }

    // ---------- navegação e listagem ----------

    /// Lista `path` na aba ativa do painel `pane`. `refresh` mantém os itens atuais até a nova lista chegar.
    fn load(&mut self, pane: usize, path: PathBuf, refresh: bool) -> Task<Message> {
        let tab = self.panes[pane].tab_mut();
        Self::load_tab(tab, path, refresh)
    }

    fn load_tab(tab: &mut Tab, path: PathBuf, refresh: bool) -> Task<Message> {
        tab.generation += 1;
        tab.loading = true;
        tab.refreshing = refresh;
        if !refresh {
            tab.focus_on_load.retain(|p| p.parent() == Some(path.as_path()));
            tab.path = path.clone();
            tab.entries = Arc::default();
            tab.view.clear();
            tab.view_filter.clear();
            tab.listed = None;
            tab.selected.clear();
            tab.cursor = 0;
            tab.scroll_y = 0.0;
            tab.filter.clear();
            tab.error = None;
        }
        let (id, generation, sort) = (tab.id, tab.generation, tab.sort);
        let scroll = if refresh { Task::none() } else { op::scroll_to(tab.scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }) };
        if computer::is(&path) {
            return Task::batch([scan_drives(), scroll]);
        }
        Task::batch([Task::run(listing::list(path, sort), move |snap| Message::Listed { tab: id, generation, snap }), scroll])
    }

    fn navigate(&mut self, pane: usize, path: PathBuf) -> Task<Message> {
        let tab = self.panes[pane].tab_mut();
        if tab.path == path {
            return Task::none();
        }
        let old = std::mem::replace(&mut tab.path, path.clone());
        tab.back.push(old);
        tab.forward.clear();
        self.load(pane, path, false)
    }

    fn go_up(&mut self) -> Task<Message> {
        let tab = self.tab();
        // Acima da raiz de um disco fica "Este Computador", como no Explorer.
        if computer::is(&tab.path) {
            return Task::none();
        }
        let parent = tab.path.parent().map(Path::to_path_buf).unwrap_or_else(computer::path);
        let child = tab.path.clone();
        let pane = self.active;
        let task = self.navigate(pane, parent);
        self.tab_mut().focus_on_load = vec![child];
        task
    }

    fn history(&mut self, back: bool) -> Task<Message> {
        let tab = self.tab_mut();
        let target = if back { tab.back.pop() } else { tab.forward.pop() };
        let Some(target) = target else { return Task::none() };
        let current = tab.path.clone();
        if back {
            tab.forward.push(current.clone())
        } else {
            tab.back.push(current.clone())
        }
        tab.focus_on_load = vec![current];
        let pane = self.active;
        self.load(pane, target, false)
    }

    fn open_entry(&mut self, pane: usize, entry: Entry) -> Task<Message> {
        if entry.is_dir {
            self.navigate(pane, entry.path)
        } else {
            match launch::open(&entry.path) {
                Ok(()) => Task::none(),
                Err(e) => self.notify(format!("Não abriu {}: {e}", entry.name), true),
            }
        }
    }

    fn on_listed(&mut self, id: u64, generation: u64, snap: Snapshot) -> Task<Message> {
        let show_hidden = self.show_hidden;
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        if tab.generation != generation {
            return Task::none();
        }
        // Ao atualizar uma pasta já aberta, os itens antigos ficam na tela até a lista nova estar completa.
        if tab.refreshing && !snap.complete {
            return Task::none();
        }
        // A ordenação pode ter mudado enquanto a lista era lida; o snapshot veio na ordem antiga.
        // "Este Computador" chega já agrupado (pastas, depois discos); só reordena se pedirem.
        let sort = tab.sort;
        let grouped = computer::is(&tab.path);
        tab.edit_entries(show_hidden, |entries| {
            *entries = snap.entries;
            if !grouped && !entries.is_sorted_by(|a, b| arbor_core::compare(a, b, sort).is_le()) {
                sort_entries(Arc::<Vec<Entry>>::make_mut(entries), sort);
            }
        });
        if snap.complete {
            tab.loading = false;
            tab.listed = Some((tab.entries.len(), snap.elapsed));
            if tab.refreshing {
                let alive: HashSet<&Path> = tab.entries.iter().map(|e| e.path.as_path()).collect();
                tab.selected.retain(|p| alive.contains(p.as_path()));
                tab.refreshing = false;
            }
            tab.error = snap.error;
        }
        let tab = self.find_tab(id).expect("aba existe");
        let mut task = Task::none();
        if !tab.focus_on_load.is_empty() {
            let wanted = tab.focus_on_load.clone();
            if let Some(first) = wanted.iter().find_map(|p| tab.position(p)) {
                tab.cursor = first;
                tab.anchor = first;
                tab.selected = wanted.iter().filter(|p| tab.position(p).is_some()).cloned().collect();
                if !tab.loading || tab.selected.len() == wanted.len() {
                    tab.focus_on_load.clear();
                }
                task = self.reveal_cursor(id);
            }
        }
        Task::batch([task, self.request_thumbs(id)])
    }

    // ---------- seleção e rolagem ----------

    fn move_cursor(&mut self, delta: isize, extend: bool) -> Task<Message> {
        let cols = if self.tab().grid { self.grid_cols() as isize } else { 1 };
        let page = ((self.tab().viewport_h / if self.tab().grid { CELL_H } else { ROW_H }) as isize).max(1) * cols;
        let delta = match delta {
            isize::MIN => isize::MIN / 2,
            isize::MAX => isize::MAX / 2,
            d if d.abs() == 1000 => d.signum() * page,
            d if d.abs() == 100 => d.signum() * cols,
            d => d,
        };
        let tab = self.tab_mut();
        if tab.view.is_empty() {
            return Task::none();
        }
        let last = tab.view.len() as isize - 1;
        tab.cursor = (tab.cursor as isize).saturating_add(delta).clamp(0, last) as usize;
        if extend {
            let (a, b) = (tab.anchor.min(tab.cursor), tab.anchor.max(tab.cursor));
            tab.selected = (a..=b).filter_map(|i| tab.entry_at(i).map(|e| e.path.clone())).collect();
        } else {
            tab.anchor = tab.cursor;
            tab.selected = tab.current().map(|e| e.path.clone()).into_iter().collect();
        }
        let id = tab.id;
        let reveal = self.reveal_cursor(id);
        let preview = self.refresh_preview();
        Task::batch([reveal, preview])
    }

    /// Rola o mínimo necessário para o cursor ficar visível.
    fn reveal_cursor(&mut self, id: u64) -> Task<Message> {
        let cols = self.grid_cols();
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        let (row, h) = if tab.grid { (tab.cursor / cols, CELL_H) } else { (tab.cursor, ROW_H) };
        let top = row as f32 * h;
        let target = if top < tab.scroll_y {
            top
        } else if top + h > tab.scroll_y + tab.viewport_h {
            top + h - tab.viewport_h
        } else {
            return Task::none();
        };
        tab.scroll_y = target.max(0.0);
        let task = op::scroll_to(tab.scroll_id(), AbsoluteOffset { x: 0.0, y: tab.scroll_y });
        Task::batch([task, self.request_thumbs(id)])
    }

    fn click(&mut self, id: u64, index: usize) -> Task<Message> {
        if let Some(pane) = self.pane_of(id) {
            self.active = pane;
        }
        let mods = self.modifiers;
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        let Some(path) = tab.entry_at(index).map(|e| e.path.clone()) else { return Task::none() };
        if mods.command() {
            if !tab.selected.remove(&path) {
                tab.selected.insert(path);
            }
            tab.anchor = index;
        } else if mods.shift() {
            let (a, b) = (tab.anchor.min(index), tab.anchor.max(index));
            tab.selected = (a..=b).filter_map(|i| tab.entry_at(i).map(|e| e.path.clone())).collect();
        } else {
            tab.selected = HashSet::from([path]);
            tab.anchor = index;
        }
        tab.cursor = index;
        self.refresh_preview()
    }

    // ---------- miniaturas e preview ----------

    fn request_thumbs(&mut self, id: u64) -> Task<Message> {
        let cols = self.grid_cols();
        let Some(tab) = self.panes.iter().flat_map(|p| p.tabs.iter()).find(|t| t.id == id) else { return Task::none() };
        if !tab.grid {
            return Task::none();
        }
        let first_row = (tab.scroll_y / CELL_H).floor() as usize;
        let rows = (tab.viewport_h / CELL_H).ceil() as usize + 2;
        let start = first_row.saturating_sub(1) * cols;
        let end = ((first_row + rows) * cols).min(tab.view.len());
        let wanted: Vec<PathBuf> = (start..end)
            .filter_map(|i| tab.entry_at(i))
            .filter(|e| !e.is_dir && media::is_image(&e.path) && !self.thumbs.contains_key(&e.path))
            .map(|e| e.path.clone())
            .collect();
        if self.thumbs.len() + wanted.len() > 5000 {
            self.thumbs.clear(); // voltam do cache em disco, que é rápido
        }
        Task::batch(wanted.into_iter().map(|path| {
            self.thumbs.insert(path.clone(), None);
            Task::perform(media::thumbnail(path.clone()), move |handle| Message::Thumb { path: path.clone(), handle })
        }))
    }

    fn open_preview(&mut self) -> Task<Message> {
        let Some(entry) = self.tab().current() else { return Task::none() };
        let path = entry.path.clone();
        self.overlay = Overlay::Preview { path: path.clone(), content: None };
        Task::perform(media::preview(path.clone()), move |content| Message::PreviewLoaded { path: path.clone(), content })
    }

    /// Com o preview aberto, mover o cursor troca o arquivo mostrado (como o Quick Look).
    fn refresh_preview(&mut self) -> Task<Message> {
        match &self.overlay {
            Overlay::Preview { path, .. } if self.tab().current().is_some_and(|e| &e.path != path) => self.open_preview(),
            _ => Task::none(),
        }
    }

    // ---------- operações ----------

    fn start(&mut self, op: Op) -> Task<Message> {
        let id = self.next_job;
        self.next_job += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs.push(Job { id, label: op.label(), done: 0, total: 0, current: String::new(), cancel: cancel.clone() });
        Task::run(ops::spawn(op, cancel), move |event| Message::Job { id, event })
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) -> Task<Message> {
        self.next_toast += 1;
        let id = self.next_toast;
        self.toast = Some(Toast { id, text: text.into(), error });
        Task::perform(tokio::time::sleep(Duration::from_secs(if error { 7 } else { 4 })), move |_| Message::ToastExpired(id))
    }

    fn refresh_all(&mut self) -> Task<Message> {
        let tasks: Vec<_> = (0..2)
            .map(|p| {
                let path = self.panes[p].tab().path.clone();
                self.load(p, path, true)
            })
            .collect();
        Task::batch(tasks)
    }

    fn run(&mut self, cmd: Cmd) -> Task<Message> {
        let pane = self.active;
        // "Este Computador" é virtual: renomear, apagar ou colar ali agiria nas pastas e discos de verdade.
        let here = computer::is(&self.tab().path);
        let there = self.dual && computer::is(&self.panes[self.other_pane()].tab().path);
        let writes_here = matches!(cmd, Cmd::Paste | Cmd::Trash | Cmd::Rename | Cmd::NewFolder | Cmd::Cut | Cmd::MoveToOther);
        if (here && writes_here) || (there && matches!(cmd, Cmd::CopyToOther | Cmd::MoveToOther)) {
            return self.notify(format!("{} não aceita essa operação; abra uma pasta", computer::TITLE), true);
        }
        match cmd {
            Cmd::Open => match self.tab().current().cloned() {
                Some(e) => self.open_entry(pane, e),
                None => Task::none(),
            },
            Cmd::OpenDefault => match self.tab().current() {
                Some(e) => match launch::open_default(&e.path) {
                    Ok(()) => Task::none(),
                    Err(err) => self.notify(err, true),
                },
                None => Task::none(),
            },
            Cmd::Preview => {
                if matches!(self.overlay, Overlay::Preview { .. }) {
                    self.overlay = Overlay::None;
                    Task::none()
                } else {
                    self.open_preview()
                }
            }
            Cmd::OpenInNewTab => {
                let current_dir = self.tab().current().filter(|e| e.is_dir).map(|e| e.path.clone());
                match self.menu_target.clone().or(current_dir) {
                    Some(path) => self.new_tab_at(pane, path),
                    None => Task::none(),
                }
            }
            Cmd::Pin => {
                let current_dir = self.tab().current().filter(|e| e.is_dir).map(|e| e.path.clone());
                let path = self.menu_target.clone().or(current_dir).unwrap_or_else(|| self.tab().path.clone());
                if computer::is(&path) {
                    return Task::none();
                }
                let pinned = pins::toggle(&mut self.pins, &path);
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
                match pins::save(pins::file().as_deref(), &self.pins) {
                    Ok(()) => {
                        self.notify(if pinned { format!("{name} fixada na barra lateral") } else { format!("{name} desafixada") }, false)
                    }
                    Err(e) => self.notify(format!("Não salvou as pastas fixadas: {e}"), true),
                }
            }
            Cmd::Up => self.go_up(),
            Cmd::Back => self.history(true),
            Cmd::Forward => self.history(false),
            Cmd::Refresh => {
                let path = self.tab().path.clone();
                self.load(pane, path, true)
            }
            Cmd::NewTab => self.new_tab(pane),
            Cmd::CloseTab => {
                let i = self.panes[pane].active;
                self.close_tab(pane, i)
            }
            Cmd::NextTab | Cmd::PrevTab => {
                let p = &mut self.panes[pane];
                let n = p.tabs.len();
                p.active = if cmd == Cmd::NextTab { (p.active + 1) % n } else { (p.active + n - 1) % n };
                Task::none()
            }
            Cmd::ToggleDual => {
                self.dual = !self.dual;
                Task::none()
            }
            Cmd::SwitchPane => {
                if self.dual {
                    self.active = self.other_pane();
                }
                Task::none()
            }
            Cmd::CopyToOther | Cmd::MoveToOther => {
                if !self.dual {
                    self.dual = true;
                    return self.notify("Painel duplo aberto: escolha o destino e aperte F5/F6 de novo", false);
                }
                let sources = self.tab().targets();
                let dest = self.panes[self.other_pane()].tab().path.clone();
                if sources.is_empty() {
                    return Task::none();
                }
                self.start(if cmd == Cmd::CopyToOther { Op::Copy { sources, dest } } else { Op::Move { sources, dest } })
            }
            Cmd::Copy | Cmd::Cut => {
                let items = self.tab().targets();
                let n = items.len();
                if n == 0 {
                    return Task::none();
                }
                self.clip = Some((cmd == Cmd::Cut, items));
                let verb = if cmd == Cmd::Cut { "recortado(s)" } else { "copiado(s)" };
                self.notify(format!("{n} item(ns) {verb}. {} cola.", Cmd::Paste.shortcut()), false)
            }
            Cmd::Paste => {
                let Some((cut, sources)) = self.clip.clone() else { return Task::none() };
                let dest = self.tab().path.clone();
                if cut {
                    self.clip = None;
                    self.start(Op::Move { sources, dest })
                } else {
                    self.start(Op::Copy { sources, dest })
                }
            }
            Cmd::Undo => match self.undo.pop() {
                Some(u) => self.start(Op::Undo(u)),
                None => self.notify("Nada para desfazer", false),
            },
            Cmd::SelectAll => {
                let tab = self.tab_mut();
                tab.selected = tab.view.iter().map(|&i| tab.entries[i].path.clone()).collect();
                Task::none()
            }
            Cmd::Rename => match self.tab().current() {
                Some(e) => {
                    let value = e.name.clone();
                    self.open_prompt(PromptKind::Rename, value)
                }
                None => Task::none(),
            },
            Cmd::NewFolder => self.open_prompt(PromptKind::NewFolder, "Nova pasta".into()),
            Cmd::Trash => {
                let paths = self.tab().targets();
                if paths.is_empty() {
                    return Task::none();
                }
                self.start(Op::Trash { paths })
            }
            Cmd::Filter => {
                let id = self.tab().filter_id();
                op::focus(id)
            }
            Cmd::GoTo => {
                let path = if here { String::new() } else { self.tab().path.to_string_lossy().into_owned() };
                self.open_prompt(PromptKind::GoTo, path)
            }
            Cmd::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                let show = self.show_hidden;
                self.panes.iter_mut().flat_map(|p| p.tabs.iter_mut()).for_each(|t| {
                    t.view_filter.clear();
                    t.rebuild(show)
                });
                self.notify(if show { "Mostrando ocultos" } else { "Ocultos escondidos" }, false)
            }
            Cmd::ToggleGrid => {
                let tab = self.tab_mut();
                tab.grid = !tab.grid;
                tab.scroll_y = 0.0;
                let id = tab.id;
                let scroll = op::scroll_to(tab.scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 });
                let reveal = self.reveal_cursor(id);
                Task::batch([scroll, reveal, self.request_thumbs(id)])
            }
            Cmd::SortName => self.sort_by(SortKey::Name),
            Cmd::SortSize => self.sort_by(SortKey::Size),
            Cmd::SortModified => self.sort_by(SortKey::Modified),
            Cmd::SortKind => self.sort_by(SortKey::Kind),
            Cmd::ToggleTheme => {
                self.dark = !self.dark;
                Task::none()
            }
            Cmd::CopyPath => {
                let text = self.tab().targets().iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n");
                let text = if text.is_empty() { self.tab().path.to_string_lossy().into_owned() } else { text };
                Task::batch([iced::clipboard::write(text), self.notify("Caminho copiado", false)])
            }
            Cmd::Terminal => {
                match launch::terminal_here(&if here { dirs::home_dir().unwrap_or_default() } else { self.tab().path.clone() }) {
                    Ok(()) => Task::none(),
                    Err(e) => self.notify(format!("Terminal: {e}"), true),
                }
            }
            Cmd::Reveal => {
                let target = self.tab().current().map(|e| e.path.clone()).unwrap_or_else(|| self.tab().path.clone());
                match launch::reveal(&target) {
                    Ok(()) => Task::none(),
                    Err(e) => self.notify(e, true),
                }
            }
            Cmd::ToggleFps => {
                self.fps = if self.fps.is_some() { None } else { Some(Fps::default()) };
                Task::none()
            }
            Cmd::CheckUpdates => self.update(Message::UpdateCheck { manual: true }),
            Cmd::ToggleAutoUpdate => {
                let on = !update::auto_enabled();
                match update::set_auto_enabled(on) {
                    Ok(()) => self.notify(if on { "Atualização automática ligada" } else { "Atualização automática desligada" }, false),
                    Err(e) => self.notify(format!("Não salvou a preferência: {e}"), true),
                }
            }
            Cmd::Palette => {
                self.overlay = Overlay::Palette { query: String::new(), selected: 0 };
                op::focus(PALETTE_INPUT)
            }
        }
    }

    fn on_update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UpdateCheck { manual } => {
                if matches!(self.update, UpdateState::Working(_)) {
                    return Task::none();
                }
                let toast = if manual { self.notify("Procurando atualizações…", false) } else { Task::none() };
                let check = Task::perform(
                    async { tokio::task::spawn_blocking(update::check).await.map_err(|e| e.to_string()).and_then(|r| r) },
                    move |result| Message::UpdateChecked { manual, result },
                );
                Task::batch([toast, check])
            }
            Message::UpdateChecked { manual, result } => match result {
                Ok(Some(u)) => {
                    self.update = UpdateState::Available(u);
                    Task::none()
                }
                Ok(None) if manual => self.notify(format!("O arbor {} já é a versão mais nova", update::CURRENT), false),
                Err(e) if manual => self.notify(format!("Atualização: {e}"), true),
                _ => Task::none(),
            },
            Message::UpdateInstall => {
                let UpdateState::Available(u) = &self.update else { return Task::none() };
                if !update::can_self_update() {
                    let page = u.page.clone();
                    return match open::that_detached(&page) {
                        Ok(()) => self.notify("Esta cópia não pode se trocar sozinha; abri a página da release", false),
                        Err(e) => self.notify(format!("Abra {page}: {e}"), true),
                    };
                }
                let u = u.clone();
                self.update = UpdateState::Working(update::Progress::Downloading { received: 0, total: None });
                let (tx, rx) = iced::futures::channel::mpsc::unbounded();
                std::thread::spawn(move || {
                    let progress = tx.clone();
                    if let Err(e) = update::install(&u, &mut |p| {
                        let _ = progress.unbounded_send(p);
                    }) {
                        let _ = tx.unbounded_send(update::Progress::Failed(e));
                    }
                });
                Task::run(rx, Message::UpdateProgress)
            }
            Message::UpdateProgress(p) => {
                if let update::Progress::Failed(e) = &p {
                    let text = format!("A atualização falhou: {e}");
                    self.update = UpdateState::Idle;
                    return self.notify(text, true);
                }
                self.update = UpdateState::Working(p);
                Task::none()
            }
            Message::UpdateRestart => match update::restart() {
                Ok(()) => iced::exit(),
                Err(e) => self.notify(format!("Reabra o arbor para usar a versão nova ({e})"), true),
            },
            Message::UpdateDismiss => {
                if matches!(self.update, UpdateState::Working(update::Progress::Downloading { .. })) {
                    update::cancel();
                }
                self.update = UpdateState::Idle;
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn sort_by(&mut self, key: SortKey) -> Task<Message> {
        let id = self.tab().id;
        self.sort_tab(id, key)
    }

    fn sort_tab(&mut self, id: u64, key: SortKey) -> Task<Message> {
        let show = self.show_hidden;
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        tab.sort = if tab.sort.key == key { Sort { key, desc: !tab.sort.desc } } else { Sort { key, desc: key != SortKey::Name } };
        let sort = tab.sort;
        tab.edit_entries(show, |entries| sort_entries(Arc::<Vec<Entry>>::make_mut(entries), sort));
        self.reveal_cursor(id)
    }

    fn open_prompt(&mut self, kind: PromptKind, value: String) -> Task<Message> {
        self.overlay = Overlay::Prompt { kind, value };
        Task::batch([op::focus(PROMPT_INPUT), op::move_cursor_to_end(PROMPT_INPUT)])
    }

    fn new_tab(&mut self, pane: usize) -> Task<Message> {
        let path = self.panes[pane].tab().path.clone();
        self.new_tab_at(pane, path)
    }

    fn new_tab_at(&mut self, pane: usize, path: PathBuf) -> Task<Message> {
        let id = self.next_tab;
        self.next_tab += 1;
        let p = &mut self.panes[pane];
        let mut tab = Tab::new(id, path.clone());
        tab.sort = p.tab().sort;
        tab.grid = p.tab().grid;
        p.tabs.push(tab);
        p.active = p.tabs.len() - 1;
        self.active = pane;
        self.load(pane, path, false)
    }

    fn close_tab(&mut self, pane: usize, index: usize) -> Task<Message> {
        let p = &mut self.panes[pane];
        if p.tabs.len() == 1 {
            return Task::none();
        }
        p.tabs.remove(index);
        if p.active >= p.tabs.len() || p.active > index {
            p.active = p.active.saturating_sub(1);
        }
        Task::none()
    }

    fn palette_items(query: &str) -> Vec<Cmd> {
        if query.is_empty() {
            return commands::ALL.to_vec();
        }
        fuzzy::rank(query, commands::ALL.iter().map(|c| c.label())).into_iter().map(|i| commands::ALL[i]).collect()
    }

    pub fn palette_view(&self) -> Vec<Cmd> {
        match &self.overlay {
            Overlay::Palette { query, .. } => Self::palette_items(query),
            _ => vec![],
        }
    }

    fn submit_prompt(&mut self) -> Task<Message> {
        let Overlay::Prompt { kind, value } = std::mem::replace(&mut self.overlay, Overlay::None) else { return Task::none() };
        let value = value.trim().to_string();
        if value.is_empty() {
            return Task::none();
        }
        let dir = self.tab().path.clone();
        match kind {
            PromptKind::Rename => {
                let Some(from) = self.tab().current().map(|e| e.path.clone()) else { return Task::none() };
                if value.contains(['/', '\\']) {
                    return self.notify("O nome não pode ter barras", true);
                }
                let to = dir.join(&value);
                if to == from {
                    return Task::none();
                }
                self.start(Op::Rename { from, to })
            }
            PromptKind::NewFolder => {
                let path = ops::unique_dest(&dir, &value);
                self.start(Op::Mkdir { path })
            }
            PromptKind::GoTo => {
                let expanded = if let Some(rest) = value.strip_prefix('~') {
                    dirs::home_dir().map(|h| h.join(rest.trim_start_matches(['/', '\\']))).unwrap_or_else(|| PathBuf::from(&value))
                } else {
                    PathBuf::from(&value)
                };
                let target = if expanded.is_relative() { dir.join(expanded) } else { expanded };
                if target.is_dir() {
                    let pane = self.active;
                    self.navigate(pane, target)
                } else if let Some(parent) = target.parent().filter(|_| target.exists()) {
                    let pane = self.active;
                    let task = self.navigate(pane, parent.to_path_buf());
                    self.tab_mut().focus_on_load = vec![target];
                    task
                } else {
                    self.notify(format!("Caminho não encontrado: {}", target.display()), true)
                }
            }
        }
    }

    fn on_key(&mut self, key: Key, mods: Modifiers, text: Option<String>, captured: bool) -> Task<Message> {
        let named = match &key {
            Key::Named(n) => Some(*n),
            _ => None,
        };
        // Overlays primeiro.
        match &mut self.overlay {
            Overlay::Palette { query, selected } => {
                let n = Self::palette_items(query).len();
                match named {
                    Some(Named::Escape) => self.overlay = Overlay::None,
                    Some(Named::ArrowDown) if n > 0 => *selected = (*selected + 1) % n,
                    Some(Named::ArrowUp) if n > 0 => *selected = (*selected + n - 1) % n,
                    _ => {}
                }
                return Task::none();
            }
            Overlay::Prompt { .. } => {
                if named == Some(Named::Escape) {
                    self.overlay = Overlay::None;
                }
                return Task::none();
            }
            Overlay::Preview { .. } => {
                return match named {
                    Some(Named::Escape | Named::Space) => {
                        self.overlay = Overlay::None;
                        Task::none()
                    }
                    Some(Named::ArrowDown) => self.move_cursor(if self.tab().grid { 100 } else { 1 }, false),
                    Some(Named::ArrowUp) => self.move_cursor(if self.tab().grid { -100 } else { -1 }, false),
                    Some(Named::ArrowRight) => self.move_cursor(1, false),
                    Some(Named::ArrowLeft) => self.move_cursor(-1, false),
                    Some(Named::Enter) => {
                        self.overlay = Overlay::None;
                        self.run(Cmd::Open)
                    }
                    _ => Task::none(),
                };
            }
            Overlay::Menu(_) => return self.menu_key(named),
            Overlay::None => {}
        }

        let cmd_mod = mods.command();
        let grid = self.tab().grid;
        // Teclas que funcionam mesmo com o filtro focado.
        match named {
            Some(Named::Escape) => {
                let tab = self.tab_mut();
                if !tab.filter.is_empty() {
                    tab.filter.clear();
                    let show = self.show_hidden;
                    self.tab_mut().rebuild(show);
                }
                return iced::advanced::widget::operate(focusable::unfocus());
            }
            Some(Named::ArrowDown) => return self.move_cursor(if grid { 100 } else { 1 }, mods.shift()),
            Some(Named::ArrowUp) => return self.move_cursor(if grid { -100 } else { -1 }, mods.shift()),
            Some(Named::PageDown) => return self.move_cursor(1000, mods.shift()),
            Some(Named::PageUp) => return self.move_cursor(-1000, mods.shift()),
            Some(Named::Tab) if mods.control() => return self.run(if mods.shift() { Cmd::PrevTab } else { Cmd::NextTab }),
            Some(Named::ContextMenu) => return self.keyboard_menu(),
            Some(Named::F10) if mods.shift() => return self.keyboard_menu(),
            Some(Named::F2) => return self.run(Cmd::Rename),
            Some(Named::F3) => return self.run(Cmd::ToggleDual),
            Some(Named::F5) => return self.run(Cmd::CopyToOther),
            Some(Named::F6) => return self.run(Cmd::MoveToOther),
            Some(Named::F7) => return self.run(Cmd::NewFolder),
            Some(Named::F8) => return self.run(Cmd::Trash),
            Some(Named::F12) => return self.run(Cmd::ToggleFps),
            _ => {}
        }
        if cmd_mod {
            if let Key::Character(c) = &key {
                let c = c.to_lowercase();
                let cmd = match (c.as_str(), mods.shift()) {
                    ("p", true) | ("k", false) => Some(Cmd::Palette),
                    ("c", true) => Some(Cmd::CopyPath),
                    ("n", true) => Some(Cmd::NewFolder),
                    ("t", false) => Some(Cmd::NewTab),
                    ("w", false) => Some(Cmd::CloseTab),
                    ("r", false) => Some(Cmd::Refresh),
                    ("l", false) => Some(Cmd::GoTo),
                    ("f", false) => Some(Cmd::Filter),
                    ("h", false) => Some(Cmd::ToggleHidden),
                    ("g", false) => Some(Cmd::ToggleGrid),
                    ("1", _) => Some(Cmd::SortName),
                    ("2", _) => Some(Cmd::SortSize),
                    ("3", _) => Some(Cmd::SortModified),
                    ("4", _) => Some(Cmd::SortKind),
                    ("'", _) => Some(Cmd::Terminal),
                    ("\\", _) => Some(Cmd::ToggleDual),
                    // Ctrl+C/X/V/Z/A dentro de um campo de texto são do campo.
                    ("c", false) if !captured => Some(Cmd::Copy),
                    ("x", false) if !captured => Some(Cmd::Cut),
                    ("v", false) if !captured => Some(Cmd::Paste),
                    ("z", false) if !captured => Some(Cmd::Undo),
                    ("a", false) if !captured => Some(Cmd::SelectAll),
                    _ => None,
                };
                return cmd.map_or_else(Task::none, |c| self.run(c));
            }
        }
        if captured {
            return Task::none();
        }
        match named {
            Some(Named::Enter) => return self.run(if mods.shift() { Cmd::OpenDefault } else { Cmd::Open }),
            Some(Named::Space) => return self.run(Cmd::Preview),
            Some(Named::Backspace) => return self.go_up(),
            Some(Named::Delete) => return self.run(Cmd::Trash),
            Some(Named::Tab) => return self.run(Cmd::SwitchPane),
            Some(Named::Home) => return self.move_cursor(isize::MIN, mods.shift()),
            Some(Named::End) => return self.move_cursor(isize::MAX, mods.shift()),
            Some(Named::ArrowLeft) if mods.alt() => return self.run(Cmd::Back),
            Some(Named::ArrowRight) if mods.alt() => return self.run(Cmd::Forward),
            Some(Named::ArrowUp) if mods.alt() => return self.go_up(),
            Some(Named::ArrowRight) if grid => return self.move_cursor(1, mods.shift()),
            Some(Named::ArrowLeft) if grid => return self.move_cursor(-1, mods.shift()),
            Some(Named::ArrowRight) => return self.run(Cmd::Open),
            Some(Named::ArrowLeft) => return self.go_up(),
            _ => {}
        }
        // Digitar filtra a pasta na hora, sem precisar clicar no campo.
        if let Some(t) = text.filter(|t| !mods.alt() && !cmd_mod && t.chars().all(|c| !c.is_control()) && !t.trim().is_empty()) {
            let show = self.show_hidden;
            let tab = self.tab_mut();
            tab.filter.push_str(&t);
            tab.rebuild(show);
            let id = tab.filter_id();
            let tab_id = tab.id;
            return Task::batch([op::focus(id.clone()), op::move_cursor_to_end(id), self.reveal_cursor(tab_id)]);
        }
        Task::none()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Listed { tab, generation, snap } => self.on_listed(tab, generation, snap),
            Message::Frame(at) => {
                if let Some(fps) = &mut self.fps {
                    fps.push(at);
                }
                Task::none()
            }
            m @ (Message::UpdateCheck { .. }
            | Message::UpdateChecked { .. }
            | Message::UpdateInstall
            | Message::UpdateProgress(_)
            | Message::UpdateRestart
            | Message::UpdateDismiss) => self.on_update(m),
            Message::Key { key, modifiers, text, captured } => self.on_key(key, modifiers, text, captured),
            Message::Modifiers(m) => {
                self.modifiers = m;
                Task::none()
            }
            Message::Resized(size) => {
                self.window = size;
                let ids: Vec<u64> = self.panes.iter().map(|p| p.tab().id).collect();
                Task::batch(ids.into_iter().map(|id| self.request_thumbs(id)).collect::<Vec<_>>())
            }
            Message::Dropped(path) => {
                let dest = self.tab().path.clone();
                if computer::is(&dest) || path.parent() == Some(dest.as_path()) {
                    return Task::none();
                }
                self.start(Op::Copy { sources: vec![path], dest })
            }
            Message::Run(cmd) => self.run(cmd),
            Message::ContextMenu { tab, index } => {
                if let Some(pane) = self.pane_of(tab) {
                    self.active = pane;
                }
                let Some(t) = self.find_tab(tab) else { return Task::none() };
                let Some(i) = index else {
                    let target = Some(t.path.clone());
                    return self.open_menu(MenuKind::Background, target);
                };
                let Some(entry) = t.entry_at(i).cloned() else { return Task::none() };
                // Como no Explorer: botão direito fora da seleção seleciona só o item clicado.
                let task = if t.selected.contains(&entry.path) {
                    t.cursor = i;
                    Task::none()
                } else {
                    let saved = std::mem::take(&mut self.modifiers);
                    let task = self.click(tab, i);
                    self.modifiers = saved;
                    task
                };
                Task::batch([task, self.open_menu(MenuKind::Item { dir: entry.is_dir }, Some(entry.path))])
            }
            Message::SidebarMenu(path) => self.open_menu(MenuKind::Sidebar, Some(path)),
            Message::OpenMenu(kind) => self.open_menu(kind, None),
            Message::MenuRun(cmd) => self.run_from_menu(cmd),
            Message::MenuHover(i) => {
                if let Overlay::Menu(m) = &mut self.overlay {
                    m.selected = Some(i);
                }
                Task::none()
            }
            Message::Click { tab, index } => self.click(tab, index),
            Message::Activate { tab, index } => {
                let Some(pane) = self.pane_of(tab) else { return Task::none() };
                let entry = self.find_tab(tab).and_then(|t| t.entry_at(index).cloned());
                match entry {
                    Some(e) => self.open_entry(pane, e),
                    None => Task::none(),
                }
            }
            Message::Scrolled { tab, viewport } => {
                if let Some(t) = self.find_tab(tab) {
                    t.scroll_y = viewport.absolute_offset().y;
                    t.viewport_h = viewport.bounds().height;
                }
                self.request_thumbs(tab)
            }
            Message::Navigate(path) => {
                let pane = self.active;
                self.navigate(pane, path)
            }
            Message::NavigateIn { tab, path } => {
                let Some(pane) = self.pane_of(tab) else { return Task::none() };
                self.active = pane;
                self.navigate(pane, path)
            }
            Message::SelectTab { pane, index } => {
                self.active = pane;
                self.panes[pane].active = index.min(self.panes[pane].tabs.len() - 1);
                Task::none()
            }
            Message::CloseTabAt { pane, index } => self.close_tab(pane, index),
            Message::NewTabIn(pane) => self.new_tab(pane),
            Message::FocusPane(pane) => {
                self.active = pane;
                Task::none()
            }
            Message::SortBy { tab, key } => self.sort_tab(tab, key),
            Message::FilterChanged { tab, value } => {
                let show = self.show_hidden;
                if let Some(pane) = self.pane_of(tab) {
                    self.active = pane;
                }
                if let Some(t) = self.find_tab(tab) {
                    t.filter = value;
                    t.rebuild(show);
                    t.scroll_y = 0.0;
                    let scroll = op::scroll_to(t.scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 });
                    return Task::batch([scroll, self.request_thumbs(tab)]);
                }
                Task::none()
            }
            Message::FilterSubmit => {
                let task = self.run(Cmd::Open);
                Task::batch([task, iced::advanced::widget::operate(focusable::unfocus())])
            }
            Message::PaletteQuery(q) => {
                if let Overlay::Palette { query, selected } = &mut self.overlay {
                    *query = q;
                    *selected = 0;
                }
                Task::none()
            }
            Message::PaletteRun(index) => {
                let Overlay::Palette { query, selected } = &self.overlay else { return Task::none() };
                let i = if index == usize::MAX { *selected } else { index };
                let cmd = Self::palette_items(query).get(i).copied();
                self.overlay = Overlay::None;
                cmd.map_or_else(Task::none, |c| self.run(c))
            }
            Message::PromptChanged(v) => {
                if let Overlay::Prompt { value, .. } = &mut self.overlay {
                    *value = v;
                }
                Task::none()
            }
            Message::PromptSubmit => self.submit_prompt(),
            Message::CloseOverlay => {
                self.overlay = Overlay::None;
                Task::none()
            }
            Message::PreviewLoaded { path, content: loaded } => {
                if let Overlay::Preview { path: p, content } = &mut self.overlay {
                    if *p == path {
                        *content = Some(loaded);
                    }
                }
                Task::none()
            }
            Message::Thumb { path, handle } => {
                if handle.is_some() {
                    self.thumbs.insert(path, handle);
                }
                Task::none()
            }
            Message::Job { id, event } => match event {
                JobEvent::Progress { done, total, current } => {
                    if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
                        (j.done, j.total, j.current) = (done, total, current);
                    }
                    Task::none()
                }
                JobEvent::Finished(result) => {
                    self.jobs.retain(|j| j.id != id);
                    let refresh = self.refresh_all();
                    let toast = match result {
                        Ok(outcome) => {
                            if let Some(u) = outcome.undo {
                                self.undo.push(u);
                                if self.undo.len() > 100 {
                                    self.undo.remove(0);
                                }
                            }
                            let tab = self.tab_mut();
                            tab.focus_on_load = outcome.created.into_iter().filter(|p| p.parent() == Some(tab.path.as_path())).collect();
                            self.notify(outcome.summary, false)
                        }
                        Err(e) => self.notify(format!("Erro: {e}"), true),
                    };
                    Task::batch([refresh, toast])
                }
            },
            Message::CancelJob(id) => {
                if let Some(j) = self.jobs.iter().find(|j| j.id == id) {
                    j.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                Task::none()
            }
            Message::ToastExpired(id) => {
                if self.toast.as_ref().is_some_and(|t| t.id == id) {
                    self.toast = None;
                }
                Task::none()
            }
            Message::DirChanged(path) => {
                let tasks: Vec<_> = (0..2)
                    .filter(|&p| self.panes[p].tab().path == path && !self.panes[p].tab().loading)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|p| self.load(p, path.clone(), true))
                    .collect();
                Task::batch(tasks)
            }
            Message::Places(drives) => {
                self.drives = drives;
                let snap = Snapshot {
                    entries: Arc::new(computer::entries(&self.places, &self.drives)),
                    complete: true,
                    elapsed: Duration::ZERO,
                    error: None,
                };
                let tabs: Vec<_> =
                    self.panes.iter().flat_map(|p| &p.tabs).filter(|t| computer::is(&t.path)).map(|t| (t.id, t.generation)).collect();
                Task::batch(tabs.into_iter().map(|(id, generation)| self.on_listed(id, generation, snap.clone())).collect::<Vec<_>>())
            }
        }
    }
}

/// `arbor arquivo.txt` abre a pasta do arquivo em vez de tentar listar um arquivo.
fn start_dir(p: PathBuf) -> Option<PathBuf> {
    if p.is_dir() {
        Some(p)
    } else {
        p.parent().map(Path::to_path_buf)
    }
}

fn scan_drives() -> Task<Message> {
    Task::perform(async { tokio::task::spawn_blocking(computer::drives).await.unwrap_or_default() }, Message::Places)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> Entry {
        Entry::synthetic(name, false, 0)
    }

    #[test]
    fn trocar_entradas_mantem_cursor_e_nao_estoura() {
        let mut tab = Tab::new(0, PathBuf::from("/"));
        tab.edit_entries(false, |e| *e = ["a", "b", "c", "d", "e"].map(entry).to_vec().into());
        tab.cursor = 4;
        // Atualização com menos itens (um arquivo apagado): antes isso estourava o índice.
        tab.edit_entries(false, |e| *e = ["a", "e"].map(entry).to_vec().into());
        assert_eq!(tab.current().unwrap().name, "e");
        tab.edit_entries(false, |e| *e = vec![entry("a")].into());
        assert_eq!(tab.cursor, 0);
    }

    #[test]
    fn argumento_arquivo_abre_a_pasta_dele() {
        let dir = std::env::temp_dir();
        let file = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        assert_eq!(start_dir(dir.clone()), Some(dir));
        assert_eq!(start_dir(file.clone()), file.parent().map(Path::to_path_buf));
    }

    #[test]
    fn filtro_fuzzy_na_aba() {
        let mut tab = Tab::new(0, PathBuf::from("/"));
        tab.edit_entries(false, |e| *e = ["Cargo.toml", "README.md", "src"].map(entry).to_vec().into());
        tab.filter = "rdm".into();
        tab.rebuild(false);
        assert_eq!(tab.view.len(), 1);
        assert_eq!(tab.current().unwrap().name, "README.md");
    }
}
