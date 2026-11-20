//! Tokens visuais do protótipo "arbor Glass": cores, fontes, raios e os estilos de vidro.
//!
//! O protótipo HTML é a especificação; aqui ficam os mesmos valores para o iced. A aurora vem de
//! `aurora.rs` e o vidro nativo do sistema (quando há) de `glass.rs`.

use iced::widget::{button, container, svg, text};
use iced::{border, gradient, Background, Border, Color, Font, Radians, Shadow, Theme, Vector};

// ---------- fontes (OFL, embutidas em assets/fonts) ----------

pub const FONTS: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/Manrope-Medium.ttf"),
    include_bytes!("../assets/fonts/Manrope-Bold.ttf"),
    include_bytes!("../assets/fonts/Sora-SemiBold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
];

pub const UI: Font = Font { weight: iced::font::Weight::Medium, ..Font::with_name("Manrope") };
pub const UI_BOLD: Font = Font { weight: iced::font::Weight::Bold, ..Font::with_name("Manrope") };
pub const DISPLAY: Font = Font { weight: iced::font::Weight::Semibold, ..Font::with_name("Sora") };
pub const MONO: Font = Font::with_name("JetBrains Mono");

// ---------- cores ----------

const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const fn rgba(hex: u32, a: f32) -> Color {
    let c = rgb(hex);
    Color { a, ..c }
}

pub const ICE: Color = rgb(0x8fe6ff);
pub const IRIS: Color = rgb(0xb9a2ff);
pub const ROSE: Color = rgb(0xff94c9);
pub const AMBER: Color = rgb(0xffd08a);
pub const MINT: Color = rgb(0x8ff0c0);
pub const CORAL: Color = rgb(0xffa184);
pub const STEEL: Color = rgb(0xc8d0e6);

/// Paleta de um tema (escuro é o padrão; o claro usa os mesmos papéis).
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub void: Color,
    /// Faixa das abas, do endereço e dos comandos: a aba ativa tem esta cor e "se funde" com ela.
    pub surface: Color,
    pub ink: Color,
    pub ink2: Color,
    pub ink3: Color,
    pub stroke: Color,
    pub stroke_hi: Color,
    pub glass: Color,
    pub glass_hi: Color,
    pub accent: Color,
    pub accent2: Color,
    pub danger: Color,
    pub good: Color,
    pub warn: Color,
}

pub const DARK: Tokens = Tokens {
    void: rgb(0x060914),
    surface: rgba(0x222848, 0.86),
    ink: rgb(0xeef1ff),
    ink2: rgba(0xdee4ff, 0.68),
    ink3: rgba(0xdee4ff, 0.42),
    stroke: rgba(0xffffff, 0.12),
    stroke_hi: rgba(0xffffff, 0.26),
    glass: rgba(0xffffff, 0.10),
    glass_hi: rgba(0xffffff, 0.07),
    accent: ICE,
    accent2: IRIS,
    danger: rgb(0xff8a8a),
    good: MINT,
    warn: AMBER,
};

pub const LIGHT: Tokens = Tokens {
    void: rgb(0xf3f5fc),
    surface: rgba(0xffffff, 0.86),
    ink: rgb(0x111528),
    ink2: rgba(0x1d2340, 0.72),
    ink3: rgba(0x1d2340, 0.5),
    stroke: rgba(0x1d2340, 0.12),
    stroke_hi: rgba(0x1d2340, 0.24),
    glass: rgba(0xffffff, 0.58),
    glass_hi: rgba(0x1d2340, 0.05),
    accent: rgb(0x0a8fc2),
    accent2: rgb(0x6c4fe0),
    danger: rgb(0xc43d3d),
    good: rgb(0x1f9d63),
    warn: rgb(0xb7790e),
};

/// Tokens do tema ativo (`palettes::current`); o `Theme` do iced só carrega a paleta básica.
pub fn tokens(_: &Theme) -> Tokens {
    now()
}

/// Os mesmos tokens fora de um estilo, para a cor dos ícones (que antes vinha fixa do tema escuro).
pub fn now() -> Tokens {
    crate::palettes::current().tokens()
}

pub fn theme(p: crate::palettes::Palette) -> Theme {
    let k = p.tokens();
    Theme::custom(
        format!("arbor {}", p.name()),
        iced::theme::Palette { background: k.void, text: k.ink, primary: k.accent, success: k.good, warning: k.warn, danger: k.danger },
    )
}

pub fn alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

// ---------- raios ----------

