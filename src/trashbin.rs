//! "Lixeira" como local navegável, como no Explorer: lista o que foi apagado, restaura, exclui de vez
//! e esvazia. Usa a lixeira do sistema (Windows e freedesktop); no macOS o crate `trash` não lista.

use arbor_core::entry::natural_key;
use arbor_core::{Entry, Ext};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

pub const TITLE: &str = "Lixeira";
const SENTINEL: &str = "arbor:trash";

pub fn path() -> PathBuf {
    PathBuf::from(SENTINEL)
}

pub fn is(p: &Path) -> bool {
    p.as_os_str() == SENTINEL
}

pub const SUPPORTED: bool = cfg!(any(windows, all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android"))));

fn entry(name: &str, original: PathBuf, is_dir: bool, size: u64, deleted: i64) -> Entry {
    Entry {
        name: name.to_string(),
        path: original,
        is_dir,
        is_link: false,
        hidden: false,
        size,
        modified: u64::try_from(deleted).ok().map(|s| UNIX_EPOCH + Duration::from_secs(s)),
        ext: if is_dir { Ext::default() } else { Path::new(name).extension().and_then(|e| e.to_str()).map(Ext::new).unwrap_or_default() },
        key: natural_key(name),
    }
}

#[cfg(any(windows, all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android"))))]
mod sys {
    use super::*;
    use trash::os_limited;
    use trash::{TrashItem, TrashItemSize};

    /// Itens da lixeira como entradas; `path` é o caminho original (a "data" é a da exclusão).
    pub fn list() -> Result<Vec<Entry>, String> {
        let items = os_limited::list().map_err(|e| e.to_string())?;
        Ok(items
            .iter()
            .map(|i| {
                let (is_dir, size) = match os_limited::metadata(i).map(|m| m.size) {
                    Ok(TrashItemSize::Entries(_)) => (true, 0),
                    Ok(TrashItemSize::Bytes(b)) => (false, b),
                    Err(_) => (false, 0),
                };
                entry(&i.name.to_string_lossy(), i.original_path(), is_dir, size, i.time_deleted)
            })
            .collect())
    }

    /// O item mais recente de cada caminho original pedido.
    fn pick(paths: &[PathBuf]) -> Result<Vec<TrashItem>, String> {
        let items = os_limited::list().map_err(|e| e.to_string())?;
        Ok(paths.iter().filter_map(|p| items.iter().filter(|i| i.original_path() == *p).max_by_key(|i| i.time_deleted).cloned()).collect())
    }

    pub fn restore(paths: &[PathBuf]) -> Result<usize, String> {
        let items = pick(paths)?;
        let n = items.len();
        os_limited::restore_all(items).map_err(|e| e.to_string())?;
        Ok(n)
    }

    pub fn purge(paths: &[PathBuf]) -> Result<usize, String> {
        let items = pick(paths)?;
        let n = items.len();
        os_limited::purge_all(items).map_err(|e| e.to_string())?;
        Ok(n)
    }

    pub fn empty() -> Result<usize, String> {
        let items = os_limited::list().map_err(|e| e.to_string())?;
        let n = items.len();
        os_limited::purge_all(items).map_err(|e| e.to_string())?;
        Ok(n)
    }
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))))]
mod sys {
    use super::*;
    const NO: &str = "a Lixeira ainda não é listada no macOS; use o Finder";
    pub fn list() -> Result<Vec<Entry>, String> {
        let _ = entry;
        Err(NO.into())
    }
    pub fn restore(_: &[PathBuf]) -> Result<usize, String> {
        Err(NO.into())
    }
    pub fn purge(_: &[PathBuf]) -> Result<usize, String> {
        Err(NO.into())
    }
    pub fn empty() -> Result<usize, String> {
        Err(NO.into())
    }
}

pub use sys::{empty, list, purge, restore};

/// Shift+Delete fora da Lixeira: apaga de vez, sem passar por ela (o diálogo de confirmação vem antes).
pub fn delete_now(paths: &[PathBuf]) -> Result<usize, String> {
    for p in paths {
        let meta = std::fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let r = if meta.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
        r.map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(paths.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinela_nao_colide_com_este_computador() {
        assert!(is(&path()) && !crate::computer::is(&path()) && !is(&crate::computer::path()));
    }

    #[test]
    fn entrada_usa_caminho_original_e_data_da_exclusao() {
        let e = entry("foto.PNG", PathBuf::from("/home/x/foto.PNG"), false, 10, 60);
        assert_eq!(e.path, PathBuf::from("/home/x/foto.PNG"));
        assert_eq!(e.ext.as_str(), "png");
        assert_eq!(e.modified, Some(UNIX_EPOCH + Duration::from_secs(60)));
        assert_eq!(entry("x", PathBuf::new(), true, 0, -5).modified, None, "data negativa não vira data falsa");
    }

    #[test]
    fn excluir_de_vez_apaga_arquivo_e_pasta_e_falha_no_inexistente() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("a.txt");
        let d = t.path().join("dir");
        std::fs::write(&f, "x").unwrap();
        std::fs::create_dir_all(d.join("sub")).unwrap();
        assert_eq!(delete_now(&[f.clone(), d.clone()]), Ok(2));
        assert!(!f.exists() && !d.exists());
        assert!(delete_now(&[f]).is_err());
    }
}
