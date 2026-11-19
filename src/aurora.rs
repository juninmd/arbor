//! A aurora do protótipo "arbor Glass": quatro manchas de luz (gelo, íris, rosa, menta) somadas em modo
//! *screen* sobre o fundo e bem desfocadas. O iced não tem gradiente radial, então a aurora é pintada uma
//! vez numa textura pequena; a GPU estica com filtro linear, o que já dá o desfoque.

use iced::widget::image::Handle;

/// Pequena de propósito: a ampliação linear faz o papel do `filter: blur(80px)` do protótipo.
const W: u32 = 240;
const H: u32 = 150;

/// (cor, centro x, centro y, raio, opacidade) em frações da largura (x, raio) e da altura (y) da janela,
/// tiradas do CSS do protótipo para uma janela 16:10.
const BLOBS: [(u32, f32, f32, f32, f32); 4] = [
    (0x1fc8ff, 0.17, 0.41, 0.30, 0.75),
    (0x7b54ff, 0.92, 0.19, 0.28, 0.75),
    (0xff3f9e, 0.52, 0.93, 0.26, 0.55),
    (0x13f0b2, -0.10, 0.89, 0.20, 0.50),
];

fn rgb(hex: u32) -> [f32; 3] {
    [((hex >> 16) & 0xff) as f32 / 255.0, ((hex >> 8) & 0xff) as f32 / 255.0, (hex & 0xff) as f32 / 255.0]
}

/// Pinta a aurora. `opacity` < 1 deixa o vidro nativo do sistema (vibrancy, transparência) aparecer por trás.
pub fn render(dark: bool, opacity: f32) -> Handle {
    let base = if dark { rgb(0x060914) } else { rgb(0xf3f5fc) };
    let aspect = W as f32 / H as f32;
    let mut px = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let (u, v) = ((x as f32 + 0.5) / W as f32, (y as f32 + 0.5) / H as f32);
            let mut c = base;
            for &(hex, bx, by, r, a) in &BLOBS {
                let (dx, dy) = (u - bx, (v - by) / aspect);
                let d = (dx * dx + dy * dy).sqrt() / (r * 1.25);
                let s = (1.0 - d).clamp(0.0, 1.0);
                let s = s * s * (3.0 - 2.0 * s) * a * 0.88;
                let k = rgb(hex);
                for i in 0..3 {
                    c[i] = if dark {
                        1.0 - (1.0 - c[i]) * (1.0 - k[i] * s) // screen, como o mix-blend-mode do CSS
                    } else {
                        c[i] + (k[i] - c[i]) * s * 0.32 // no claro, só um véu de cor
                    };
                }
            }
            // saturate(150%) do protótipo
            let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
            for ch in c {
                px.push(((l + (ch - l) * 1.5).clamp(0.0, 1.0) * 255.0).round() as u8);
            }
            px.push((opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    Handle::from_rgba(W, H, px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aurora_tem_cor_e_respeita_opacidade() {
        let Handle::Rgba { width, height, pixels, .. } = render(true, 0.5) else { panic!("esperava RGBA") };
        assert_eq!((width, height), (W, H));
        assert!(pixels.chunks(4).all(|p| p[3] == 128));
        // perto da mancha gelo o azul domina; no canto escuro sobra o fundo
        let at = |x: u32, y: u32| &pixels[((y * W + x) * 4) as usize..][..3];
        let ice = at((W as f32 * 0.17) as u32, (H as f32 * 0.41) as u32);
        assert!(ice[2] > 150 && ice[2] > ice[0]);
        let corner = at(W / 2, 2);
        assert!(corner.iter().all(|&c| c < 120));
    }
}