pub const R_ROW: f32 = 10.0;
pub const R_CHIP: f32 = 8.0;

// ---------- estilos ----------

/// Painel de vidro, como no protótipo: `linear-gradient(140deg, branco 10%, 2,5% aos 55%, 5%)` sobre a
/// aurora. Encosta nas bordas da janela, então não tem canto arredondado nem sombra; só o filete.
pub fn glass(t: &Theme, active: bool) -> container::Style {
    let k = tokens(t);
    let g = gradient::Linear::new(Radians(2.44))
        .add_stop(0.0, k.glass)
        .add_stop(0.55, alpha(k.glass, k.glass.a * 0.25))
        .add_stop(1.0, alpha(k.glass, k.glass.a * 0.5));
    container::Style {
        background: Some(Background::Gradient(g.into())),
        text_color: Some(k.ink),
        border: Border {
            color: if active { alpha(k.accent, 0.36) } else { alpha(k.stroke, 0.6) },
            width: if active { 1.0 } else { 0.0 },
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

/// Faixa de cima (endereço e comandos) na cor da aba ativa.
pub fn surface(t: &Theme) -> container::Style {
    let k = tokens(t);
    container::Style { background: Some(k.surface.into()), text_color: Some(k.ink), ..Default::default() }
}

/// Barra de título própria (janela sem moldura do sistema): mais escura que a faixa das abas.
pub fn titlebar(t: &Theme) -> container::Style {
    let k = tokens(t);
    container::Style { background: Some(alpha(k.void, 0.55).into()), text_color: Some(k.ink), ..Default::default() }
}

/// Aba no estilo do Explorer do Windows 11: cantos de cima arredondados e, ativa, a cor da faixa abaixo.
pub fn win_tab(t: &Theme, status: button::Status, selected: bool) -> button::Style {
    let k = tokens(t);
    let background = if selected {
        Some(k.surface.into())
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        Some(k.glass_hi.into())
    } else {
        None
    };
    button::Style {
        background,
        text_color: if selected || status == button::Status::Hovered { k.ink } else { k.ink2 },
        border: Border {
            radius: border::Radius { top_left: 9.0, top_right: 9.0, bottom_right: 0.0, bottom_left: 0.0 },
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Aba vertical: a pílula da barra lateral, com a cor de destaque na ativa.
pub fn side_tab(t: &Theme, status: button::Status, selected: bool) -> button::Style {
    place(t, status, selected)
}

/// Minimizar, maximizar e fechar; fechar fica vermelho no hover, como no Windows.
pub fn window_button(t: &Theme, status: button::Status, close: bool) -> button::Style {
    let k = tokens(t);
    let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let background = match (hot, close) {
        (true, true) => Some(rgb(0xc42b1c).into()),
        (true, false) => Some(k.glass_hi.into()),
        _ => None,
    };
    button::Style { background, text_color: if hot && close { Color::WHITE } else { k.ink2 }, ..Default::default() }
}

/// Contorno da janela sem moldura: mais claro no escuro, mais escuro no claro, como no Windows 11.
pub fn window_border(t: &Theme) -> Border {
    let k = tokens(t);
    let color = if t.extended_palette().is_dark { alpha(k.ink, 0.16) } else { Color::from_rgba(0.0, 0.0, 0.0, 0.22) };
    Border { color, width: 1.0, radius: 0.0.into() }
}

/// Sobreposição (paleta, preview, diálogo): vidro mais opaco e azulado.
pub fn sheet(t: &Theme) -> container::Style {
    let k = tokens(t);
    let dark = t.extended_palette().is_dark;
    // o protótipo usa 60%/55% com desfoque; sem desfoque o conteúdo por trás atrapalharia a leitura
    let (a, b) = if dark { (rgba(0x283056, 0.86), rgba(0x12162c, 0.84)) } else { (rgba(0xffffff, 0.9), rgba(0xf1f3fb, 0.88)) };
    container::Style {
        background: Some(Background::Gradient(gradient::Linear::new(Radians(2.2)).add_stop(0.0, a).add_stop(1.0, b).into())),
        text_color: Some(k.ink),
        border: Border { color: k.stroke_hi, width: 1.0, radius: 20.0.into() },
        shadow: Shadow { color: Color::from_rgba(0.0, 0.0, 0.0, 0.55), offset: Vector::new(0.0, 24.0), blur_radius: 60.0 },
        snap: false,
    }
}

/// Véu atrás dos diálogos; `t` (0 → 1) é a entrada animada.
pub fn scrim(_: &Theme, t: f32) -> container::Style {
    container::Style { background: Some(Color::from_rgba(0.02, 0.03, 0.07, 0.5 * t).into()), ..Default::default() }
}

/// Linha da lista: a pílula de seleção do protótipo (gradiente gelo → íris) e um contorno para o cursor.
pub fn row(t: &Theme, selected: bool, cursor: bool, pane_active: bool) -> container::Style {
    let k = tokens(t);
    let strength = if pane_active { 1.0 } else { 0.45 };
    let background = if selected {
        let g = gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2))
            .add_stop(0.0, alpha(k.accent, 0.24 * strength))
            .add_stop(1.0, alpha(k.accent2, 0.12 * strength));
        Some(Background::Gradient(g.into()))
    } else if cursor {
        Some(k.glass_hi.into())
    } else {
        None
    };
    let edge = if cursor && pane_active {
        alpha(k.accent, 0.5)
    } else if selected {
        alpha(k.accent, 0.25 * strength)
    } else {
        Color::TRANSPARENT
    };
    container::Style {
        background,
        border: Border { color: edge, width: 1.0, radius: R_ROW.into() },
        shadow: if selected && pane_active {
            Shadow { color: alpha(k.accent, 0.22), offset: Vector::ZERO, blur_radius: 18.0 }
        } else {
            Shadow::default()
        },
        ..Default::default()
    }
}

/// Item da barra lateral e da paleta.
pub fn place(t: &Theme, status: button::Status, active: bool) -> button::Style {
    let k = tokens(t);
    let background = if active {
        let g = gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2))
            .add_stop(0.0, alpha(k.accent, 0.2))
            .add_stop(1.0, alpha(k.accent, 0.02));
        Some(Background::Gradient(g.into()))
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        Some(k.glass_hi.into())
    } else {
        None
    };
    button::Style {
        background,
        text_color: if active || status == button::Status::Hovered { k.ink } else { k.ink2 },
        border: Border { color: if active { alpha(k.accent, 0.25) } else { Color::TRANSPARENT }, width: 1.0, radius: R_ROW.into() },
        ..Default::default()
    }
}

pub fn tab(t: &Theme, status: button::Status, selected: bool, pane_active: bool) -> button::Style {
    let k = tokens(t);
    let background = if selected {
        Some(alpha(k.ink, 0.09).into())
    } else if status == button::Status::Hovered {
        Some(k.glass_hi.into())
    } else {
        None
    };
    let underline = if selected { alpha(k.accent, if pane_active { 0.7 } else { 0.25 }) } else { Color::TRANSPARENT };
    button::Style {
        background,
        text_color: if selected { k.ink } else { k.ink3 },
        border: Border { color: underline, width: if selected { 1.0 } else { 0.0 }, radius: 11.0.into() },
        ..Default::default()
    }
}

pub fn ghost(t: &Theme, status: button::Status) -> button::Style {
    let k = tokens(t);
    let background = matches!(status, button::Status::Hovered | button::Status::Pressed).then_some(Background::from(k.glass_hi));
    button::Style {
        background,
        text_color: if status == button::Status::Hovered { k.ink } else { k.ink2 },
        border: border::rounded(8),
        ..Default::default()
    }
}

/// Botão de destaque (atualizar agora).
pub fn primary(t: &Theme, status: button::Status) -> button::Style {
    let k = tokens(t);
    let lift = if status == button::Status::Hovered { 0.1 } else { 0.0 };
    let g = gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2))
        .add_stop(0.0, alpha(k.accent, 0.85 + lift))
        .add_stop(1.0, alpha(k.accent2, 0.85 + lift));
    button::Style {
        background: Some(Background::Gradient(g.into())),
        text_color: rgb(0x060914),
        border: border::rounded(9),
        shadow: Shadow { color: alpha(k.accent, 0.35), offset: Vector::ZERO, blur_radius: 16.0 },
        ..Default::default()
    }
}

