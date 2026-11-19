//! Última posição do mouse, guardada fora do estado do app: um `Message` por movimento redesenharia a
//! janela inteira a cada pixel. Só os menus (botão direito, Classificar, Visualizar) leem a posição.

use iced::{mouse, Event, Point};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static POS: AtomicU64 = AtomicU64::new(0);
/// Um gesto (arrastar, retângulo) está ativo: só então movimentos viram mensagens.
static ARMED: AtomicBool = AtomicBool::new(false);

pub fn arm(on: bool) {
    ARMED.store(on, Ordering::Relaxed);
}

pub fn armed() -> bool {
    ARMED.load(Ordering::Relaxed)
}

/// Ctrl (⌘ no macOS) pressionado: a roda do mouse vira zoom dos ícones.
static CTRL: AtomicBool = AtomicBool::new(false);

pub fn set_ctrl(on: bool) {
    CTRL.store(on, Ordering::Relaxed);
}

pub fn ctrl() -> bool {
    CTRL.load(Ordering::Relaxed)
}

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
