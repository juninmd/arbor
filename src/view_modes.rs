//! Modos da grade, percorridos com Ctrl+roda como no Explorer: Conteúdo ← Detalhes → Lista → Blocos →
//! ícones pequenos … extra grandes. Detalhes (a lista com colunas) é representado por `None`.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shape {
    /// Ícone pequeno e nome numa linha.
    Line,
    /// Ícone médio ao lado de nome, tipo e tamanho.
    Tile,
    /// Miniatura em cima, nome e tamanho embaixo.
    Icon,
    /// Uma faixa por item na largura toda: ícone, nome e tipo à esquerda, data e tamanho à direita.
    Content,
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

pub const MODES: [Mode; 7] = [
    Mode { name: "Lista", shape: Shape::Line, w: 250.0, h: 34.0, art: 20.0 },
    Mode { name: "Blocos", shape: Shape::Tile, w: 300.0, h: 84.0, art: 56.0 },
    icons("Ícones pequenos", 0.75),
    icons("Ícones médios", 1.0),
    icons("Ícones grandes", 1.35),
    icons("Ícones extra grandes", 1.8),
    // Largura vem da janela (uma coluna só); `w` não é usado.
    Mode { name: "Conteúdo", shape: Shape::Content, w: 0.0, h: 68.0, art: 44.0 },
];

pub const CONTENT: usize = 6;

/// Ordem do Ctrl+roda, do menor ao maior; `None` é Detalhes.
const LADDER: [Option<usize>; 8] = [Some(CONTENT), None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(5)];

/// Ícones médios: o modo da grade antes de qualquer zoom.
pub const DEFAULT: usize = 3;

/// Próximo modo ao aumentar ou diminuir, parando nas pontas.
pub fn step(current: Option<usize>, larger: bool) -> Option<usize> {
    let at = LADDER.iter().position(|&m| m == current).unwrap_or(1);
    let next = if larger { (at + 1).min(LADDER.len() - 1) } else { at.saturating_sub(1) };
    LADDER[next]
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
        for _ in 0..6 {
            at = step(at, false);
        }
        assert_eq!(at, None, "seis passos abaixo de extra grandes é Detalhes");
        assert_eq!(step(None, false), Some(CONTENT), "abaixo de Detalhes vem Conteúdo, como no Explorer");
        assert_eq!(step(Some(CONTENT), false), Some(CONTENT));
        assert_eq!(step(Some(CONTENT), true), None);
    }

    #[test]
    fn cells_grow_with_the_mode() {
        assert!(MODES[2..CONTENT].windows(2).all(|w| w[0].art < w[1].art && w[0].w < w[1].w));
        assert_eq!(MODES[DEFAULT].w, 142.0);
    }
}
