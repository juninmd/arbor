//! Modos da grade, do menor ao maior, percorridos com Ctrl+roda como no Explorer. Detalhes (a lista com
//! colunas) fica abaixo do primeiro e é representado por `None`.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shape {
    /// Ícone pequeno e nome numa linha.
    Line,
    /// Ícone médio ao lado de nome, tipo e tamanho.
    Tile,
    /// Miniatura em cima, nome e tamanho embaixo.
    Icon,
}

pub struct Mode {
    pub name: &'static str,
    pub shape: Shape,
    /// Largura e altura da célula, com o espaçamento entre células.
    pub w: f32,
    pub h: f32,
    /// Lado da miniatura ou do ícone.
    pub art: f32,
}

const fn icons(name: &'static str, z: f32) -> Mode {
    Mode { name, shape: Shape::Icon, w: 142.0 * z, h: 168.0 + 96.0 * (z - 1.0), art: 96.0 * z }
}

pub const MODES: [Mode; 6] = [
    Mode { name: "Lista", shape: Shape::Line, w: 250.0, h: 34.0, art: 20.0 },
    Mode { name: "Blocos", shape: Shape::Tile, w: 300.0, h: 84.0, art: 56.0 },
    icons("Ícones pequenos", 0.75),
    icons("Ícones médios", 1.0),
    icons("Ícones grandes", 1.35),
    icons("Ícones extra grandes", 1.8),
];

/// Ícones médios: o modo da grade antes de qualquer zoom.
pub const DEFAULT: usize = 3;

/// Próximo modo ao aumentar ou diminuir; `None` é Detalhes, abaixo de Lista.
pub fn step(current: Option<usize>, larger: bool) -> Option<usize> {
    match (current, larger) {
        (None, true) => Some(0),
        (None, false) | (Some(0), false) => None,
        (Some(i), false) => Some(i - 1),
        (Some(i), true) => Some((i + 1).min(MODES.len() - 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_wheel_walks_details_list_tiles_icons_and_stops_at_ends() {
        let mut at = None;
        let mut seen = vec![];
        for _ in 0..8 {
            at = step(at, true);
            seen.push(at.map_or("Detalhes", |i| MODES[i].name));
        }
        assert_eq!(&seen[..6], ["Lista", "Blocos", "Ícones pequenos", "Ícones médios", "Ícones grandes", "Ícones extra grandes"]);
        assert_eq!(seen[7], "Ícones extra grandes");
        for _ in 0..8 {
            at = step(at, false);
        }
        assert_eq!(at, None, "diminuir sempre termina em Detalhes");
        assert_eq!(step(None, false), None);
    }

    #[test]
    fn cells_grow_with_the_mode() {
        assert!(MODES.windows(2).skip(2).all(|w| w[0].art < w[1].art && w[0].w < w[1].w));
        assert_eq!(MODES[DEFAULT].w, 142.0);
    }
}
