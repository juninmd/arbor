//! Animações curtas: diálogos que entram, painel de detalhes que desliza e o cartão de avisos.
//! Cada uma é um `Animation<bool>` do iced; enquanto alguma corre, a assinatura de quadros redesenha
//! a janela, e parada não custa nada. `ARBOR_NO_ANIM=1` desliga (movimento reduzido).

use super::{App, Overlay, UpdateState};
use iced::animation::Easing;
use iced::Animation;
use std::mem::Discriminant;
use std::time::{Duration, Instant};

/// Entrada de diálogo e menu: rápida, para não atrasar quem usa o teclado.
const OVERLAY_MS: u64 = 160;
const PANEL_MS: u64 = 220;
const NOTICE_MS: u64 = 240;
/// ~60 quadros por segundo enquanto alguma animação corre.
pub const FRAME: Duration = Duration::from_millis(16);

pub struct Motion {
    overlay: Animation<bool>,
    details: Animation<bool>,
    notices: Animation<bool>,
}

impl Default for Motion {
    fn default() -> Self {
        Motion { overlay: anim(OVERLAY_MS), details: anim(PANEL_MS), notices: anim(NOTICE_MS) }
    }
}

fn reduced() -> bool {
    std::env::var_os("ARBOR_NO_ANIM").is_some()
}

fn anim(ms: u64) -> Animation<bool> {
    let ms = if reduced() { 0 } else { ms };
    Animation::new(false).duration(Duration::from_millis(ms)).easing(Easing::EaseOutCubic)
}

impl Motion {
    pub fn animating(&self, at: Instant) -> bool {
        self.overlay.is_animating(at) || self.details.is_animating(at) || self.notices.is_animating(at)
    }

    /// Um diálogo novo (ou de outro tipo) abriu: recomeça a entrada do zero.
    pub fn overlay_opened(&mut self, at: Instant) {
        self.overlay = anim(OVERLAY_MS).go(true, at);
    }

    pub fn overlay_closed(&mut self) {
        self.overlay = anim(OVERLAY_MS);
    }

    pub fn set_details(&mut self, on: bool, at: Instant) {
        if self.details.value() != on {
            self.details.go_mut(on, at);
        }
    }

    pub fn set_notices(&mut self, on: bool, at: Instant) {
        if self.notices.value() != on {
            // Sumir é instantâneo (o conteúdo já saiu); só a entrada anima.
            self.notices = if on { anim(NOTICE_MS).go(true, at) } else { anim(NOTICE_MS) };
        }
    }

    /// 0 → 1 enquanto o diálogo entra.
    pub fn overlay(&self, at: Instant) -> f32 {
        self.overlay.interpolate(0.0, 1.0, at)
    }

    /// Fração aberta do painel de detalhes; > 0 enquanto ainda fecha.
    pub fn details(&self, at: Instant) -> f32 {
        self.details.interpolate(0.0, 1.0, at)
    }

    pub fn notices(&self, at: Instant) -> f32 {
        self.notices.interpolate(0.0, 1.0, at)
    }
}

impl App {
    /// Depois de cada mensagem: dispara as animações do que acabou de aparecer ou sumir.
    pub(super) fn observe_motion(&mut self, before: Discriminant<Overlay>) {
        let now = Instant::now();
        if std::mem::discriminant(&self.overlay) != before {
            match self.overlay {
                Overlay::None => self.motion.overlay_closed(),
                _ => self.motion.overlay_opened(now),
            }
        }
        self.motion.set_details(self.details_on, now);
        let notices = !self.jobs.is_empty() || self.toast.is_some() || !matches!(self.update, UpdateState::Idle);
        self.motion.set_notices(notices, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_reaches_its_end_and_stops_asking_for_frames() {
        let t0 = Instant::now();
        let mut m = Motion::default();
        assert!(!m.animating(t0), "parado não pede quadros");
        m.set_details(true, t0);
        let mid = m.details(t0 + Duration::from_millis(PANEL_MS / 2));
        assert!(reduced() || (mid > 0.0 && mid < 1.0), "no meio está entre fechado e aberto: {mid}");
        let end = t0 + Duration::from_millis(PANEL_MS + 50);
        assert_eq!(m.details(end), 1.0);
        assert!(!m.animating(end), "terminou: a assinatura de quadros some");
    }

    #[test]
    fn reopening_a_dialog_restarts_its_entrance() {
        let t0 = Instant::now();
        let mut m = Motion::default();
        m.overlay_opened(t0);
        let later = t0 + Duration::from_millis(OVERLAY_MS + 50);
        assert_eq!(m.overlay(later), 1.0);
        m.overlay_opened(later);
        assert!(reduced() || m.overlay(later) < 0.01, "outro diálogo entra de novo do começo");
    }
}
