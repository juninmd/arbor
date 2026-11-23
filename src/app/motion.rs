//! Animações curtas: diálogos que entram, painel de detalhes que desliza e o cartão de avisos.
//! Cada uma é um `Animation<bool>` do iced; enquanto alguma corre, a assinatura de quadros redesenha
//! a janela, e parada não custa nada. `ARBOR_NO_ANIM=1` desliga (movimento reduzido).

use super::{App, Overlay, UpdateState};
use iced::animation::Easing;
use iced::Animation;
use std::collections::{HashMap, HashSet};
use std::mem::Discriminant;
use std::time::{Duration, Instant};

/// Entrada de diálogo e menu: rápida, para não atrasar quem usa o teclado.
const OVERLAY_MS: u64 = 160;
const PANEL_MS: u64 = 220;
const NOTICE_MS: u64 = 240;
const TAB_MS: u64 = 200;
const SIDEBAR_MS: u64 = 180;
/// ~60 quadros por segundo enquanto alguma animação corre.
pub const FRAME: Duration = Duration::from_millis(16);

pub struct Motion {
    overlay: Animation<bool>,
    details: Animation<bool>,
    notices: Animation<bool>,
    sidebar: Animation<bool>,
    /// Abas que acabaram de abrir e quando; o resto já está inteiro.
    tab_born: HashMap<u64, Instant>,
    tab_seen: Option<HashSet<u64>>,
}

impl Default for Motion {
    fn default() -> Self {
        Motion {
            overlay: anim(OVERLAY_MS),
            details: anim(PANEL_MS),
            notices: anim(NOTICE_MS),
            sidebar: anim(SIDEBAR_MS),
            tab_born: HashMap::new(),
            tab_seen: None,
        }
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
        self.overlay.is_animating(at)
            || self.details.is_animating(at)
            || self.notices.is_animating(at)
            || self.sidebar.is_animating(at)
            || self.tab_born.values().any(|b| at.saturating_duration_since(*b) < Duration::from_millis(TAB_MS))
    }

    pub fn set_sidebar(&mut self, open: bool, at: Instant) {
        if self.sidebar.value() != open {
            self.sidebar.go_mut(open, at);
        }
    }

    /// Fração aberta da barra lateral flutuante; > 0 enquanto ainda fecha.
    pub fn sidebar(&self, at: Instant) -> f32 {
        self.sidebar.interpolate(0.0, 1.0, at)
    }

    /// Registra as abas existentes; as que não estavam antes entram animadas (a primeira leitura não anima).
    pub fn observe_tabs(&mut self, ids: &[u64], at: Instant) {
        let now_ids: HashSet<u64> = ids.iter().copied().collect();
        if let Some(seen) = &self.tab_seen {
            if !reduced() {
                for id in now_ids.difference(seen) {
                    self.tab_born.insert(*id, at);
                }
            }
        }
        self.tab_born.retain(|id, b| now_ids.contains(id) && at.saturating_duration_since(*b) < Duration::from_millis(TAB_MS));
        self.tab_seen = Some(now_ids);
    }

    /// 0 → 1 enquanto a aba abre; 1 para as demais.
    pub fn tab_open(&self, id: u64, at: Instant) -> f32 {
        let Some(born) = self.tab_born.get(&id) else { return 1.0 };
        let t = (at.saturating_duration_since(*born).as_secs_f32() / (TAB_MS as f32 / 1000.0)).clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
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
        self.motion.set_sidebar(self.sidebar_hide && self.sidebar_hover, now);
        let ids: Vec<u64> = self.panes.iter().flat_map(|p| p.tabs.iter().map(|t| t.id)).collect();
        self.motion.observe_tabs(&ids, now);
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
    fn new_tab_grows_in_and_old_ones_stay_whole() {
        let t0 = Instant::now();
        let mut m = Motion::default();
        m.observe_tabs(&[1], t0);
        assert_eq!(m.tab_open(1, t0), 1.0, "a primeira leitura não anima");
        m.observe_tabs(&[1, 2], t0);
        assert_eq!(m.tab_open(1, t0), 1.0);
        let mid = m.tab_open(2, t0 + Duration::from_millis(TAB_MS / 2));
        assert!(reduced() || (mid > 0.0 && mid < 1.0), "no meio da entrada: {mid}");
        assert_eq!(m.tab_open(2, t0 + Duration::from_millis(TAB_MS + 5)), 1.0);
        assert!(!m.animating(t0 + Duration::from_millis(TAB_MS + 5)));
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
