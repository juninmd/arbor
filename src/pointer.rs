//! Última posição do mouse, guardada fora do estado do app: um `Message` por movimento redesenharia a
//! janela inteira a cada pixel. Só os menus (botão direito, Classificar, Visualizar) leem a posição.

use iced::{mouse, Event, Point};
use std::sync::atomic::{AtomicU64, Ordering};

static POS: AtomicU64 = AtomicU64::new(0);

pub fn track(event: &Event) {
    if let Event::Mouse(mouse::Event::CursorMoved { position }) = event {
        POS.store(((position.x.to_bits() as u64) << 32) | position.y.to_bits() as u64, Ordering::Relaxed);
    }
}

pub fn get() -> Point {
    let v = POS.load(Ordering::Relaxed);
    Point::new(f32::from_bits((v >> 32) as u32), f32::from_bits(v as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarda_a_ultima_posicao() {
        track(&Event::Mouse(mouse::Event::CursorMoved { position: Point::new(12.5, 340.0) }));
        assert_eq!(get(), Point::new(12.5, 340.0));
    }
}