pub fn field(t: &Theme, focused: bool) -> container::Style {
    let k = tokens(t);
    container::Style {
        background: Some(if focused { alpha(k.accent, 0.08) } else { k.glass_hi }.into()),
        border: Border { color: if focused { alpha(k.accent, 0.5) } else { k.stroke }, width: 1.0, radius: 9.0.into() },
        ..Default::default()
    }
}

pub fn input(t: &Theme, status: iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    use iced::widget::text_input::Status;
    let k = tokens(t);
    let focused = matches!(status, Status::Focused { .. });
    iced::widget::text_input::Style {
        background: Color::TRANSPARENT.into(),
        border: Border { color: if focused { alpha(k.accent, 0.5) } else { k.stroke }, width: 1.0, radius: 9.0.into() },
        icon: k.ink3,
        placeholder: k.ink3,
        value: k.ink,
        selection: alpha(k.accent, 0.35),
    }
}

/// Tecla desenhada (`kbd`), usada na doca de atalhos e nas dicas.
pub fn kbd(t: &Theme) -> container::Style {
    let k = tokens(t);
    container::Style {
        background: Some(alpha(k.ink, 0.07).into()),
        text_color: Some(k.ink2),
        border: Border { color: k.stroke, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    }
}

/// Selo "N itens em X ms": verde quando rápido, âmbar quando passou de 150 ms.
pub fn latency(t: &Theme, slow: bool) -> container::Style {
    let k = tokens(t);
    let c = if slow { k.warn } else { k.good };
    container::Style {
        background: Some(alpha(c, 0.09).into()),
        text_color: Some(c),
        border: Border { color: alpha(c, 0.28), width: 1.0, radius: 7.0.into() },
        ..Default::default()
    }
}

/// Ficha de ícone colorida pelo tipo de arquivo.
/// Ficha com a cor atenuada por `strength` (item oculto fica mais apagado).
pub fn chip_soft(color: Color, strength: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(alpha(color, 0.15 * strength).into()),
        border: Border { color: alpha(color, 0.24 * strength), width: 1.0, radius: R_CHIP.into() },
        ..Default::default()
    }
}

