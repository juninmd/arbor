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
    once("arbor-shares", sort, move || crate::network::shares(&server).map(|names| crate::network::share_entries(&server, &names)))
}

/// Computadores da rede local ("Rede"), também numa thread.
pub fn list_network(sort: Sort) -> impl Stream<Item = Snapshot> {
    once("arbor-network", sort, || crate::discovery::computers().map(|names| crate::discovery::computer_entries(&names)))
}

/// Roda `fetch` numa thread e entrega um único snapshot completo, ordenado.
fn once(
    name: &str,
    sort: Sort,
    fetch: impl FnOnce() -> Result<Vec<arbor_core::Entry>, String> + Send + 'static,
) -> impl Stream<Item = Snapshot> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let start = std::time::Instant::now();
            let (mut entries, error) = match fetch() {
                Ok(entries) => (entries, None),
                Err(e) => (vec![], Some(e)),
            };
            arbor_core::sort_entries(&mut entries, sort);
            let _ = tx.unbounded_send(Snapshot { entries: std::sync::Arc::new(entries), complete: true, elapsed: start.elapsed(), error });
        })
        .expect("thread de rede");
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

/// Arquivos duplicados abaixo de `root`, numa thread própria: um snapshot só, no fim, com os grupos em
/// sequência (o que mais desperdiça espaço primeiro). Para se a aba sair da busca.
pub fn dupes(root: PathBuf, show_hidden: bool) -> impl Stream<Item = Snapshot> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("arbor-dupes".into())
        .spawn(move || {
            let start = std::time::Instant::now();
            let stop = || tx.is_closed();
            let Some(found) = arbor_core::dupes::find(&root, show_hidden, &stop) else { return };
            let error = if found.groups.is_empty() {
                Some("Nenhum arquivo duplicado".to_string())
            } else if found.truncated {
                Some(format!(
                    "Pasta grande demais: olhei só os primeiros {} arquivos",
                    arbor_core::human_count(arbor_core::dupes::MAX_FILES)
                ))
            } else {
                None
            };
            let entries: Vec<_> = found.groups.into_iter().flatten().collect();
            let _ = tx.unbounded_send(Snapshot { entries: std::sync::Arc::new(entries), complete: true, elapsed: start.elapsed(), error });
        })
        .expect("thread de duplicados");
    rx
}

/// Busca no índice do sistema (Everything, Spotlight, locate) numa thread própria.
pub fn global(query: String, sort: Sort) -> impl Stream<Item = Snapshot> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("arbor-global".into())
        .spawn(move || {
            let _ = tx.unbounded_send(crate::global_search::snapshot(&query, sort));
        })
        .expect("thread de busca global");
    rx
}

/// A cesta como listagem (rápido: só um `stat` por item guardado).
pub fn shelf(sort: Sort) -> impl Stream<Item = Snapshot> {
    let (tx, rx) = mpsc::unbounded();
    let _ = tx.unbounded_send(crate::shelf::snapshot(sort));
    rx
}
