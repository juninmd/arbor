//! Tamanho de pastas (a coluna "Tamanho" das pastas no Directory Opus, XYplorer e no Espaço do Total
//! Commander): soma recursiva em paralelo, sem seguir links e parando assim que `stop` liga.

use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Bytes de todos os arquivos abaixo de `dir`. Pastas sem permissão contam como vazias.
pub fn dir_size(dir: &Path, stop: &AtomicBool) -> u64 {
    if stop.load(Ordering::Relaxed) {
        return 0;
    }
    let Ok(read) = std::fs::read_dir(dir) else { return 0 };
    let mut files = 0u64;
    let mut subdirs: Vec<PathBuf> = Vec::new();
    for item in read.flatten() {
        let Ok(ft) = item.file_type() else { continue };
        if ft.is_dir() {
            subdirs.push(item.path());
        } else if ft.is_file() {
            files += item.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    files + subdirs.par_iter().map(|d| dir_size(d, stop)).sum::<u64>()
}

/// Tamanho de cada pasta de `dirs`, em paralelo. Devolve `None` se foi cancelado no meio.
pub fn sizes(dirs: &[PathBuf], stop: &AtomicBool) -> Option<Vec<(PathBuf, u64)>> {
    let out: Vec<(PathBuf, u64)> = dirs.par_iter().map(|d| (d.clone(), dir_size(d, stop))).collect();
    (!stop.load(Ordering::Relaxed)).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn soma_subpastas_e_respeita_o_cancelamento() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("a/b")).unwrap();
        fs::write(d.path().join("a/x"), [0u8; 10]).unwrap();
        fs::write(d.path().join("a/b/y"), [0u8; 5]).unwrap();
        let stop = AtomicBool::new(false);
        assert_eq!(dir_size(&d.path().join("a"), &stop), 15);
        assert_eq!(sizes(&[d.path().join("a")], &stop).unwrap(), [(d.path().join("a"), 15)]);
        stop.store(true, Ordering::Relaxed);
        assert!(sizes(&[d.path().join("a")], &stop).is_none());
    }
}
