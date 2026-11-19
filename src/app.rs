//! Estado e lógica do app (o `update` do iced). A interface fica em `view.rs`.

use crate::commands::{self, Cmd};
mod address;
mod details;
mod drag;
mod jump;
mod menu;
mod trash_view;
mod tree;

pub use details::Details;
pub use drag::{valid_target as drag_valid, Gesture};
pub use jump::{Jump, JumpKind};
pub use menu::{Menu, MenuKind};

use crate::computer::{self, Drive};
use crate::media::{self, Preview};
use crate::session::{self, Session};
use crate::{instance, keymap, launch, listing, pins, pointer, trashbin, update, watch};
use arbor_core::frecency::Frecency;
use arbor_core::ops::{self, Conflict, JobEvent, Op, Undo};
use arbor_core::{archive, fuzzy, rename, sort_entries, Entry, Snapshot, Sort, SortKey};
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
/// Tamanhos de ícone da grade (pequeno, médio, grande, extra grande), trocados com Ctrl+roda.
pub const ZOOMS: [f32; 4] = [0.75, 1.0, 1.35, 1.8];
pub const SIDEBAR_W: f32 = 216.0;

pub struct App {
    pub panes: [Pane; 2],
    pub active: usize,
    pub dual: bool,
    pub overlay: Overlay,
    pub jobs: Vec<Job>,
    next_job: u64,
    /// Desfazer: o que reverte cada operação e a própria operação (para o refazer).
    undo: Vec<(Undo, Op)>,
    /// Operações desfeitas que Ctrl+Y refaz. Uma operação nova limpa a pilha.
    redo: Vec<Op>,
    clip: Option<(bool, Vec<PathBuf>)>,
    sysclip: crate::sysclip::SysClip,
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
    views: crate::folder_views::FolderViews,
    pub columns: crate::columns::Columns,
    /// Alvo do menu de onde o comando atual veio (fixar ou abrir uma pasta da barra lateral).
    menu_target: Option<PathBuf>,
    pub suggest: address::Suggest,
    pub details_on: bool,
    pub details: Option<Details>,
    /// Índice em `ZOOMS`.
    pub zoom: usize,
    pub gesture: Gesture,
    pub tree: tree::Tree,
    /// Caminho-sentinela de "Este Computador", guardado para a barra lateral poder emprestá-lo.
    pub computer: PathBuf,
    pub trash: PathBuf,
    next_tab: u64,
    pub update: UpdateState,
    pub fps: Option<Fps>,
    /// Pastas visitadas, para o Ctrl+K.
    frecency: Frecency,
    /// Abas fechadas (painel, pasta, grade), da mais antiga para a mais recente: Ctrl+Shift+T reabre.
    closed: Vec<(usize, PathBuf, bool)>,
    /// Última sessão gravada; só regrava quando muda.
    saved: Option<Session>,
    /// "Restaurar abas ao abrir" ligado (lido do disco uma vez, na abertura).
    restore: bool,
    /// Vidro nativo ativo (vibrancy/transparência): a aurora fica translúcida e o fundo da janela some.
    pub glass: bool,
    /// Textura da aurora para o tema e o vidro atuais.
    pub aurora: iced::widget::image::Handle,
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
    /// Aba de resultados de busca: consulta e se inclui ocultos. `path` é a pasta onde a busca começou.
    pub search: Option<(String, bool)>,
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
            search: None,
        }
    }

    pub fn title(&self) -> String {
        if let Some((q, _)) = &self.search {
            return format!("Busca: {q}");
        }
        if let Some(t) = computer::virtual_title(&self.path) {
            return t.into();
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
    /// Esperando outra cópia/movimentação terminar (a fila roda uma transferência por vez).
    pub queued: bool,
    pub paused: bool,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    op: Op,
    kind: JobKind,
}

enum JobKind {
    Normal,
    /// Um desfazer; se der certo, `redo` vai para a pilha do Ctrl+Y.
    Undo {
        redo: Op,
    },
}

pub struct Toast {
    pub id: u64,
    pub text: String,
    pub error: bool,
}

pub enum Overlay {
    None,
    /// Itens com o mesmo nome no destino: substituir, pular ou manter os dois.
    Conflict {
        op: Op,
        names: Vec<String>,
    },
    Palette {
        query: String,
        selected: usize,
    },
    /// Ctrl+K: pular para uma pasta frequente digitando parte do nome.
    Frecent {
        query: String,
        selected: usize,
    },
    Prompt {
        kind: PromptKind,
        value: String,
    },
    Preview {
        path: PathBuf,
        content: Option<Preview>,
    },
    Menu(Menu),
    Jump(jump::Jump),
    /// Pergunta antes do que não tem volta (excluir de vez, esvaziar a Lixeira).
    Confirm {
        text: String,
        action: trash_view::Irreversible,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Search,
    Rename,
    /// F2 com vários itens selecionados: modelo ("Foto {n:3}") ou troca ("antigo => novo").
    BatchRename,
    NewFolder,
    NewFile,
    GoTo,
    OpenWith,
}

impl PromptKind {
    pub fn title(self) -> &'static str {
        match self {
            PromptKind::Search => "Buscar nesta pasta e subpastas (palavras, ou *.pdf)",
            PromptKind::Rename => "Renomear",
            PromptKind::BatchRename => "Renomear em lote",
            PromptKind::NewFolder => "Nova pasta",
            PromptKind::NewFile => "Novo documento de texto",
            PromptKind::GoTo => "Ir para",
            PromptKind::OpenWith => "Abrir com (programa)",
        }
    }
}

pub const PALETTE_INPUT: &str = "palette-input";
pub const JUMP_INPUT: &str = "jump-input";
pub const PROMPT_INPUT: &str = "prompt-input";

#[derive(Debug, Clone)]
pub enum Message {
    Listed {
        tab: u64,
        generation: u64,
        snap: Snapshot,
    },
    Key {
        key: Key,
        modifiers: Modifiers,
        text: Option<String>,
        captured: bool,
    },
    Modifiers(Modifiers),
    Resized(Size),
    Dropped(PathBuf),
    Run(Cmd),
    ContextMenu {
        tab: u64,
        index: Option<usize>,
    },
    SidebarMenu(PathBuf),
    OpenMenu(MenuKind),
    MenuRun(Cmd),
    JumpOpen {
        kind: JumpKind,
        dir: Option<PathBuf>,
    },
    JumpListed {
        generation: u64,
        dirs: Vec<PathBuf>,
    },
    JumpPick(usize),
    JumpHover(usize),
    MenuHover(usize),
    Suggested {
        generation: u64,
        items: Vec<PathBuf>,
    },
    SuggestPick(usize),
    Zoom(bool),
    Created(Result<PathBuf, String>),
    TreeToggle(PathBuf),
    TrashListed(Result<Vec<Entry>, String>),
    TrashDone(Result<String, String>),
    Confirmed,
    Ejected(Result<(), String>),
    /// Varredura periódica de discos: só atualiza a tela se um volume entrou ou saiu.
    DrivesPolled(Vec<Drive>),
    DrivesTick,
    TreeLoaded {
        path: PathBuf,
        dirs: Vec<PathBuf>,
    },
    PressItem {
        tab: u64,
        index: usize,
    },
    PressEmpty(u64),
    BandMove {
        tab: u64,
        at: iced::Point,
    },
    PointerMoved(iced::Point),
    PointerUp,
    ColResizeStart(crate::columns::Col),
    HeaderMenu,
    DragOver(Option<PathBuf>),
    DragLeave {
        from: PathBuf,
        back: Option<PathBuf>,
    },
    DetailsLoaded {
        path: PathBuf,
        preview: Preview,
        props: Vec<(String, String)>,
    },
    Click {
        tab: u64,
        index: usize,
    },
    Activate {
        tab: u64,
        index: usize,
    },
    Scrolled {
        tab: u64,
        viewport: Viewport,
    },
    Navigate(PathBuf),
    NavigateIn {
        tab: u64,
        path: PathBuf,
    },
    SelectTab {
        pane: usize,
        index: usize,
    },
    CloseTabAt {
        pane: usize,
        index: usize,
    },
    NewTabIn(usize),
    FocusPane(usize),
    SortBy {
        tab: u64,
        key: SortKey,
    },
    FilterChanged {
        tab: u64,
        value: String,
    },
    FilterSubmit,
    PaletteQuery(String),
    JumpQuery(String),
    JumpRun(usize),
    PaletteRun(usize),
    PromptChanged(String),
    PromptSubmit,
    CloseOverlay,
    PreviewLoaded {
        path: PathBuf,
        content: Preview,
    },
    Thumb {
        path: PathBuf,
        handle: Option<Handle>,
    },
    Job {
        id: u64,
        event: JobEvent,
    },
    CancelJob(u64),
    /// Texto para o aviso do rodapé, vindo de uma tarefa em segundo plano.
    Info(String),
    /// Resultado de abrir um arquivo de dentro de um compactado.
    Opened {
        name: String,
        result: Result<(), String>,
    },
    PauseJob(u64),
    /// Resposta do diálogo de conflito; `None` cancela.
    ResolveConflict(Option<Conflict>),
    ToastExpired(u64),
    DirChanged(PathBuf),
    Places(Vec<Drive>),
    Glass(bool),
    Frame(Instant),
    UpdateCheck {
        manual: bool,
    },
    UpdateChecked {
        manual: bool,
        result: Result<Option<update::Update>, String>,
    },
    UpdateInstall,
    UpdateProgress(update::Progress),
    UpdateRestart,
    UpdateDismiss,
    /// Outra execução do arbor pediu esta pasta (ou só uma janela nova, se `None`).
    Remote(Option<PathBuf>),
}

impl App {
    pub fn boot() -> (App, Task<Message>) {
        let start = instance::start_arg().and_then(|p| std::fs::canonicalize(p).ok()).and_then(start_dir).unwrap_or_else(computer::path);
        let second = dirs::download_dir().filter(|d| d.is_dir()).unwrap_or_else(|| start.clone());
        let has_arg = std::env::args_os().nth(1).is_some();
        let restore = session::restore_enabled();
        let restored = if !has_arg && restore { Session::load() } else { None };
        let mut app = App {
            panes: [
                Pane { tabs: vec![Tab::new(0, start.clone())], active: 0 },
                Pane { tabs: vec![Tab::new(1, second.clone())], active: 0 },
            ],
            active: 0,
            dual: false,
            closed: vec![],
            saved: None,
            restore,
            overlay: Overlay::None,
            jobs: vec![],
            next_job: 0,
            undo: vec![],
            redo: vec![],
            clip: None,
            sysclip: Default::default(),
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
            views: crate::folder_views::FolderViews::load(crate::folder_views::FolderViews::default_file()),
            columns: crate::columns::Columns::load(crate::columns::Columns::file().as_deref()),
            menu_target: None,
            suggest: Default::default(),
            details_on: false,
            details: None,
            zoom: 1,
            gesture: Gesture::None,
            tree: Default::default(),
            computer: computer::path(),
            trash: trashbin::path(),
            next_tab: 2,
            update: UpdateState::Idle,
            fps: None,
            frecency: load_frecency(),
            glass: false,
            aurora: crate::aurora::render(true, 1.0),
        };
        let lists = match restored {
            Some(session) => app.restore(session),
            None => Task::batch([app.load(0, start, false), app.load(1, second, false)]),
        };
        let tasks = Task::batch([
            window::oldest().and_then(|id| window::run(id, crate::glass::apply)).map(Message::Glass),
            lists,
            scan_drives(),
            if update::auto_enabled() {
                Task::perform(tokio::time::sleep(update::FIRST_CHECK), |_| Message::UpdateCheck { manual: false })
            } else {
                Task::none()
            },
        ]);
        (app, tasks)
    }

    /// Recria as abas da sessão salva e já começa a listar todas elas.
    fn restore(&mut self, session: Session) -> Task<Message> {
        self.dual = session.dual;
        self.active = session.active;
        let mut tasks = Vec::new();
        for (pane, (active, tabs)) in session.panes.into_iter().enumerate() {
            let mut list = Vec::new();
            for (path, grid) in tabs {
                let mut tab = Tab::new(self.next_tab, path.clone());
                self.next_tab += 1;
                tab.grid = grid;
                tasks.push(Self::load_tab(&mut tab, path, false));
                list.push(tab);
            }
            self.panes[pane] = Pane { tabs: list, active };
        }
        Task::batch(tasks)
    }

    fn session(&self) -> Session {
        let pane = |p: &Pane| (p.active, p.tabs.iter().map(|t| (t.path.clone(), t.grid)).collect());
        Session { dual: self.dual, active: self.active, panes: [pane(&self.panes[0]), pane(&self.panes[1])] }
    }

    /// Grava a sessão se ela mudou (e se restaurar estiver ligado). Barato: compara poucas strings.
    fn persist(&mut self) {
        if !self.restore {
            return;
        }
        let now = self.session();
        if self.saved.as_ref() != Some(&now) {
            now.save();
            self.saved = Some(now);
        }
    }

    pub fn title(&self) -> String {
        format!("{} — arbor", self.tab().title())
    }

    /// Mais translúcida com vidro nativo, para o material do sistema aparecer; opaca sem ele.
    fn refresh_aurora(&mut self) {
        self.aurora = crate::aurora::render(self.dark, if self.glass { 0.62 } else { 1.0 });
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
        ((width / self.cell_w()).floor() as usize).max(1)
    }

    pub fn cell_w(&self) -> f32 {
        CELL_W * ZOOMS[self.zoom]
    }

    /// Só a miniatura (96 px no médio) cresce; nome e tamanho embaixo mantêm a altura.
    pub fn cell_h(&self) -> f32 {
        CELL_H + 96.0 * (ZOOMS[self.zoom] - 1.0)
    }

    /// Ctrl+roda como no Explorer: aumenta os ícones; abaixo do menor volta para a lista, acima da lista vira grade.
    fn zoom(&mut self, larger: bool) -> Task<Message> {
        let grid = self.tab().grid;
        match (grid, larger) {
            (false, true) => {
                self.zoom = 0;
                return self.run(Cmd::ToggleGrid);
            }
            (false, false) => return Task::none(),
            (true, false) if self.zoom == 0 => return self.run(Cmd::ToggleGrid),
            (true, false) => self.zoom -= 1,
            (true, true) => self.zoom = (self.zoom + 1).min(ZOOMS.len() - 1),
        }
        let id = self.tab().id;
        Task::batch([self.reveal_cursor(id), self.request_thumbs(id)])
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keys = event::listen_with(|event, status, _| match event {
            Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) if pointer::ctrl() => {
                let y = match delta {
                    iced::mouse::ScrollDelta::Lines { y, .. } | iced::mouse::ScrollDelta::Pixels { y, .. } => y,
                };
                (y != 0.0).then_some(Message::Zoom(y > 0.0))
            }
            Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                pointer::track(&event);
                pointer::armed().then_some(Message::PointerMoved(position))
            }
            Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) if pointer::armed() => Some(Message::PointerUp),
            // Botões laterais do mouse voltam e avançam.
            Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Back)) => Some(Message::Run(Cmd::Back)),
            Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Forward)) => Some(Message::Run(Cmd::Forward)),
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, text, .. }) => {
                Some(Message::Key { key, modifiers, text: text.map(|t| t.to_string()), captured: status == event::Status::Captured })
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => {
                pointer::set_ctrl(m.command());
                Some(Message::Modifiers(m))
            }
            Event::Window(window::Event::Resized(size)) => Some(Message::Resized(size)),
            Event::Window(window::Event::FileDropped(path)) => Some(Message::Dropped(path)),
            _ => None,
        });
        let visible = if self.dual { &self.panes[..] } else { &self.panes[self.active..=self.active] };
        let watchers = visible
            .iter()
            .filter(|p| !computer::is_virtual(&p.tab().path))
            .map(|p| Subscription::run_with(p.tab().path.clone(), watch::watch).map(Message::DirChanged));
        let fps = if self.fps.is_some() { window::frames().map(Message::Frame) } else { Subscription::none() };
        let updates = if update::auto_enabled() {
            iced::time::every(update::EVERY).map(|_| Message::UpdateCheck { manual: false })
        } else {
            Subscription::none()
        };
        let remote = Subscription::run(instance::requests).map(Message::Remote);
        let drives = iced::time::every(DRIVES_EVERY).map(|_| Message::DrivesTick);
        Subscription::batch(std::iter::once(keys).chain(watchers).chain([fps, updates, drives, remote]))
    }

    // ---------- navegação e listagem ----------

    /// Lista `path` na aba ativa do painel `pane`. `refresh` mantém os itens atuais até a nova lista chegar.
    fn load(&mut self, pane: usize, path: PathBuf, refresh: bool) -> Task<Message> {
        let view = (!refresh).then(|| self.views.get(&path));
        let tab = self.panes[pane].tab_mut();
        if let Some(v) = view {
            tab.sort = v.sort;
            tab.grid = v.grid;
        }
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
        if trashbin::is(&path) {
            return Task::batch([scan_trash(), scroll]);
        }
        let stream = match (tab.search.clone(), archive::split(&path)) {
            (Some((query, hidden)), _) if refresh => {
                Task::run(listing::search(path, query, sort, hidden), move |snap| Message::Listed { tab: id, generation, snap })
            }
            (_, Some((file, inner))) => {
                tab.search = None;
                Task::run(listing::list_archive(file, inner, sort), move |snap| Message::Listed { tab: id, generation, snap })
            }
            _ => {
                tab.search = None;
                Task::run(listing::list(path, sort), move |snap| Message::Listed { tab: id, generation, snap })
            }
        };
        Task::batch([stream, scroll])
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
        if computer::is_virtual(&tab.path) {
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
        // Compactados abrem como pasta (Shift+Enter abre no app do sistema).
        if entry.is_dir || (archive::is_archive(&entry.path) && entry.path.is_file()) {
            self.navigate(pane, entry.path)
        } else if let Some((file, inner)) = archive::split(&entry.path) {
            // Arquivo de dentro de um compactado: extrai para uma pasta temporária e abre de lá.
            let name = entry.name.clone();
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let dir = std::env::temp_dir().join(format!("arbor-{}", std::process::id()));
                        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                        let out = archive::extract(&file, &[inner], &dir, |_, _| true).map_err(|e| e.to_string())?;
                        let path = out.into_iter().next().ok_or("nada extraído")?;
                        launch::open(&path)
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                },
                move |r| Message::Opened { name: name.clone(), result: r },
            )
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
            if !tab.refreshing && snap.error.is_none() {
                let path = tab.path.clone();
                self.frecency.visit(&path);
                save_frecency(&self.frecency);
            }
            let Some(tab) = self.find_tab(id) else { return Task::none() };
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
        let page = ((self.tab().viewport_h / if self.tab().grid { self.cell_h() } else { ROW_H }) as isize).max(1) * cols;
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
        let (cols, cell_h) = (self.grid_cols(), self.cell_h());
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        let (row, h) = if tab.grid { (tab.cursor / cols, cell_h) } else { (tab.cursor, ROW_H) };
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
        let (cols, cell_h) = (self.grid_cols(), self.cell_h());
        let Some(tab) = self.panes.iter().flat_map(|p| p.tabs.iter()).find(|t| t.id == id) else { return Task::none() };
        if !tab.grid {
            return Task::none();
        }
        let first_row = (tab.scroll_y / cell_h).floor() as usize;
        let rows = (tab.viewport_h / cell_h).ceil() as usize + 2;
        let start = first_row.saturating_sub(1) * cols;
        let end = ((first_row + rows) * cols).min(tab.view.len());
        let wanted: Vec<PathBuf> = (start..end)
            .filter_map(|i| tab.entry_at(i))
            .filter(|e| !e.is_dir && crate::thumbs::supported(&e.path) && !self.thumbs.contains_key(&e.path))
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

    /// Uma operação nova do usuário: limpa o refazer.
    fn start(&mut self, op: Op) -> Task<Message> {
        self.redo.clear();
        self.start_job(op, JobKind::Normal)
    }

    /// Copiar/mover: pergunta antes se algum nome já existe no destino.
    fn transfer(&mut self, op: Op) -> Task<Message> {
        let (Op::Copy { sources, dest, .. } | Op::Move { sources, dest, .. }) = &op else { return self.start(op) };
        let names = ops::conflicts(sources, dest);
        if names.is_empty() {
            return self.start(op);
        }
        self.overlay = Overlay::Conflict { op, names };
        Task::none()
    }

    fn start_job(&mut self, op: Op, kind: JobKind) -> Task<Message> {
        let id = self.next_job;
        self.next_job += 1;
        let queued = op.is_transfer() && self.jobs.iter().any(|j| j.op.is_transfer() && !j.queued);
        let (cancel, pause) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
        let label = op.label();
        self.jobs.push(Job { id, label, done: 0, total: 0, current: String::new(), queued, paused: false, cancel, pause, op, kind });
        if queued {
            return Task::none();
        }
        self.spawn_job(id)
    }

    fn spawn_job(&mut self, id: u64) -> Task<Message> {
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) else { return Task::none() };
        job.queued = false;
        let stream = ops::spawn(job.op.clone(), job.cancel.clone(), job.pause.clone());
        Task::run(stream, move |event| Message::Job { id, event })
    }

    /// Depois de uma transferência terminar, começa a próxima da fila.
    fn next_queued(&mut self) -> Task<Message> {
        if self.jobs.iter().any(|j| j.op.is_transfer() && !j.queued) {
            return Task::none();
        }
        match self.jobs.iter().find(|j| j.queued).map(|j| j.id) {
            Some(id) => self.spawn_job(id),
            None => Task::none(),
        }
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

    /// A pasta atual é o interior de um compactado: (compactado, caminho interno).
    fn in_archive(&self) -> Option<(PathBuf, PathBuf)> {
        archive::split(&self.tab().path)
    }

    fn run(&mut self, cmd: Cmd) -> Task<Message> {
        let pane = self.active;
        // Dentro de um compactado tudo é só leitura; copiar para o outro painel vira extrair.
        if let Some((file, inner)) = self.in_archive() {
            match cmd {
                Cmd::CopyToOther | Cmd::Extract => {
                    let members: Vec<PathBuf> =
                        self.tab().targets().iter().filter_map(|p| p.strip_prefix(&file).ok().map(Path::to_path_buf)).collect();
                    let members = if members.is_empty() && !inner.as_os_str().is_empty() { vec![inner] } else { members };
                    let dest = if self.dual {
                        self.panes[self.other_pane()].tab().path.clone()
                    } else {
                        file.parent().map(Path::to_path_buf).unwrap_or_default()
                    };
                    if archive::split(&dest).is_some() {
                        return self.notify("Escolha uma pasta comum como destino", true);
                    }
                    return self.start(Op::Extract { archive: file, members, dest });
                }
                Cmd::MoveToOther | Cmd::Cut | Cmd::Paste | Cmd::Rename | Cmd::NewFolder | Cmd::Trash | Cmd::Compress => {
                    return self.notify("Dentro de um compactado é só leitura: use Extrair (Ctrl+E)", true);
                }
                _ => {}
            }
        }
        // "Este Computador" é virtual: renomear, apagar ou colar ali agiria nas pastas e discos de verdade.
        let here = computer::is_virtual(&self.tab().path);
        let there = self.dual && computer::is_virtual(&self.panes[self.other_pane()].tab().path);
        if trashbin::is(&self.tab().path)
            && matches!(cmd, Cmd::Open | Cmd::OpenDefault | Cmd::OpenWith | Cmd::Copy | Cmd::CopyToOther | Cmd::Preview)
        {
            return self.notify("Restaure o item para abrir ou copiar", false);
        }
        let writes_here =
            matches!(cmd, Cmd::Paste | Cmd::Trash | Cmd::Rename | Cmd::NewFolder | Cmd::NewTextFile | Cmd::Cut | Cmd::MoveToOther);
        if (here && writes_here) || (there && matches!(cmd, Cmd::CopyToOther | Cmd::MoveToOther)) {
            return self.notify(format!("{} não aceita essa operação; abra uma pasta", computer::TITLE), true);
        }
        match cmd {
            Cmd::Extract => {
                let archives: Vec<PathBuf> = self.tab().targets().into_iter().filter(|p| archive::is_archive(p) && p.is_file()).collect();
                if archives.is_empty() {
                    return self.notify("Selecione um zip, 7z ou tar para extrair", false);
                }
                let dir = self.tab().path.clone();
                let tasks: Vec<_> = archives
                    .into_iter()
                    .map(|file| {
                        let stem = folder_name(&file);
                        let dest = ops::unique_dest(&dir, &stem);
                        self.start(Op::Extract { archive: file, members: vec![], dest })
                    })
                    .collect();
                Task::batch(tasks)
            }
            Cmd::Compress => {
                let sources = self.tab().targets();
                if sources.is_empty() {
                    return Task::none();
                }
                let base = if sources.len() == 1 { folder_name(&sources[0]) } else { self.tab().title() };
                let dest = ops::unique_dest(&self.tab().path, &format!("{base}.zip"));
                self.start(Op::Compress { sources, dest })
            }
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
            Cmd::OpenWith => match self.tab().current().filter(|e| !e.is_dir) {
                // No Windows o próprio sistema mostra a lista de apps; nos outros, digita-se o programa.
                Some(e) if cfg!(windows) => match launch::open_with_dialog(&e.path) {
                    Ok(()) => Task::none(),
                    Err(err) => self.notify(err, true),
                },
                Some(_) => self.open_prompt(PromptKind::OpenWith, String::new()),
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
                if computer::is_virtual(&path) {
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
            Cmd::Details => {
                self.details_on = !self.details_on;
                if !self.details_on {
                    self.details = None;
                }
                Task::none()
            }
            Cmd::Properties => {
                self.details_on = true;
                Task::none()
            }
            Cmd::ZoomIn => self.zoom(true),
            Cmd::ZoomOut => self.zoom(false),
            Cmd::Restore | Cmd::Purge | Cmd::EmptyTrash | Cmd::DeleteNow => self.trash_cmd(cmd),
            Cmd::Eject => {
                let Some(path) = self.menu_target.clone().or_else(|| self.tab().current().map(|e| e.path.clone())) else {
                    return Task::none();
                };
                if !self.drives.iter().any(|d| d.path == path && d.removable) {
                    return self.notify("Só dá para ejetar mídia removível", false);
                }
                Task::perform(
                    async move { tokio::task::spawn_blocking(move || computer::eject(&path)).await.unwrap_or_else(|e| Err(e.to_string())) },
                    Message::Ejected,
                )
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
            Cmd::ReopenTab => {
                let Some((pane, path, grid)) = self.closed.pop() else { return self.notify("Nenhuma aba fechada", false) };
                let task = self.new_tab(pane);
                self.panes[pane].tab_mut().grid = grid;
                let reload = self.navigate(pane, path);
                self.panes[pane].tab_mut().back.clear();
                Task::batch([task, reload])
            }
            Cmd::ToggleRestore => {
                let on = !self.restore;
                match session::set_restore_enabled(on) {
                    Ok(()) => {
                        self.restore = on;
                        self.saved = None;
                        self.persist();
                        self.notify(if on { "O arbor vai abrir com estas abas" } else { "O arbor vai abrir do zero" }, false)
                    }
                    Err(e) => self.notify(format!("Não salvou a preferência: {e}"), true),
                }
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
                let conflict = Conflict::KeepBoth;
                self.transfer(if cmd == Cmd::CopyToOther {
                    Op::Copy { sources, dest, conflict }
                } else {
                    Op::Move { sources, dest, conflict }
                })
            }
            Cmd::Copy | Cmd::Cut => {
                let items = self.tab().targets();
                let n = items.len();
                if n == 0 {
                    return Task::none();
                }
                // Também vai para o sistema; recortar fica marcado só aqui (vira mover ao colar no arbor).
                let shared = self.sysclip.set_files(&items).is_ok();
                self.clip = Some((cmd == Cmd::Cut, items));
                let verb = if cmd == Cmd::Cut { "recortado(s)" } else { "copiado(s)" };
                let only_here = if shared { "" } else { " (só no arbor: a área do sistema não respondeu)" };
                self.notify(format!("{n} item(ns) {verb}. {} cola.{only_here}", Cmd::Paste.shortcut()), !shared)
            }
            Cmd::Paste => {
                let system = self.sysclip.files();
                let Some((cut, sources)) = crate::sysclip::choose(self.clip.as_ref(), system) else { return Task::none() };
                let dest = self.tab().path.clone();
                let conflict = Conflict::KeepBoth;
                if cut {
                    self.clip = None;
                    self.transfer(Op::Move { sources, dest, conflict })
                } else {
                    self.transfer(Op::Copy { sources, dest, conflict })
                }
            }
            Cmd::Undo => match self.undo.pop() {
                Some((u, op)) => self.start_job(Op::Undo(u), JobKind::Undo { redo: op }),
                None => self.notify("Nada para desfazer", false),
            },
            Cmd::Redo => match self.redo.pop() {
                Some(op) => self.start_job(op, JobKind::Normal),
                None => self.notify("Nada para refazer", false),
            },
            Cmd::SelectAll => {
                let tab = self.tab_mut();
                tab.selected = tab.view.iter().map(|&i| tab.entries[i].path.clone()).collect();
                Task::none()
            }
            Cmd::Rename if self.tab().targets().len() > 1 => self.open_prompt(PromptKind::BatchRename, "{name}".into()),
            Cmd::Rename => match self.tab().current() {
                Some(e) => {
                    let value = e.name.clone();
                    self.open_prompt(PromptKind::Rename, value)
                }
                None => Task::none(),
            },
            Cmd::NewFolder => self.open_prompt(PromptKind::NewFolder, "Nova pasta".into()),
            Cmd::NewTextFile => self.open_prompt(PromptKind::NewFile, "Novo documento de texto.txt".into()),
            Cmd::NewWindow => {
                let arg = (!here).then(|| self.tab().path.clone());
                match launch::new_window(arg.as_deref()) {
                    Ok(()) => Task::none(),
                    Err(e) => self.notify(format!("Nova janela: {e}"), true),
                }
            }
            Cmd::InvertSelection => {
                let tab = self.tab_mut();
                let all: HashSet<PathBuf> = tab.view.iter().map(|&i| tab.entries[i].path.clone()).collect();
                tab.selected = all.difference(&tab.selected).cloned().collect();
                Task::none()
            }
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
            Cmd::Search => {
                let filter = self.tab().filter.trim().to_string();
                if filter.is_empty() {
                    self.open_prompt(PromptKind::Search, String::new())
                } else {
                    let show = self.show_hidden;
                    let tab = self.tab_mut();
                    tab.filter.clear();
                    tab.rebuild(show);
                    self.start_search(filter)
                }
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
            Cmd::ColKind | Cmd::ColSize | Cmd::ColModified => {
                self.columns.toggle(column_of(cmd).expect("comando de coluna"));
                self.save_columns()
            }
            Cmd::ToggleGrid => {
                let tab = self.tab_mut();
                tab.grid = !tab.grid;
                tab.scroll_y = 0.0;
                let id = tab.id;
                let scroll = op::scroll_to(tab.scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 });
                let saved = self.remember_view(id);
                let reveal = self.reveal_cursor(id);
                Task::batch([scroll, reveal, saved, self.request_thumbs(id)])
            }
            Cmd::SortName => self.sort_by(SortKey::Name),
            Cmd::SortSize => self.sort_by(SortKey::Size),
            Cmd::SortModified => self.sort_by(SortKey::Modified),
            Cmd::SortKind => self.sort_by(SortKey::Kind),
            Cmd::ToggleTheme => {
                self.dark = !self.dark;
                self.refresh_aurora();
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
            Cmd::WhoUses => {
                let targets = self.tab().targets();
                if targets.is_empty() {
                    return Task::none();
                }
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let lines: Vec<String> = targets
                                .iter()
                                .filter_map(|p| {
                                    let who = arbor_core::lockers::who_uses(p);
                                    (!who.is_empty()).then(|| format!("{}: {}", ops::name(p), arbor_core::lockers::describe(&who)))
                                })
                                .collect();
                            if lines.is_empty() {
                                "Nenhum programa está usando a seleção".to_string()
                            } else {
                                format!("Em uso por {}", lines.join(" · "))
                            }
                        })
                        .await
                        .unwrap_or_default()
                    },
                    Message::Info,
                )
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
            Cmd::EditKeys => match keymap::ensure_file().and_then(|f| launch::open_default(&f).map(|_| f)) {
                Ok(f) => self.notify(format!("Editando {}. Salve e use \"Recarregar atalhos\".", f.display()), false),
                Err(e) => self.notify(format!("Atalhos: {e}"), true),
            },
            Cmd::ReloadKeys => {
                let (n, bad) = keymap::reload();
                if bad.is_empty() {
                    self.notify(format!("{n} atalho(s) personalizado(s) ativo(s)"), false)
                } else {
                    self.notify(format!("{n} atalho(s) ativo(s); linhas ignoradas: {}", bad.join(" | ")), true)
                }
            }
            Cmd::Jump => {
                self.overlay = Overlay::Frecent { query: String::new(), selected: 0 };
                op::focus(JUMP_INPUT)
            }
            Cmd::SetDefault | Cmd::UnsetDefault => match instance::set_default(cmd == Cmd::SetDefault) {
                Ok(msg) => self.notify(msg, false),
                Err(e) => self.notify(e, true),
            },
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
        let saved = self.remember_view(id);
        Task::batch([saved, self.reveal_cursor(id)])
    }

    /// Grava ordenação e modo da pasta da aba para a próxima visita.
    fn remember_view(&mut self, id: u64) -> Task<Message> {
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        let (path, view) = (tab.path.clone(), crate::folder_views::View { sort: tab.sort, grid: tab.grid });
        if computer::is_virtual(&path) {
            return Task::none();
        }
        match self.views.set(&path, view) {
            Err(e) => self.notify(format!("Não salvou a visão da pasta: {e}"), true),
            Ok(()) => Task::none(),
        }
    }

    /// Abre uma aba de resultados no painel ativo, buscando `query` a partir da pasta atual.
    fn start_search(&mut self, query: String) -> Task<Message> {
        let pane = self.active;
        let root = self.tab().path.clone();
        let _ = self.new_tab(pane);
        let hidden = self.show_hidden;
        let tab = self.panes[pane].tab_mut();
        // Zera a aba nova (a listagem comum é descartada) e roda a busca no lugar dela, mostrando cada lote.
        let _ = Self::load_tab(tab, root.clone(), false);
        tab.search = Some((query, hidden));
        let task = Self::load_tab(tab, root, true);
        tab.refreshing = false;
        task
    }

    fn open_prompt(&mut self, kind: PromptKind, value: String) -> Task<Message> {
        self.suggest.items.clear();
        self.suggest.selected = None;
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
        let tab = p.tabs.remove(index);
        self.closed.push((pane, tab.path, tab.grid));
        if self.closed.len() > 30 {
            self.closed.remove(0);
        }
        let p = &mut self.panes[pane];
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

    /// Pastas do Ctrl+K: um caminho digitado que existe vem primeiro, depois as frequentes.
    pub fn jump_view(&self) -> Vec<PathBuf> {
        let Overlay::Frecent { query, .. } = &self.overlay else { return vec![] };
        let mut items = Vec::new();
        let typed = PathBuf::from(query.trim());
        if typed.is_absolute() && typed.is_dir() {
            items.push(typed);
        }
        for p in self.frecency.query(query, 12) {
            if !items.contains(&p) {
                items.push(p);
            }
        }
        items.truncate(12);
        items
    }

    pub fn palette_view(&self) -> Vec<Cmd> {
        match &self.overlay {
            Overlay::Palette { query, .. } => Self::palette_items(query),
            _ => vec![],
        }
    }

    /// Itens da renomeação em lote, na ordem em que aparecem na lista.
    fn batch_items(&self) -> Vec<rename::Item> {
        let tab = self.tab();
        let targets: HashSet<PathBuf> = tab.targets().into_iter().collect();
        tab.view
            .iter()
            .map(|&i| &tab.entries[i])
            .filter(|e| targets.contains(&e.path))
            .map(|e| rename::Item { path: e.path.clone(), modified: e.modified, is_dir: e.is_dir })
            .collect()
    }

    /// Prévia mostrada enquanto o modelo é digitado: quantos mudam e os primeiros pares (antes, depois).
    pub fn batch_preview(&self, pattern: &str) -> Result<(usize, Vec<(String, String)>), String> {
        let pairs = rename::plan(pattern, &self.batch_items())?;
        let first = pairs.iter().take(6).map(|(from, to)| (ops::name(from), ops::name(to))).collect();
        Ok((pairs.len(), first))
    }

    fn submit_prompt(&mut self) -> Task<Message> {
        let Overlay::Prompt { kind, value } = std::mem::replace(&mut self.overlay, Overlay::None) else { return Task::none() };
        let value = value.trim().to_string();
        if value.is_empty() {
            return Task::none();
        }
        let dir = self.tab().path.clone();
        match kind {
            PromptKind::Search => self.start_search(value),
            PromptKind::BatchRename => match rename::plan(&value, &self.batch_items()) {
                Ok(pairs) if pairs.is_empty() => self.notify("Nenhum nome muda com esse modelo", false),
                Ok(pairs) => self.start(Op::RenameMany { pairs }),
                Err(e) => {
                    self.overlay = Overlay::Prompt { kind, value };
                    self.notify(e, true)
                }
            },
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
            PromptKind::NewFile => {
                if value.contains(['/', '\\']) {
                    return self.notify("O nome não pode ter barras", true);
                }
                let path = ops::unique_dest(&dir, &value);
                self.tab_mut().focus_on_load = vec![path.clone()];
                Task::perform(
                    async move {
                        let p = path.clone();
                        tokio::task::spawn_blocking(move || std::fs::OpenOptions::new().write(true).create_new(true).open(&p).map(drop))
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|r| r.map_err(|e| e.to_string()))
                            .map(|()| path)
                    },
                    Message::Created,
                )
            }
            PromptKind::OpenWith => {
                let Some(file) = self.tab().current().map(|e| e.path.clone()) else { return Task::none() };
                match launch::open_with(&value, &file) {
                    Ok(()) => Task::none(),
                    Err(err) => self.notify(err, true),
                }
            }
            PromptKind::GoTo => {
                let value = self.chosen_suggestion().unwrap_or(value);
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
            Overlay::Frecent { .. } => {
                let n = self.jump_view().len();
                if let Overlay::Frecent { selected, .. } = &mut self.overlay {
                    match named {
                        Some(Named::Escape) => self.overlay = Overlay::None,
                        Some(Named::ArrowDown) if n > 0 => *selected = (*selected + 1) % n,
                        Some(Named::ArrowUp) if n > 0 => *selected = (*selected + n - 1) % n,
                        _ => {}
                    }
                }
                return Task::none();
            }
            Overlay::Conflict { .. } => {
                let choice = match (&key, named) {
                    (_, Some(Named::Escape)) => Some(None),
                    (_, Some(Named::Enter)) => Some(Some(Conflict::KeepBoth)),
                    (Key::Character(c), _) => match c.to_lowercase().as_str() {
                        "s" => Some(Some(Conflict::Replace)),
                        "p" => Some(Some(Conflict::Skip)),
                        "m" => Some(Some(Conflict::KeepBoth)),
                        _ => None,
                    },
                    _ => None,
                };
                return choice.map_or_else(Task::none, |c| self.update(Message::ResolveConflict(c)));
            }
            Overlay::Prompt { .. } => return self.prompt_key(named),
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
            Overlay::Jump(_) => return self.jump_key(named),
            Overlay::Confirm { .. } => {
                return match named {
                    Some(Named::Enter) => self.confirmed(),
                    Some(Named::Escape) => {
                        self.overlay = Overlay::None;
                        Task::none()
                    }
                    _ => Task::none(),
                };
            }
            Overlay::None => {}
        }

        // Atalhos do usuário valem antes dos de fábrica. Num campo de texto, só os com Ctrl/Alt ou teclas F.
        let is_f = matches!(&key, Key::Named(n) if format!("{n:?}").strip_prefix('F').is_some_and(|d| d.parse::<u8>().is_ok()));
        if !captured || mods.command() || mods.alt() || is_f {
            if let Some(cmd) = keymap::lookup(&key, mods) {
                return self.run(cmd);
            }
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
            Some(Named::F4) => return self.run(Cmd::GoTo),
            Some(Named::F8) => return self.run(Cmd::Trash),
            Some(Named::F12) => return self.run(Cmd::ToggleFps),
            _ => {}
        }
        if cmd_mod {
            if let Key::Character(c) = &key {
                let c = c.to_lowercase();
                let cmd = match (c.as_str(), mods.shift()) {
                    ("p", true) => Some(Cmd::Palette),
                    ("k", false) => Some(Cmd::Jump),
                    ("=" | "+", _) => Some(Cmd::ZoomIn),
                    ("-", _) => Some(Cmd::ZoomOut),
                    ("c", true) => Some(Cmd::CopyPath),
                    ("n", true) => Some(Cmd::NewFolder),
                    ("n", false) => Some(Cmd::NewWindow),
                    ("t", false) => Some(Cmd::NewTab),
                    ("e", false) => Some(Cmd::Extract),
                    ("e", true) => Some(Cmd::Compress),
                    ("t", true) => Some(Cmd::ReopenTab),
                    ("w", false) => Some(Cmd::CloseTab),
                    ("r", false) => Some(Cmd::Refresh),
                    ("l", false) => Some(Cmd::GoTo),
                    ("f", false) => Some(Cmd::Filter),
                    ("f", true) => Some(Cmd::Search),
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
                    ("z", true) | ("y", false) if !captured => Some(Cmd::Redo),
                    ("a", false) if !captured => Some(Cmd::SelectAll),
                    _ => None,
                };
                return cmd.map_or_else(Task::none, |c| self.run(c));
            }
        }
        if mods.alt() && mods.shift() && matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("p")) {
            return self.run(Cmd::Details);
        }
        // Alt+D: barra de endereço, como no Explorer.
        if mods.alt() && !mods.shift() && matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("d")) {
            return self.run(Cmd::GoTo);
        }
        if captured {
            return Task::none();
        }
        match named {
            Some(Named::Enter) if mods.alt() => return self.run(Cmd::Properties),
            Some(Named::Enter) => return self.run(if mods.shift() { Cmd::OpenDefault } else { Cmd::Open }),
            Some(Named::Space) => return self.run(Cmd::Preview),
            Some(Named::Backspace) => return self.go_up(),
            Some(Named::Delete) if mods.shift() => return self.run(Cmd::DeleteNow),
            Some(Named::Delete) if trashbin::is(&self.tab().path) => return self.run(Cmd::Purge),
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
        // Só mensagens que podem mudar abas ou painéis gravam a sessão; quadros, rolagem e progresso não.
        let structural = matches!(
            message,
            Message::Run(_)
                | Message::Key { .. }
                | Message::Navigate(_)
                | Message::NavigateIn { .. }
                | Message::Activate { .. }
                | Message::SelectTab { .. }
                | Message::CloseTabAt { .. }
                | Message::NewTabIn(_)
                | Message::PromptSubmit
                | Message::PaletteRun(_)
        );
        let task = self.handle(message);
        if structural {
            self.persist();
        }
        Task::batch([task, self.refresh_details()])
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Listed { tab, generation, snap } => self.on_listed(tab, generation, snap),
            Message::Glass(on) => {
                self.glass = on;
                self.refresh_aurora();
                Task::none()
            }
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
                if computer::is_virtual(&dest) || path.parent() == Some(dest.as_path()) {
                    return Task::none();
                }
                self.transfer(Op::Copy { sources: vec![path], dest, conflict: Conflict::KeepBoth })
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
            Message::JumpOpen { kind, dir } => self.open_jump(kind, dir),
            Message::JumpListed { generation, dirs } => self.jump_listed(generation, dirs),
            Message::JumpPick(i) => self.jump_pick(i),
            Message::JumpHover(i) => {
                if let Overlay::Jump(j) = &mut self.overlay {
                    j.selected = Some(i);
                }
                Task::none()
            }
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
                // Ctrl+Enter no filtro busca o mesmo texto nas subpastas.
                let task = self.run(if self.modifiers.command() { Cmd::Search } else { Cmd::Open });
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
            Message::JumpQuery(q) => {
                if let Overlay::Frecent { query, selected } = &mut self.overlay {
                    *query = q;
                    *selected = 0;
                }
                Task::none()
            }
            Message::JumpRun(index) => {
                let Overlay::Frecent { selected, .. } = &self.overlay else { return Task::none() };
                let i = if index == usize::MAX { *selected } else { index };
                let target = self.jump_view().get(i).cloned();
                self.overlay = Overlay::None;
                match target {
                    Some(dir) => {
                        let pane = self.active;
                        self.navigate(pane, dir)
                    }
                    None => Task::none(),
                }
            }
            Message::PromptChanged(v) => {
                let goto = matches!(self.overlay, Overlay::Prompt { kind: PromptKind::GoTo, .. });
                if let Overlay::Prompt { value, .. } = &mut self.overlay {
                    *value = v.clone();
                }
                if goto {
                    self.request_suggestions(v)
                } else {
                    Task::none()
                }
            }
            Message::Suggested { generation, items } => self.on_suggested(generation, items),
            Message::SuggestPick(i) => self.accept_suggestion(Some(i)),
            Message::Zoom(larger) => self.zoom(larger),
            Message::TreeToggle(path) => self.tree_toggle(path),
            Message::TrashListed(result) => self.on_trash_listed(result),
            Message::TrashDone(result) => self.on_trash_done(result),
            Message::Confirmed => self.confirmed(),
            Message::Ejected(result) => {
                let note = match result {
                    Ok(()) => self.notify("Pode remover a mídia com segurança", false),
                    Err(e) => self.notify(format!("Não ejetou: {e}"), true),
                };
                Task::batch([note, scan_drives()])
            }
            Message::TreeLoaded { path, dirs } => {
                self.tree.children.insert(path, dirs);
                Task::none()
            }
            Message::Created(result) => match result {
                Ok(_) => Task::none(),
                Err(e) => self.notify(format!("Não criou o arquivo: {e}"), true),
            },
            Message::PressItem { tab, index } => self.press_item(tab, index),
            Message::PressEmpty(tab) => self.press_empty(tab),
            Message::BandMove { tab, at } => self.band_move(tab, at),
            Message::PointerMoved(at) => self.pointer_moved(at),
            Message::PointerUp => self.pointer_up(),
            Message::ColResizeStart(col) => self.resize_start(col),
            Message::HeaderMenu => self.open_menu(MenuKind::Columns, None),
            Message::DragOver(target) => self.drag_over(target),
            Message::DragLeave { from, back } => self.drag_leave(from, back),
            Message::DetailsLoaded { path, preview, props } => self.on_details(path, preview, props),
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
                    let Some(pos) = self.jobs.iter().position(|j| j.id == id) else { return Task::none() };
                    let job = self.jobs.remove(pos);
                    let refresh = self.refresh_all();
                    let next = self.next_queued();
                    let toast = match result {
                        Ok(outcome) => {
                            match (outcome.undo, job.kind) {
                                (Some(u), JobKind::Normal) => {
                                    self.undo.push((u, job.op));
                                    if self.undo.len() > 100 {
                                        self.undo.remove(0);
                                    }
                                }
                                (_, JobKind::Undo { redo }) => self.redo.push(redo),
                                _ => {}
                            }
                            let tab = self.tab_mut();
                            tab.focus_on_load = outcome.created.into_iter().filter(|p| p.parent() == Some(tab.path.as_path())).collect();
                            self.notify(outcome.summary, false)
                        }
                        Err(e) => self.notify(format!("Erro: {e}"), true),
                    };
                    Task::batch([refresh, next, toast])
                }
            },
            Message::Info(text) => self.notify(text, false),
            Message::Opened { name, result } => match result {
                Ok(()) => Task::none(),
                Err(e) => self.notify(format!("Não abriu {name}: {e}"), true),
            },
            Message::CancelJob(id) => {
                // Na fila, ainda não começou: basta tirar.
                if self.jobs.iter().any(|j| j.id == id && j.queued) {
                    self.jobs.retain(|j| j.id != id);
                } else if let Some(j) = self.jobs.iter().find(|j| j.id == id) {
                    j.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                Task::none()
            }
            Message::PauseJob(id) => {
                if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id && !j.queued) {
                    j.paused = !j.paused;
                    j.pause.store(j.paused, std::sync::atomic::Ordering::Relaxed);
                }
                Task::none()
            }
            Message::ResolveConflict(choice) => {
                let Overlay::Conflict { op, .. } = std::mem::replace(&mut self.overlay, Overlay::None) else { return Task::none() };
                let Some(conflict) = choice else { return Task::none() };
                let op = match op {
                    Op::Copy { sources, dest, .. } => Op::Copy { sources, dest, conflict },
                    Op::Move { sources, dest, .. } => Op::Move { sources, dest, conflict },
                    other => other,
                };
                self.start(op)
            }
            Message::ToastExpired(id) => {
                if self.toast.as_ref().is_some_and(|t| t.id == id) {
                    self.toast = None;
                }
                Task::none()
            }
            Message::DirChanged(path) => {
                let tasks: Vec<_> = (0..2)
                    .filter(|&p| {
                        let t = self.panes[p].tab();
                        t.path == path && !t.loading && t.search.is_none()
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|p| self.load(p, path.clone(), true))
                    .collect();
                Task::batch(tasks)
            }
            Message::Remote(target) => {
                let pane = self.active;
                let tab = self.new_tab(pane);
                let go = match target.and_then(start_dir_keep) {
                    Some((dir, file)) => {
                        let task = self.navigate(pane, dir);
                        let t = self.tab_mut();
                        t.back.clear();
                        t.focus_on_load = file.into_iter().collect();
                        task
                    }
                    None => Task::none(),
                };
                let focus = window::latest().and_then(window::gain_focus);
                Task::batch([tab, go, focus])
            }
            Message::DrivesTick => {
                Task::perform(async { tokio::task::spawn_blocking(computer::drives).await.unwrap_or_default() }, Message::DrivesPolled)
            }
            Message::DrivesPolled(drives) => {
                let ids = |v: &[Drive]| v.iter().map(|d| (d.label.clone(), d.path.clone())).collect::<Vec<_>>();
                if ids(&drives) == ids(&self.drives) {
                    return Task::none();
                }
                self.handle(Message::Places(drives))
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

fn frecency_file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor").join("frecency"))
}

fn load_frecency() -> Frecency {
    frecency_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| Frecency::from_text(&t)).unwrap_or_default()
}

/// Grava numa thread à parte (arquivo pequeno, troca atômica).
fn save_frecency(f: &Frecency) {
    let Some(file) = frecency_file() else { return };
    let text = f.to_text();
    std::thread::spawn(move || {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = file.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &file);
        }
    });
}

