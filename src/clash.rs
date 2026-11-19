//! Comparação do diálogo de conflito, como o "Comparar informações" do Explorer: tamanho e data dos dois
//! lados, e a regra "Só os mais novos" (substitui quando a origem é mais recente, pula o resto).

use arbor_core::{human_size, human_time};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub struct Side {
    /// `None` para pastas: o tamanho delas não diz nada sem percorrer tudo.
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct Clash {
    pub name: String,
    pub source: Side,
    pub dest: Side,
}

fn side(p: &Path) -> Side {
    let meta = std::fs::metadata(p).ok();
    Side { size: meta.as_ref().filter(|m| !m.is_dir()).map(|m| m.len()), modified: meta.and_then(|m| m.modified().ok()) }
}

/// Um `Clash` por fonte cujo nome já existe em `dest`.
pub fn compare(sources: &[PathBuf], dest: &Path, names: &[String]) -> Vec<Clash> {
    sources
        .iter()
        .filter_map(|s| {
            let name = s.file_name()?.to_string_lossy().into_owned();
            names.contains(&name).then(|| Clash { source: side(s), dest: side(&dest.join(&name)), name })
        })
        .collect()
}

impl Clash {
    pub fn source_newer(&self) -> bool {
        matches!((self.source.modified, self.dest.modified), (Some(a), Some(b)) if a > b)
    }

    pub fn same(&self) -> bool {
        self.source == self.dest
    }
}

/// "2,0 KB · 19/11/2026 10:00", com a marca de mais novo quando cabe.
pub fn describe(s: &Side, newer: bool) -> String {
    let mut parts: Vec<String> = s.size.map(human_size).into_iter().collect();
    if s.modified.is_some() {
        parts.push(human_time(s.modified));
    }
    if newer {
        parts.push("mais novo".into());
    }
    parts.join(" · ")
}

/// Fontes a manter com "Só os mais novos": as sem conflito e as mais recentes que o destino.
pub fn newer_sources(sources: &[PathBuf], clashes: &[Clash]) -> Vec<PathBuf> {
    sources
        .iter()
        .filter(|s| {
            let name = s.file_name().map(|n| n.to_string_lossy().into_owned());
            clashes.iter().find(|c| Some(&c.name) == name.as_ref()).is_none_or(Clash::source_newer)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn touch(p: &Path, body: &str, age_secs: u64) {
        std::fs::write(p, body).unwrap();
        let t = SystemTime::now() - Duration::from_secs(age_secs);
        std::fs::File::options().write(true).open(p).unwrap().set_modified(t).unwrap();
    }

    #[test]
    fn keeps_only_sources_newer_than_what_they_would_replace() {
        let (src, dst) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (new, old, free) = (src.path().join("novo.txt"), src.path().join("velho.txt"), src.path().join("livre.txt"));
        touch(&new, "abc", 10);
        touch(&dst.path().join("novo.txt"), "a", 1000);
        touch(&old, "abc", 1000);
        touch(&dst.path().join("velho.txt"), "abcd", 10);
        touch(&free, "x", 0);
        let sources = vec![new.clone(), old, free.clone()];
        let clashes = compare(&sources, dst.path(), &arbor_core::ops::conflicts(&sources, dst.path()));
        assert_eq!(clashes.len(), 2);
        assert!(clashes[0].source_newer() && !clashes[1].source_newer());
        assert_eq!(clashes[0].source.size, Some(3));
        assert_eq!(newer_sources(&sources, &clashes), [new, free]);
    }

    #[test]
    fn describe_omits_folder_size_and_marks_the_newer_side() {
        let s = Side { size: None, modified: None };
        assert_eq!(describe(&s, false), "");
        let f = Side { size: Some(2048), modified: None };
        assert_eq!(describe(&f, true), "2.0 KB · mais novo");
    }
}
