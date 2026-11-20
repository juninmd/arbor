//! Tamanho de pastas (a coluna "Tamanho" das pastas no Directory Opus, XYplorer e no Espaço do Total
//! Commander): soma recursiva em paralelo, sem seguir links e parando assim que `stop` liga.

use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Soma de uma pasta: `complete` é falso quando algo ficou de fora (sem permissão, erro de leitura).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub bytes: u64,
    pub complete: bool,
}

#[cfg(unix)]
fn device(m: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(m.dev())
}

#[cfg(not(unix))]
fn device(_: &std::fs::Metadata) -> Option<u64> {
    None
}

/// Bytes de todos os arquivos abaixo de `dir`, sem seguir links nem entrar em outro dispositivo
/// (autofs, NFS, FUSE e discos montados dentro da pasta ficam de fora, como no `du -x`).
pub fn dir_size(dir: &Path, stop: &AtomicBool) -> Size {
    let root = std::fs::metadata(dir).ok().and_then(|m| device(&m));
    walk(dir, root, stop)
}

fn walk(dir: &Path, root: Option<u64>, stop: &AtomicBool) -> Size {
    if stop.load(Ordering::Relaxed) {
        return Size::default();
    }
    let Ok(read) = std::fs::read_dir(dir) else { return Size { bytes: 0, complete: false } };
    let mut total = Size { bytes: 0, complete: true };
    let mut subdirs: Vec<PathBuf> = Vec::new();
    for item in read {
        let Ok(item) = item else {
            total.complete = false;
            continue;
        };
        let Ok(ft) = item.file_type() else {
            total.complete = false;
            continue;
        };
        if ft.is_dir() {
            // Outro dispositivo: não entra (pode ser rede lenta ou montagem sob demanda).
            match (root, item.metadata().ok().and_then(|m| device(&m))) {
                (Some(r), Some(d)) if r != d => {}
                _ => subdirs.push(item.path()),
            }
        } else if ft.is_file() {
            match item.metadata() {
                Ok(m) => total.bytes += m.len(),
                Err(_) => total.complete = false,
            }
        }
    }
    for s in subdirs.par_iter().map(|d| walk(d, root, stop)).collect::<Vec<_>>() {
        total.bytes += s.bytes;
        total.complete &= s.complete;
    }
    total
}

/// Tamanho de cada pasta de `dirs`, em paralelo. Devolve `None` se foi cancelado no meio.
pub fn sizes(dirs: &[PathBuf], stop: &AtomicBool) -> Option<Vec<(PathBuf, Size)>> {
    let out: Vec<(PathBuf, Size)> = dirs.par_iter().map(|d| (d.clone(), dir_size(d, stop))).collect();
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
        assert_eq!(dir_size(&d.path().join("a"), &stop), Size { bytes: 15, complete: true });
        assert_eq!(sizes(&[d.path().join("a")], &stop).unwrap(), [(d.path().join("a"), Size { bytes: 15, complete: true })]);
        stop.store(true, Ordering::Relaxed);
        assert!(sizes(&[d.path().join("a")], &stop).is_none());
    }

    #[test]
    fn pasta_ilegivel_vira_desconhecida() {
        let d = tempfile::tempdir().unwrap();
        let stop = AtomicBool::new(false);
        assert_eq!(dir_size(&d.path().join("sumiu"), &stop), Size { bytes: 0, complete: false });
    }
}
