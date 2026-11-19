//! "Início" como no Explorer: Acesso rápido (pastas fixadas e as mais visitadas) e Recentes (arquivos
//! abertos pelo arbor). Os recentes ficam em `~/.config/arbor/recent`, um caminho por linha.

use arbor_core::Entry;
use std::path::{Path, PathBuf};

pub const TITLE: &str = "Início";
const SENTINEL: &str = "arbor:home";
const MAX_RECENT: usize = 30;
/// Pastas frequentes mostradas além das fixadas.
pub const FREQUENT: usize = 8;

pub fn path() -> PathBuf {
    PathBuf::from(SENTINEL)
}

pub fn is(p: &Path) -> bool {
    p.as_os_str() == SENTINEL
}

/// Põe `p` no topo dos recentes, sem repetir e com limite.
pub fn record(recent: &mut Vec<PathBuf>, p: &Path) {
    recent.retain(|r| r != p);
    recent.insert(0, p.to_path_buf());
    recent.truncate(MAX_RECENT);
}

/// Pastas do Acesso rápido (sem repetir) e depois os arquivos recentes que ainda existem.
pub fn entries(quick: &[PathBuf], recent: &[PathBuf]) -> Vec<Entry> {
    let mut seen = std::collections::HashSet::new();
    let dirs = quick.iter().filter(|p| seen.insert(p.as_path())).filter_map(|p| entry(p).filter(|e| e.is_dir));
    let files = recent.iter().filter_map(|p| entry(p).filter(|e| !e.is_dir));
    dirs.chain(files).collect()
}

fn entry(p: &Path) -> Option<Entry> {
    let meta = std::fs::metadata(p).ok()?;
    let name = p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned());
    Some(Entry::new(name, p.to_path_buf(), &meta, false))
}

fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor").join("recent"))
}

pub fn load() -> Vec<PathBuf> {
    let text = file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    text.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from).take(MAX_RECENT).collect()
}

/// Grava fora da interface; perder a lista de recentes não é motivo para avisar o usuário.
pub fn save(recent: &[PathBuf]) {
    let Some(file) = file() else { return };
    let text: String = recent.iter().map(|p| format!("{}\n", p.display())).collect();
    std::thread::spawn(move || {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = file.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &file);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_moves_to_top_without_duplicates_and_caps() {
        let mut r = vec![];
        for i in 0..40 {
            record(&mut r, Path::new(&format!("/f{i}")));
        }
        record(&mut r, Path::new("/f35"));
        assert_eq!(r.len(), MAX_RECENT);
        assert_eq!(r[0], PathBuf::from("/f35"));
        assert_eq!(r.iter().filter(|p| p.as_path() == Path::new("/f35")).count(), 1);
    }

    #[test]
    fn folders_first_then_existing_recent_files() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("fotos");
        std::fs::create_dir(&sub).unwrap();
        let f = dir.path().join("nota.txt");
        std::fs::write(&f, "x").unwrap();
        let gone = dir.path().join("sumiu.txt");
        let e = entries(&[sub.clone(), sub.clone(), f.clone()], &[gone, f.clone(), sub.clone()]);
        let names: Vec<_> = e.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["fotos", "nota.txt"], "pasta uma vez, arquivo apagado some, arquivo no quick e pasta no recente ignorados");
    }
}
