//! Velocidade e tempo restante de uma cópia, com o histórico do gráfico do Explorer.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Janela usada na média: curta para reagir, longa para não pular a cada arquivo.
const WINDOW: Duration = Duration::from_secs(3);
/// Uma barra do gráfico a cada meio segundo; 40 barras mostram os últimos 20 s.
const STEP: Duration = Duration::from_millis(500);
const BARS: usize = 40;

#[derive(Debug, Default)]
pub struct Rate {
    samples: VecDeque<(Instant, u64)>,
    pub history: VecDeque<f64>,
    last_bar: Option<Instant>,
}

impl Rate {
    pub fn push(&mut self, now: Instant, done: u64) {
        // Bytes que voltam (outro arquivo, recomeço) zeram a janela em vez de dar velocidade negativa.
        if self.samples.back().is_some_and(|&(_, d)| done < d) {
            self.samples.clear();
        }
        self.samples.push_back((now, done));
        while self.samples.len() > 2 && self.samples.front().is_some_and(|&(t, _)| now.duration_since(t) > WINDOW) {
            self.samples.pop_front();
        }
        if self.last_bar.is_none_or(|t| now.duration_since(t) >= STEP) {
            self.last_bar = Some(now);
            if let Some(v) = self.per_sec() {
                self.history.push_back(v);
                if self.history.len() > BARS {
                    self.history.pop_front();
                }
            }
        }
    }

    /// Bytes por segundo na janela, se já houver tempo suficiente medido.
    pub fn per_sec(&self) -> Option<f64> {
        let (&(t0, d0), &(t1, d1)) = (self.samples.front()?, self.samples.back()?);
        let secs = t1.duration_since(t0).as_secs_f64();
        (secs >= 0.25).then(|| (d1 - d0) as f64 / secs)
    }

    pub fn eta(&self, done: u64, total: u64) -> Option<Duration> {
        let v = self.per_sec().filter(|v| *v > 0.0)?;
        Some(Duration::from_secs_f64(total.saturating_sub(done) as f64 / v))
    }
}

/// "faltam 2 min 5 s", "falta 1 s".
pub fn human_eta(d: Duration) -> String {
    let s = d.as_secs().max(1);
    match (s / 3600, s % 3600 / 60, s % 60) {
        (0, 0, 1) => "falta 1 s".into(),
        (0, 0, sec) => format!("faltam {sec} s"),
        (0, m, sec) => format!("faltam {m} min {sec} s"),
        (h, m, _) => format!("faltam {h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_and_eta_follow_the_recent_window() {
        let t = Instant::now();
        let mut r = Rate::default();
        r.push(t, 0);
        assert_eq!(r.per_sec(), None, "um ponto só não dá velocidade");
        r.push(t + Duration::from_secs(1), 10_000_000);
        assert_eq!(r.per_sec(), Some(10_000_000.0));
        assert_eq!(r.eta(10_000_000, 40_000_000), Some(Duration::from_secs(3)));
        // Mais devagar depois: a janela de 3 s esquece o começo.
        r.push(t + Duration::from_secs(5), 12_000_000);
        r.push(t + Duration::from_secs(6), 13_000_000);
        assert_eq!(r.per_sec(), Some(1_000_000.0));
        assert!(!r.history.is_empty() && r.history.len() <= BARS);
    }

    #[test]
    fn going_backwards_restarts_instead_of_negative_speed() {
        let t = Instant::now();
        let mut r = Rate::default();
        r.push(t, 500);
        r.push(t + Duration::from_secs(1), 100);
        assert_eq!(r.per_sec(), None);
        assert_eq!(r.eta(0, 10), None);
    }

    #[test]
    fn eta_reads_like_explorer() {
        assert_eq!(human_eta(Duration::from_millis(300)), "falta 1 s");
        assert_eq!(human_eta(Duration::from_secs(42)), "faltam 42 s");
        assert_eq!(human_eta(Duration::from_secs(125)), "faltam 2 min 5 s");
        assert_eq!(human_eta(Duration::from_secs(3 * 3600 + 120)), "faltam 3 h 2 min");
    }
}
