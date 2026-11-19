//! Pastas frequentes ("frecência", como o zoxide): cada visita soma pontos, e pastas visitadas há pouco
//! valem mais. `Ctrl+K` pula para qualquer uma delas digitando parte do nome.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Soma máxima dos pontos; passando disso, todo mundo envelhece 10% e as esquecidas saem da lista.
const MAX_TOTAL: f64 = 2_000.0;

#[derive(Debug, Clone, Default)]
pub struct Frecency {
    entries: HashMap<PathBuf, (f64, u64)>, // (pontos, última visita em segundos Unix)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Frecency {
    pub fn visit(&mut self, path: &Path) {
        self.visit_at(path, now());
    }

    fn visit_at(&mut self, path: &Path, at: u64) {
        let e = self.entries.entry(path.to_path_buf()).or_insert((0.0, at));
        e.0 += 1.0;
        e.1 = at;
        let total: f64 = self.entries.values().map(|e| e.0).sum();
        if total > MAX_TOTAL {
            self.entries.values_mut().for_each(|e| e.0 *= 0.9);
            self.entries.retain(|_, e| e.0 >= 1.0);
        }
    }

    fn score(rank: f64, last: u64, at: u64) -> f64 {
        let age = at.saturating_sub(last);
        let factor = match age {
            a if a < 3_600 => 4.0,
            a if a < 86_400 => 2.0,
            a if a < 604_800 => 0.5,
            _ => 0.25,
        };
        rank * factor
    }

    /// Pastas que casam com `query` (palavras, sem diferenciar maiúsculas; a última palavra precisa estar no
    /// nome da pasta), da mais para a menos frecente. Pastas que não existem mais ficam de fora.
    pub fn query(&self, query: &str, limit: usize) -> Vec<PathBuf> {
        self.query_at(query, limit, now(), |p| p.is_dir())
    }

    fn query_at(&self, query: &str, limit: usize, at: u64, exists: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
        let words: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
        let mut hits: Vec<(f64, &PathBuf)> = self
            .entries
            .iter()
            .filter(|(p, _)| {
                let full = p.to_string_lossy().to_lowercase();
                let last = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                words.iter().all(|w| full.contains(w.as_str())) && words.last().is_none_or(|w| last.contains(w.as_str()))
            })
            .map(|(p, &(rank, last))| (Self::score(rank, last, at), p))
            .collect();
        hits.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        hits.into_iter().map(|(_, p)| p).filter(|p| exists(p)).take(limit).cloned().collect()
    }

    pub fn to_text(&self) -> String {
        self.entries.iter().map(|(p, (rank, last))| format!("{rank:.3}\t{last}\t{}\n", p.display())).collect()
    }

    pub fn from_text(text: &str) -> Frecency {
        let entries = text
            .lines()
            .filter_map(|l| {
                let mut parts = l.splitn(3, '\t');
                let rank = parts.next()?.parse().ok()?;
                let last = parts.next()?.parse().ok()?;
                Some((PathBuf::from(parts.next()?), (rank, last)))
            })
            .collect();
        Frecency { entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mais_visitada_e_mais_recente_vem_primeiro() {
        let mut f = Frecency::default();
        let day = 86_400;
        let t = 100 * day;
        for _ in 0..5 {
            f.visit_at(Path::new("/home/a/projetos/arbor"), t - 30 * day);
        }
        f.visit_at(Path::new("/home/a/projetos/vislo"), t - 60);
        f.visit_at(Path::new("/home/a/Downloads"), t - 60);
        // arbor: 5 × 0,25 = 1,25; vislo: 1 × 4 = 4.
        let all = f.query_at("", 5, t, |_| true);
        assert_eq!(all.last(), Some(&PathBuf::from("/home/a/projetos/arbor")));
        assert_eq!(f.query_at("proj ar", 5, t, |_| true), [PathBuf::from("/home/a/projetos/arbor")]);
        // A última palavra precisa estar no nome da pasta: "a" casa com tudo, "down" só com Downloads.
        assert_eq!(f.query_at("home down", 5, t, |_| true), [PathBuf::from("/home/a/Downloads")]);
        assert!(f.query_at("arbor", 5, t, |_| false).is_empty());
        let back = Frecency::from_text(&f.to_text());
        assert_eq!(back.query_at("", 5, t, |_| true).len(), 3);
    }

    #[test]
    fn envelhece_e_esquece() {
        let mut f = Frecency::default();
        f.visit_at(Path::new("/raro"), 0);
        for i in 0..2_100 {
            f.visit_at(Path::new("/comum"), i);
        }
        assert!(f.query_at("raro", 5, 3_000, |_| true).is_empty());
        assert_eq!(f.query_at("comum", 5, 3_000, |_| true).len(), 1);
    }
}