/// Pílula de informação na barra de comandos (git, ocultos): vidro discreto tingido com `color`.
pub fn pill(color: Color, strong: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t, status| {
        let k = tokens(t);
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let a = if strong { 0.16 } else { 0.07 } + if hot { 0.06 } else { 0.0 };
        button::Style {
            background: Some(alpha(color, a).into()),
            text_color: k.ink2,
            border: Border { color: alpha(color, if strong { 0.4 } else { 0.22 }), width: 1.0, radius: 7.0.into() },
            ..Default::default()
        }
    }
}

/// Opacidade de itens ocultos: ainda legíveis, mas recuados em relação aos demais.
pub const HIDDEN_ALPHA: f32 = 0.5;

pub fn tooltip(t: &Theme) -> container::Style {
    let k = tokens(t);
    container::Style {
        background: Some(if t.extended_palette().is_dark { rgba(0x1e2444, 0.98) } else { rgba(0xffffff, 0.98) }.into()),
        text_color: Some(k.ink),
        border: Border { color: k.stroke_hi, width: 1.0, radius: 8.0.into() },
        shadow: Shadow { color: Color::from_rgba(0.0, 0.0, 0.0, 0.3), offset: Vector::new(0.0, 6.0), blur_radius: 18.0 },
        ..Default::default()
    }
}

pub fn progress(t: &Theme) -> iced::widget::progress_bar::Style {
    let k = tokens(t);
    let g = gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2)).add_stop(0.0, k.accent).add_stop(1.0, k.accent2);
    iced::widget::progress_bar::Style {
        background: alpha(k.ink, 0.08).into(),
        bar: Background::Gradient(g.into()),
        border: border::rounded(4),
    }
}

/// Retângulo de seleção: preenchimento leve e borda na cor de destaque.
pub fn band(t: &Theme) -> container::Style {
    let k = tokens(t);
    container::Style {
        background: Some(alpha(k.accent, 0.12).into()),
        border: Border { color: alpha(k.accent, 0.6), width: 1.0, radius: 3.0.into() },
        ..Default::default()
    }
}

/// Área de soltar (fixar na barra lateral): acende quando o arrasto passa por cima.
pub fn drop_zone(t: &Theme, hot: bool) -> container::Style {
    let k = tokens(t);
    container::Style {
        background: Some(alpha(k.accent, if hot { 0.2 } else { 0.05 }).into()),
        border: Border { color: alpha(k.accent, if hot { 0.7 } else { 0.3 }), width: 1.0, radius: R_ROW.into() },
        text_color: Some(if hot { k.ink } else { k.ink2 }),
        ..Default::default()
    }
}

/// Disco quase cheio: mesma barra, em vermelho.
pub fn progress_full(t: &Theme) -> iced::widget::progress_bar::Style {
    iced::widget::progress_bar::Style { bar: Background::Color(tokens(t).danger), ..progress(t) }
}

