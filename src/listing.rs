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
