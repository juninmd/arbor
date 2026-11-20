//! Temas vindos das skins do canto-widget (juninmd/canto-widget, `src/styles.css`), com os mesmos valores.

use super::{rgb, rgba, Blob};
use crate::theme::{Hues, Tokens};

/// Uma skin do canto-widget, com os nomes de `src/styles.css` de juninmd/canto-widget.
struct Skin {
    ink: u32,
    panel: u32,
    edge: u32,
    accent: u32,
    accent_text: u32,
    fg: u32,
    muted: u32,
    faint: u32,
    line: u32,
    state: [u32; 3],
    /// games, meet, docs, chat, web: as cores de categoria viram os chips de ícone.
    cat: [u32; 5],
}

/// Leva a skin para os tokens: fundo e painel opacos, bordas sólidas e os mesmos tons de texto, sem a aurora do arbor.
const fn canto(k: Skin) -> Tokens {
    Tokens {
        void: rgb(k.ink),
        surface: rgba(k.panel, 0.95),
        ink: rgb(k.fg),
        ink2: rgb(k.muted),
        ink3: rgb(k.faint),
        stroke: rgb(k.edge),
        stroke_hi: rgb(k.line),
        glass: rgba(k.panel, 0.92),
        glass_hi: rgba(k.fg, 0.07),
        accent: rgb(k.accent_text),
        accent2: rgb(k.accent),
        danger: rgb(k.state[0]),
        good: rgb(k.state[1]),
        warn: rgb(k.state[2]),
        hues: Hues {
            ice: rgb(k.accent_text),
            rose: rgb(k.cat[0]),
            iris: rgb(k.cat[3]),
            amber: rgb(k.cat[2]),
            mint: rgb(k.cat[4]),
            coral: rgb(k.state[0]),
            steel: rgb(k.muted),
        },
    }
}

pub(super) const CANTO: Tokens = canto(Skin {
    ink: 0x0b0f16,
    panel: 0x111826,
    edge: 0x1f2a3c,
    accent: 0x4ade80,
    accent_text: 0x4ade80,
    fg: 0xe2e8f0,
    muted: 0x94a3b8,
    faint: 0x8491a7,
    line: 0x586781,
    state: [0xfb7185, 0x4ade80, 0xfbbf24],
    cat: [0xfb7185, 0x38bdf8, 0xfbbf24, 0xa78bfa, 0x2dd4bf],
});
pub(super) const HUECO_MUNDO: Tokens = canto(Skin {
    ink: 0x050506,
    panel: 0x0e0e11,
    edge: 0x2c2c33,
    accent: 0xe11d48,
    accent_text: 0xf6506d,
    fg: 0xede9e0,
    muted: 0xa8a29e,
    faint: 0x948e86,
    line: 0x62626b,
    state: [0xf87171, 0x86efac, 0xfbbf24],
    cat: [0xfb923c, 0x7dd3fc, 0xfcd34d, 0xc4b5fd, 0x5eead4],
});
pub(super) const DRACULA: Tokens = canto(Skin {
    ink: 0x21222c,
    panel: 0x282a36,
    edge: 0x44475a,
    accent: 0xbd93f9,
    accent_text: 0xbd93f9,
    fg: 0xf8f8f2,
    muted: 0xb6b8c8,
    faint: 0x9ba6cf,
    line: 0x737898,
    state: [0xff5555, 0x50fa7b, 0xf1fa8c],
    cat: [0xff5555, 0x8be9fd, 0xffb86c, 0xff79c6, 0x50fa7b],
});
/// Glass: a única skin translúcida do canto, por isso fica com o vidro do arbor por cima de um brilho azul discreto.
pub(super) const VIDRO: Tokens = Tokens {
    surface: rgba(0x161b26, 0.75),
    glass: rgba(0x161b26, 0.62),
    stroke: rgba(0xffffff, 0.15),
    ..canto(Skin {
        ink: 0x0c1017,
        panel: 0x161b26,
        edge: 0x273142,
        accent: 0x0a84ff,
        accent_text: 0x409cff,
        fg: 0xf5f5f7,
        muted: 0xa1abbc,
        faint: 0x8894a8,
        line: 0x586987,
        state: [0xff453a, 0x32d74b, 0xffd60a],
        cat: [0xff375f, 0x64d2ff, 0xffd60a, 0xbf5af2, 0x30d158],
    })
};

/// Skins opacas do canto: sem aurora, só o fundo.
pub(super) const NO_BLOBS: [Blob; 4] = [(0, 0.0, 0.0, 0.1, 0.0); 4];