pub fn scrollbar(t: &Theme, status: iced::widget::scrollable::Status) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{Rail, Scroller, Status};
    let k = tokens(t);
    let hot = matches!(status, Status::Hovered { .. } | Status::Dragged { .. });
    let rail = Rail {
        background: None,
        border: border::rounded(4),
        scroller: Scroller { background: alpha(k.ink, if hot { 0.32 } else { 0.16 }).into(), border: border::rounded(4) },
    };
    let mut s = iced::widget::scrollable::default(t, status);
    s.container = container::Style::default();
    s.vertical_rail = rail;
    s.horizontal_rail = rail;
    s
}

// ---------- texto ----------

pub fn ink2(t: &Theme) -> text::Style {
    text::Style { color: Some(tokens(t).ink2) }
}

pub fn ink3(t: &Theme) -> text::Style {
    text::Style { color: Some(tokens(t).ink3) }
}

pub fn danger(t: &Theme) -> text::Style {
    text::Style { color: Some(tokens(t).danger) }
}

pub fn accent(t: &Theme) -> text::Style {
    text::Style { color: Some(tokens(t).accent) }
}

// ---------- ícones (traços do protótipo, 24×24) ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Folder,
    Image,
    Code,
    Doc,
    Audio,
    Video,
    Archive,
    File,
    Home,
    Search,
    Bolt,
    Left,
    Right,
    Up,
    Down,
    Plus,
    List,
    Grid,
    Drive,
    DriveSystem,
    Usb,
    Disc,
    Network,
    Download,
    Cut,
    Copy,
    Paste,
    Rename,
    Trash,
    Sort,
    Eye,
    More,
    Pin,
    Palette,
    FolderGit,
    Branch,
    WinMin,
    WinMax,
    WinClose,
    Logo,
}

