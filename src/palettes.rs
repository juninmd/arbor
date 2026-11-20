//! Temas: cada um é uma paleta de tokens (`theme::Tokens`) mais as manchas da aurora atrás do vidro.
//!
//! O tema escolhido fica em `settings` (`theme=<id>`) e vale para a janela toda. `current()` é lido pelos
//! estilos a cada quadro, por isso é um atômico e não um campo que precisaria descer por todas as funções.

use crate::commands::Cmd;
use crate::theme::{Tokens, DARK, LIGHT};
use iced::Color;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Palette {
    #[default]
    Aurora,
    Light,
    Midnight,
    Graphite,
    Ocean,
    Sunset,
    Canto,
    HuecoMundo,
    Dracula,
    Vidro,
}

pub const ALL: [Palette; 10] = [
    Palette::Aurora,
    Palette::Light,
    Palette::Midnight,
    Palette::Graphite,
    Palette::Ocean,
    Palette::Sunset,
    Palette::Canto,
    Palette::HuecoMundo,
    Palette::Dracula,
    Palette::Vidro,
];

/// Mancha da aurora: (cor, centro x, centro y, raio, opacidade), em frações da janela.
pub type Blob = (u32, f32, f32, f32, f32);

const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const fn rgba(hex: u32, a: f32) -> Color {
    let c = rgb(hex);
    Color { a, ..c }
}

/// Tema escuro sobre outro fundo e outras cores de destaque, com o mesmo texto e vidro do Aurora.
const fn dark_with(void: u32, surface: u32, accent: u32, accent2: u32) -> Tokens {
    Tokens { void: rgb(void), surface: rgba(surface, 0.88), accent: rgb(accent), accent2: rgb(accent2), ..DARK }
}

const MIDNIGHT: Tokens =
    Tokens { glass: rgba(0xffffff, 0.06), glass_hi: rgba(0xffffff, 0.05), ..dark_with(0x000000, 0x111318, 0x8fe6ff, 0xb9a2ff) };
const GRAPHITE: Tokens = dark_with(0x15171c, 0x262a33, 0x9ad1ff, 0xc9b8ff);
const OCEAN: Tokens = dark_with(0x021318, 0x0f2e38, 0x6ff0e0, 0x7fb2ff);
const SUNSET: Tokens = dark_with(0x140a10, 0x34202c, 0xffb38a, 0xff8fc8);

/// Skins do canto-widget (src/styles.css de juninmd/canto-widget): fundo, painel, texto, texto fraco e as cores de
/// destaque e de estado de cada uma, levadas para os tokens do arbor.
const fn canto(void: u32, panel: u32, fg: u32, muted: u32, accent: u32, accent2: u32, state: [u32; 3]) -> Tokens {
    Tokens {
        void: rgb(void),
        surface: rgba(panel, 0.88),
        ink: rgb(fg),
        ink2: rgba(muted, 0.92),
        ink3: rgba(muted, 0.62),
        accent: rgb(accent),
        accent2: rgb(accent2),
        danger: rgb(state[0]),
        good: rgb(state[1]),
        warn: rgb(state[2]),
        ..DARK
    }
}

const CANTO: Tokens = canto(0x0b0f16, 0x111826, 0xe2e8f0, 0x94a3b8, 0x4ade80, 0x38bdf8, [0xfb7185, 0x4ade80, 0xfbbf24]);
const HUECO_MUNDO: Tokens = canto(0x050506, 0x0e0e11, 0xede9e0, 0xa8a29e, 0xf6506d, 0xede9e0, [0xf87171, 0x86efac, 0xfbbf24]);
const DRACULA: Tokens = canto(0x21222c, 0x282a36, 0xf8f8f2, 0xb6b8c8, 0xbd93f9, 0xff79c6, [0xff5555, 0x50fa7b, 0xf1fa8c]);
const VIDRO: Tokens = canto(0x0c1017, 0x161b26, 0xf5f5f7, 0xa1abbc, 0x409cff, 0xbf5af2, [0xff453a, 0x32d74b, 0xffd60a]);