/// Como [`start_dir`], mas lembrando o arquivo pedido para selecioná-lo na pasta.
fn start_dir_keep(p: PathBuf) -> Option<(PathBuf, Option<PathBuf>)> {
    if p.is_dir() {
        Some((p, None))
    } else {
        p.parent().map(|d| (d.to_path_buf(), Some(p.clone())))
    }
}

/// Nome do arquivo sem as extensões de compactado ("fotos.tar.gz" → "fotos").
fn folder_name(p: &Path) -> String {
    let name = ops::name(p);
    let lower = name.to_lowercase();
    for ext in [".tar.gz", ".tar.xz", ".tgz", ".txz", ".tar", ".zip", ".7z"] {
        if lower.ends_with(ext) && name.len() > ext.len() {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(name)
}

/// `arbor arquivo.txt` abre a pasta do arquivo em vez de tentar listar um arquivo.
fn start_dir(p: PathBuf) -> Option<PathBuf> {
    if p.is_dir() {
        Some(p)
    } else {
        p.parent().map(Path::to_path_buf)
    }
}

fn scan_trash() -> Task<Message> {
    Task::perform(async { tokio::task::spawn_blocking(trashbin::list).await.unwrap_or_else(|e| Err(e.to_string())) }, Message::TrashListed)
}

/// Pendrive entrou ou saiu: o Explorer percebe em segundos, sem F5.
const DRIVES_EVERY: Duration = Duration::from_secs(4);

/// Coluna que um comando "Coluna …" mostra ou oculta.
pub fn column_of(cmd: Cmd) -> Option<crate::columns::Col> {
    use crate::columns::Col;
    match cmd {
        Cmd::ColKind => Some(Col::Kind),
        Cmd::ColSize => Some(Col::Size),
        Cmd::ColModified => Some(Col::Modified),
        _ => None,
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
    fn nome_da_pasta_de_extracao() {
        assert_eq!(folder_name(Path::new("/x/fotos.tar.gz")), "fotos");
        assert_eq!(folder_name(Path::new("/x/Pacote.ZIP")), "Pacote");
        assert_eq!(folder_name(Path::new("/x/notas.txt")), "notas");
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
