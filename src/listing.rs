//! Ponte entre a listagem do `arbor-core` (uma thread que não bloqueia a interface) e o iced.

use arbor_core::{scan, Snapshot, Sort};
use iced::futures::channel::mpsc;
use iced::futures::Stream;
use std::path::PathBuf;

/// Lista `path` numa thread própria; o stream termina depois do snapshot completo. Se a aba mudar de pasta,
/// o receptor some e a thread para no próximo snapshot.
pub fn list(path: PathBuf, sort: Sort) -> impl Stream<Item = Snapshot> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("arbor-list".into())
        .spawn(move || scan(&path, sort, |snap| tx.unbounded_send(snap).is_ok()))
        .expect("thread de listagem");
    rx
}

/// Compartilhamentos de `\\servidor` numa thread: a rede pode demorar a responder.
pub fn list_shares(server: String, sort: Sort) -> impl Stream<Item = Snapshot> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("arbor-shares".into())
        .spawn(move || {
            let start = std::time::Instant::now();
            let (mut entries, error) = match crate::network::shares(&server) {
                Ok(names) => (crate::network::share_entries(&server, &names), None),
                Err(e) => (vec![], Some(e)),
            };
            arbor_core::sort_entries(&mut entries, sort);
            let _ = tx.unbounded_send(Snapshot { entries: std::sync::Arc::new(entries), complete: true, elapsed: start.elapsed(), error });
        })
        .expect("thread de compartilhamentos");
    rx
}

/// Lista o conteúdo de um compactado (`archive` + caminho interno) numa thread, como um snapshot completo.
pub fn list_archive(archive: PathBuf, inner: PathBuf, sort: Sort) -> impl Stream<Item = Snapshot> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("arbor-archive".into())
        .spawn(move || {
            let start = std::time::Instant::now();
            let (mut entries, error) = match arbor_core::archive::list(&archive, &inner) {
                Ok(e) => (e, None),
                Err(e) => (vec![], Some(format!("Não abriu {}: {e}", archive.display()))),
            };
            arbor_core::sort_entries(&mut entries, sort);
            let _ = tx.unbounded_send(Snapshot { entries: std::sync::Arc::new(entries), complete: true, elapsed: start.elapsed(), error });
        })
        .expect("thread de compactado");
    rx
}

/// Busca `query` em `root` e subpastas numa thread própria, com os mesmos snapshots da listagem.
pub fn search(root: PathBuf, query: String, sort: Sort, show_hidden: bool) -> impl Stream<Item = Snapshot> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("arbor-search".into())
        .spawn(move || arbor_core::search::search(&root, &query, sort, show_hidden, |snap| tx.unbounded_send(snap).is_ok()))
        .expect("thread de busca");
    rx
}
