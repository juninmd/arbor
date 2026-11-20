//! Cesta (Shelf do Files, File Collections do Directory Opus, Catalog do XYplorer): junta arquivos de várias
//! pastas para depois copiar, mover ou compactar tudo de uma vez. Guarda só caminhos, na memória; a aba
//! "Cesta" lista o que ainda existe, e as operações comuns agem sobre os arquivos de verdade.

use arbor_core::{sort_entries, Entry, Snapshot, Sort};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Consulta especial da aba (como `search::FLAT`).
pub const QUERY: &str = "\u{1}cesta";

static ITEMS: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());

/// Põe na cesta sem repetir; devolve quantos entraram.
pub fn add(paths: impl IntoIterator<Item = PathBuf>) -> usize {
    let Ok(mut items) = ITEMS.write() else { return 0 };
    let before = items.len();
    for p in paths {
        if !items.contains(&p) {
            items.push(p);
        }
    }
    items.len() - before
}

pub fn remove(paths: &[PathBuf]) {
    if let Ok(mut items) = ITEMS.write() {
        items.retain(|p| !paths.contains(p));
    }
}

pub fn clear() {
    if let Ok(mut items) = ITEMS.write() {
        items.clear();
    }
}

pub fn len() -> usize {
    ITEMS.read().map(|i| i.len()).unwrap_or(0)
}

/// O que está na cesta e ainda existe, como uma listagem comum.
pub fn snapshot(sort: Sort) -> Snapshot {
    let start = Instant::now();
    let paths = ITEMS.read().map(|i| i.clone()).unwrap_or_default();
    let mut entries: Vec<Entry> = paths
        .iter()
        .filter_map(|p| {
            let meta = std::fs::symlink_metadata(p).ok()?;
            Some(Entry::new(p.file_name()?.to_string_lossy().into_owned(), p.clone(), &meta, meta.file_type().is_symlink()))
        })
        .collect();
    sort_entries(&mut entries, sort);
    let error = entries.is_empty().then(|| "Cesta vazia: selecione arquivos e use \"Pôr na cesta\"".to_string());
    Snapshot { entries: Arc::new(entries), complete: true, elapsed: start.elapsed(), error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cesta_nao_repete_e_some_com_o_que_foi_apagado() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        clear();
        assert_eq!(add([a.clone(), b.clone(), a.clone()]), 2);
        assert_eq!(add([a.clone()]), 0);
        std::fs::remove_file(&b).unwrap();
        let names: Vec<_> = snapshot(Sort::default()).entries.iter().map(|e| e.name.clone()).collect();
        assert_eq!(names, ["a"]);
        remove(std::slice::from_ref(&a));
        assert_eq!(len(), 1);
        clear();
        assert_eq!(len(), 0);
    }
}