impl Icon {
    fn path(self) -> &'static str {
        match self {
            Icon::Folder => {
                r#"<path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H9l2 2h7.5A2.5 2.5 0 0 1 21 9.5v8a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 17.5z" fill="currentColor" fill-opacity=".2"/>"#
            }
            Icon::Image => r#"<rect x="3" y="4" width="18" height="16" rx="3"/><circle cx="9" cy="10" r="2"/><path d="m21 16-5-5-9 9"/>"#,
            Icon::Code => r#"<path d="m8 8-4 4 4 4M16 8l4 4-4 4M13.5 5.5l-3 13"/>"#,
            Icon::Doc => r#"<path d="M6 3h8l4 4v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z"/><path d="M14 3v4h4M9 12h6M9 16h4"/>"#,
            Icon::Audio => r#"<path d="M9 18V5l11-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="17" cy="16" r="3"/>"#,
            Icon::Video => r#"<rect x="3" y="5" width="13" height="14" rx="3"/><path d="m16 10 5-3v10l-5-3"/>"#,
            Icon::Archive => {
                r#"<rect x="3" y="4" width="18" height="5" rx="1.5"/><path d="M5 9v9a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9M10 13h4"/>"#
            }
            Icon::File => r#"<path d="M6 3h8l4 4v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z"/><path d="M14 3v4h4"/>"#,
            Icon::Home => r#"<path d="m3 11 9-7 9 7v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z"/>"#,
            Icon::Search => r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>"#,
            Icon::Bolt => r#"<path d="M13 2 4 14h7l-1 8 9-12h-7z"/>"#,
            Icon::Left => r#"<path d="m15 6-6 6 6 6"/>"#,
            Icon::Right => r#"<path d="m9 6 6 6-6 6"/>"#,
            Icon::Up => r#"<path d="m6 15 6-6 6 6"/>"#,
            Icon::Down => r#"<path d="m6 9 6 6 6-6"/>"#,
            Icon::Plus => r#"<path d="M12 5v14M5 12h14"/>"#,
            Icon::List => r#"<path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01"/>"#,
            Icon::Grid => {
                r#"<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>"#
            }
            Icon::Drive => r#"<rect x="3" y="13" width="18" height="7" rx="2"/><path d="M5 13 7.5 5h9L19 13M17 16.5h.01"/>"#,
            Icon::DriveSystem => {
                r#"<rect x="3" y="13" width="18" height="7" rx="2"/><path d="M5 13 7.5 5h9L19 13M17 16.5h.01"/><path d="M10 7h1.6v1.6H10zM12.4 7H14v1.6h-1.6zM10 9.4h1.6V11H10zM12.4 9.4H14V11h-1.6z" fill="currentColor" stroke="none"/>"#
            }
            Icon::Usb => r#"<rect x="7" y="10" width="10" height="12" rx="2"/><path d="M9 10V3h6v7M10.5 5.5V7M13.5 5.5V7"/>"#,
            Icon::Disc => r#"<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="2.5"/><path d="M12 5.5a6.5 6.5 0 0 1 6.5 6.5"/>"#,
            Icon::Network => r#"<rect x="3" y="3" width="18" height="9" rx="2"/><path d="M7 7.5h.01M12 12v5M4 21h16M12 17v4"/>"#,
            Icon::Download => r#"<path d="M12 4v11m0 0-4.5-4.5M12 15l4.5-4.5M5 20h14"/>"#,
            Icon::Cut => {
                r#"<circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M20 4 8.1 15.9M14.5 14.5 20 20M8.1 8.1 12 12"/>"#
            }
            Icon::Copy => {
                r#"<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1"/>"#
            }
            Icon::Paste => {
                r#"<rect x="8" y="2" width="8" height="4" rx="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>"#
            }
            Icon::Rename => r#"<path d="M4 20h4L18.5 9.5a2.1 2.1 0 0 0-3-3L5 17v3zM13.5 7.5l3 3"/>"#,
            Icon::Trash => r#"<path d="M4 7h16M10 11v6M14 11v6M5 7l1 12a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2l1-12M9 7V4h6v3"/>"#,
            Icon::Sort => r#"<path d="M7 4v16m0 0-3-3m3 3 3-3M17 20V4m0 0-3 3m3-3 3 3"/>"#,
            Icon::Eye => r#"<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>"#,
            Icon::More => r#"<path d="M5 12h.01M12 12h.01M19 12h.01" stroke-width="3"/>"#,
            Icon::Pin => r#"<path d="M12 17v5M9 3h6l-1 6 4 4H6l4-4z"/>"#,
            Icon::Palette => {
                r#"<path d="M12 3a9 9 0 1 0 0 18c1.1 0 1.6-.9 1.2-1.8-.5-1-.1-2.2 1.1-2.2H17a4 4 0 0 0 4-4c0-5.5-4-10-9-10z"/><circle cx="7.5" cy="11" r="1.2"/><circle cx="10.5" cy="7" r="1.2"/><circle cx="15.5" cy="7.5" r="1.2"/>"#
            }
            Icon::FolderGit => {
                r#"<path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H9l2 2h7.5A2.5 2.5 0 0 1 21 9.5v8a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 17.5z" fill="currentColor" fill-opacity=".2"/><path d="m14.5 10 3.5 3.5-3.5 3.5-3.5-3.5z" fill="currentColor" stroke="none"/>"#
            }
            Icon::Branch => {
                r#"<circle cx="6" cy="5.5" r="2.3"/><circle cx="6" cy="18.5" r="2.3"/><circle cx="18" cy="8" r="2.3"/><path d="M6 7.8v8.4M18 10.3c0 4.2-6.5 3.2-11 6.4"/>"#
            }
            Icon::WinMin => r#"<path d="M4 12h16"/>"#,
            Icon::WinMax => r#"<rect x="4" y="4" width="16" height="16" rx="2"/>"#,
            Icon::WinClose => r#"<path d="M5 5l14 14M19 5 5 19"/>"#,
            Icon::Logo => "",
        }
    }

    pub fn handle(self) -> svg::Handle {
        use std::sync::OnceLock;
        static CACHE: OnceLock<std::sync::Mutex<Vec<(Icon, svg::Handle)>>> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let mut cache = cache.lock().expect("cache de ícones");
        if let Some((_, h)) = cache.iter().find(|(i, _)| *i == self) {
            return h.clone();
        }
        let source = if self == Icon::Logo {
            LOGO_SVG.to_string()
        } else {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
                self.path().replace("currentColor", "black")
            )
        };
        let h = svg::Handle::from_memory(source.into_bytes());
        cache.push((self, h.clone()));
        h
    }
}

/// A árvore do protótipo, com o gradiente gelo → íris.
pub const LOGO_SVG: &str = include_str!("../assets/logo.svg");

/// Ícone tingido com `color` (o SVG é desenhado em preto e recolorido pelo iced).
pub fn icon<'a, M: 'a>(i: Icon, size: f32, color: Color) -> iced::Element<'a, M> {
    let s = svg(i.handle()).width(size).height(size);
    if i == Icon::Logo {
        s.into()
    } else {
        s.style(move |_, _| svg::Style { color: Some(color) }).into()
    }
}