const AURORA_BLOBS: [Blob; 4] = [
    (0x1fc8ff, 0.17, 0.41, 0.30, 0.75),
    (0x7b54ff, 0.92, 0.19, 0.28, 0.75),
    (0xff3f9e, 0.52, 0.93, 0.26, 0.55),
    (0x13f0b2, -0.10, 0.89, 0.20, 0.50),
];

impl Palette {
    pub fn id(self) -> &'static str {
        match self {
            Palette::Aurora => "aurora",
            Palette::Light => "claro",
            Palette::Midnight => "meia-noite",
            Palette::Graphite => "grafite",
            Palette::Ocean => "oceano",
            Palette::Sunset => "por-do-sol",
            Palette::Canto => "canto",
            Palette::HuecoMundo => "hueco-mundo",
            Palette::Dracula => "dracula",
            Palette::Vidro => "vidro",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Palette::Aurora => "Aurora",
            Palette::Light => "Claro",
            Palette::Midnight => "Meia-noite",
            Palette::Graphite => "Grafite",
            Palette::Ocean => "Oceano",
            Palette::Sunset => "Pôr do sol",
            Palette::Canto => "Canto",
            Palette::HuecoMundo => "Hueco Mundo",
            Palette::Dracula => "Drácula",
            Palette::Vidro => "Vidro",
        }
    }

    pub fn from_id(id: &str) -> Option<Palette> {
        ALL.into_iter().find(|p| p.id() == id.trim())
    }

    pub fn is_dark(self) -> bool {
        self != Palette::Light
    }

    pub fn tokens(self) -> Tokens {
        match self {
            Palette::Aurora => DARK,
            Palette::Light => LIGHT,
            Palette::Midnight => MIDNIGHT,
            Palette::Graphite => GRAPHITE,
            Palette::Ocean => OCEAN,
            Palette::Sunset => SUNSET,
            Palette::Canto => CANTO,
            Palette::HuecoMundo => HUECO_MUNDO,
            Palette::Dracula => DRACULA,
            Palette::Vidro => VIDRO,
        }
    }

    pub fn blobs(self) -> [Blob; 4] {
        match self {
            Palette::Aurora | Palette::Light => AURORA_BLOBS,
            // Meia-noite: a mesma aurora, bem mais fraca, para telas OLED e ambientes escuros.
            Palette::Midnight => AURORA_BLOBS.map(|(c, x, y, r, a)| (c, x, y, r, a * 0.35)),
            Palette::Graphite => [
                (0x5a6b8c, 0.17, 0.41, 0.30, 0.45),
                (0x6c6f7d, 0.92, 0.19, 0.28, 0.40),
                (0x3d4a5e, 0.52, 0.93, 0.26, 0.40),
                (0x4f5b66, -0.10, 0.89, 0.20, 0.35),
            ],
            Palette::Ocean => [
                (0x00b3c7, 0.17, 0.41, 0.30, 0.70),
                (0x2a5bd7, 0.92, 0.19, 0.28, 0.65),
                (0x00e0a4, 0.52, 0.93, 0.26, 0.45),
                (0x0a7ea8, -0.10, 0.89, 0.20, 0.50),
            ],
            Palette::Sunset => [
                (0xff7a45, 0.17, 0.41, 0.30, 0.65),
                (0xb03cff, 0.92, 0.19, 0.28, 0.60),
                (0xff3f7f, 0.52, 0.93, 0.26, 0.55),
                (0xffc145, -0.10, 0.89, 0.20, 0.45),
            ],
            Palette::Canto => [
                (0x4ade80, 0.17, 0.41, 0.30, 0.45),
                (0x38bdf8, 0.92, 0.19, 0.28, 0.45),
                (0xa78bfa, 0.52, 0.93, 0.26, 0.35),
                (0x2dd4bf, -0.10, 0.89, 0.20, 0.35),
            ],
            // Hueco Mundo: lua pálida e um brilho vermelho, quase sem cor.
            Palette::HuecoMundo => [
                (0xede9e0, 0.92, 0.19, 0.28, 0.18),
                (0xe11d48, 0.17, 0.41, 0.30, 0.35),
                (0x7f1d1d, 0.52, 0.93, 0.26, 0.40),
                (0x44403c, -0.10, 0.89, 0.20, 0.35),
            ],
            Palette::Dracula => [
                (0xbd93f9, 0.17, 0.41, 0.30, 0.50),
                (0xff79c6, 0.92, 0.19, 0.28, 0.45),
                (0x8be9fd, 0.52, 0.93, 0.26, 0.30),
                (0x50fa7b, -0.10, 0.89, 0.20, 0.25),
            ],
            Palette::Vidro => [
                (0x0a84ff, 0.17, 0.41, 0.30, 0.55),
                (0xbf5af2, 0.92, 0.19, 0.28, 0.45),
                (0x64d2ff, 0.52, 0.93, 0.26, 0.35),
                (0xff375f, -0.10, 0.89, 0.20, 0.30),
            ],
        }
    }

    /// Comando da paleta e do menu de temas que escolhe este tema.
    pub fn cmd(self) -> Cmd {
        match self {
            Palette::Aurora => Cmd::ThemeAurora,
            Palette::Light => Cmd::ThemeLight,
            Palette::Midnight => Cmd::ThemeMidnight,
            Palette::Graphite => Cmd::ThemeGraphite,
            Palette::Ocean => Cmd::ThemeOcean,
            Palette::Sunset => Cmd::ThemeSunset,
            Palette::Canto => Cmd::ThemeCanto,
            Palette::HuecoMundo => Cmd::ThemeHuecoMundo,
            Palette::Dracula => Cmd::ThemeDracula,
            Palette::Vidro => Cmd::ThemeVidro,
        }
    }

    pub fn of_cmd(cmd: Cmd) -> Option<Palette> {
        ALL.into_iter().find(|p| p.cmd() == cmd)
    }

    /// O próximo da lista, para o atalho "Próximo tema".
    pub fn next(self) -> Palette {
        let i = ALL.iter().position(|p| *p == self).unwrap_or(0);
        ALL[(i + 1) % ALL.len()]
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(p: Palette) {
    let i = ALL.iter().position(|x| *x == p).unwrap_or(0);
    CURRENT.store(i as u8, Ordering::Relaxed);
}

pub fn current() -> Palette {
    ALL.get(CURRENT.load(Ordering::Relaxed) as usize).copied().unwrap_or_default()
}

pub fn load() -> Palette {
    crate::session::setting("theme").and_then(|v| Palette::from_id(&v)).unwrap_or_default()
}

pub fn save(p: Palette) -> Result<(), String> {
    crate::session::set_setting("theme", p.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_sao_unicos_e_voltam() {
        for p in ALL {
            assert_eq!(Palette::from_id(p.id()), Some(p));
        }
        assert_eq!(Palette::from_id("inexistente"), None);
        for p in ALL {
            assert_eq!(Palette::of_cmd(p.cmd()), Some(p));
        }
        assert_eq!(Palette::of_cmd(Cmd::Copy), None);
        let mut seen: Vec<&str> = ALL.iter().map(|p| p.id()).collect();
        seen.dedup();
        assert_eq!(seen.len(), ALL.len());
    }

    #[test]
    fn proximo_percorre_todos_e_volta_ao_inicio() {
        let mut p = Palette::Aurora;
        for _ in 0..ALL.len() {
            p = p.next();
        }
        assert_eq!(p, Palette::Aurora);
    }

    #[test]
    fn so_o_claro_e_claro_e_o_texto_contrasta_com_o_fundo() {
        for p in ALL {
            let k = p.tokens();
            let lum = |c: Color| 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
            assert_eq!(p.is_dark(), lum(k.void) < 0.5, "{}", p.name());
            assert!((lum(k.ink) - lum(k.void)).abs() > 0.6, "{} sem contraste", p.name());
        }
    }
}
