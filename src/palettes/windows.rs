//! Temas no estilo do Explorer do Windows 11: fundo Mica opaco, acento azul do sistema, pasta amarela.

use super::{rgb, rgba};
use crate::theme::{Hues, Tokens};

pub(super) const EXPLORER_CLARO: Tokens = Tokens {
    void: rgb(0xf3f3f3),
    surface: rgba(0xfbfbfb, 0.97),
    ink: rgb(0x1b1b1b),
    ink2: rgb(0x5d5d5d),
    ink3: rgb(0x6b6b6b),
    stroke: rgb(0xe5e5e5),
    stroke_hi: rgb(0xc8c8c8),
    glass: rgba(0xffffff, 0.92),
    glass_hi: rgba(0x000000, 0.05),
    accent: rgb(0x0067c0),
    accent2: rgb(0x003e92),
    danger: rgb(0xc42b1c),
    good: rgb(0x0f7b0f),
    warn: rgb(0x9d5d00),
    hues: Hues {
        ice: rgb(0x0067c0),
        iris: rgb(0x5b5fc7),
        rose: rgb(0xc42b1c),
        amber: rgb(0xd69a00),
        mint: rgb(0x0f7b0f),
        coral: rgb(0xc4501c),
        steel: rgb(0x5d5d5d),
    },
};

pub(super) const EXPLORER_ESCURO: Tokens = Tokens {
    void: rgb(0x202020),
    surface: rgba(0x2b2b2b, 0.97),
    ink: rgb(0xffffff),
    ink2: rgb(0xc5c5c5),
    ink3: rgb(0x9d9d9d),
    stroke: rgb(0x3a3a3a),
    stroke_hi: rgb(0x606060),
    glass: rgba(0x2d2d2d, 0.92),
    glass_hi: rgba(0xffffff, 0.06),
    accent: rgb(0x60cdff),
    accent2: rgb(0x0078d4),
    danger: rgb(0xff99a4),
    good: rgb(0x6ccb5f),
    warn: rgb(0xfce100),
    hues: Hues {
        ice: rgb(0x60cdff),
        iris: rgb(0x9ea2ff),
        rose: rgb(0xff99a4),
        amber: rgb(0xffb900),
        mint: rgb(0x6ccb5f),
        coral: rgb(0xf7630c),
        steel: rgb(0xc5c5c5),
    },
};
