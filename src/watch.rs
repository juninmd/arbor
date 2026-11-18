//! Atualiza a pasta sozinha quando algo muda no disco (o F5 que não atualiza é uma queixa comum).

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use notify::{RecursiveMode, Watcher};
use std::path::PathBuf;
use std::time::Duration;

#[allow(clippy::ptr_arg)] // a assinatura é a que `Subscription::run_with` exige
pub fn watch(path: &PathBuf) -> impl Stream<Item = PathBuf> + use<> {
    let path = path.clone();
    iced::stream::channel(4, async move |mut out: mpsc::Sender<PathBuf>| {
        let (tx, mut rx) = mpsc::unbounded::<()>();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if res.is_ok_and(|e| !e.kind.is_access()) {
                let _ = tx.unbounded_send(());
            }
        });
        let Ok(mut watcher) = watcher else { return };
        if watcher.watch(&path, RecursiveMode::NonRecursive).is_err() {
            return;
        }
        while rx.next().await.is_some() {
            // Junta rajadas de eventos (uma cópia grande gera milhares) numa atualização só.
            tokio::time::sleep(Duration::from_millis(250)).await;
            while rx.try_recv().is_ok() {}
            if out.send(path.clone()).await.is_err() {
                break;
            }
        }
        drop(watcher);
    })
}
