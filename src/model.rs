//! Entradas de diretório, listagem em segundo plano e ordenação.

use iced::futures::channel::mpsc;
use iced::futures::Stream;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_link: bool,
    pub hidden: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// Extensão em minúsculas, sem o ponto.
    pub ext: String,
}

impl Entry {
    fn build(name: String, path: PathBuf, meta: &std::fs::Metadata, is_link: bool) -> Entry {
        let is_dir = meta.is_dir();
        let ext = if is_dir {
            String::new()
        } else {
            Path::new(&name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
        };
        Entry {
            hidden: is_hidden(&name, meta),
            size: if is_dir { 0 } else { meta.len() },
            modified: meta.modified().ok(),
            name,
            path,
            is_dir,
            is_link,
            ext,
        }
    }
}

#[cfg(windows)]
fn is_hidden(name: &str, meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    name.starts_with('.') || meta.file_attributes() & HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &str, _meta: &std::fs::Metadata) -> bool {
    name.starts_with('.')
}

#[derive(Debug, Clone)]
pub enum ListEvent {
    Batch(Vec<Entry>),
    Done(Result<(), String>),
}

/// Primeiro lote pequeno para a pasta aparecer na hora; os seguintes são maiores.
const FIRST_BATCH: usize = 256;
const BATCH: usize = 4096;

/// Lê a pasta numa thread própria e entrega lotes. A interface nunca espera pelo disco.
pub fn list(path: PathBuf) -> impl Stream<Item = ListEvent> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("arbor-list".into())
        .spawn(move || {
            let result = read_into(&path, |batch| tx.unbounded_send(ListEvent::Batch(batch)).is_ok());
            let _ = tx.unbounded_send(ListEvent::Done(result));
        })
        .expect("thread de listagem");
    rx
}

/// Lê `path` chamando `emit` com lotes; para quando `emit` devolve `false` (a aba fechou ou mudou).
pub fn read_into(path: &Path, mut emit: impl FnMut(Vec<Entry>) -> bool) -> Result<(), String> {
    let dir = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut batch = Vec::with_capacity(FIRST_BATCH);
    let mut limit = FIRST_BATCH;
    let mut last = Instant::now();
    for item in dir.flatten() {
        let name = item.file_name().to_string_lossy().into_owned();
        let is_link = item.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        let meta = if is_link { std::fs::metadata(item.path()) } else { item.metadata() };
        let Ok(meta) = meta.or_else(|_| item.metadata()) else { continue };
        batch.push(Entry::build(name, item.path(), &meta, is_link));
        if batch.len() >= limit || (last.elapsed().as_millis() > 80 && !batch.is_empty()) {
            if !emit(std::mem::take(&mut batch)) {
                return Ok(());
            }
            limit = BATCH;
            last = Instant::now();
        }
    }
    if !batch.is_empty() {
        emit(batch);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
    Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub key: SortKey,
    pub desc: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Sort { key: SortKey::Name, desc: false }
    }
}

/// Pastas antes de arquivos; nomes em ordem natural ("2" antes de "10"), sem diferenciar maiúsculas.
pub fn compare(a: &Entry, b: &Entry, sort: Sort) -> Ordering {
    b.is_dir.cmp(&a.is_dir).then_with(|| {
        let by_name = || natord::compare_ignore_case(&a.name, &b.name);
        let ord = match sort.key {
            SortKey::Name => by_name(),
            SortKey::Size => a.size.cmp(&b.size).then_with(by_name),
            SortKey::Modified => a.modified.cmp(&b.modified).then_with(by_name),
            SortKey::Kind => a.ext.cmp(&b.ext).then_with(by_name),
        };
        if sort.desc {
            ord.reverse()
        } else {
            ord
        }
    })
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v < 10.0 {
        format!("{v:.1} {}", UNITS[i])
    } else {
        format!("{v:.0} {}", UNITS[i])
    }
}

pub fn human_time(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return String::new() };
    let dt: chrono::DateTime<chrono::Local> = t.into();
    dt.format("%d/%m/%Y %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, dir: bool, size: u64) -> Entry {
        Entry { name: name.into(), path: name.into(), is_dir: dir, is_link: false, hidden: false, size, modified: None, ext: String::new() }
    }

    #[test]
    fn pastas_primeiro_e_ordem_natural() {
        let mut v = [e("b10", false, 1), e("B2", false, 1), e("z", true, 0), e("a", false, 1)];
        v.sort_by(|a, b| compare(a, b, Sort::default()));
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["z", "a", "B2", "b10"]);
    }

    #[test]
    fn ordena_por_tamanho_desc() {
        let mut v = [e("a", false, 1), e("b", false, 3), e("c", false, 2)];
        v.sort_by(|a, b| compare(a, b, Sort { key: SortKey::Size, desc: true }));
        assert_eq!(v[0].name, "b");
    }

    #[test]
    fn lista_em_lotes() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..600 {
            std::fs::write(dir.path().join(format!("f{i}")), b"x").unwrap();
        }
        let mut batches = 0;
        let mut total = 0;
        read_into(dir.path(), |b| {
            batches += 1;
            total += b.len();
            true
        })
        .unwrap();
        assert_eq!(total, 600);
        assert!(batches >= 2, "o primeiro lote deve sair cedo");
    }

    #[test]
    fn tamanhos_legiveis() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(50 * 1024 * 1024), "50 MB");
    }
}
