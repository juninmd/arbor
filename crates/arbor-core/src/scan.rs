//! Listagem de pastas fora da thread de interface.
//!
//! Os nomes saem do disco num passe só (`getdents`/`FindNextFile`), os metadados são lidos em paralelo e a
//! lista já chega ordenada. A interface recebe *snapshots* imutáveis (`Arc`), nunca um item por mensagem:
//! o primeiro com até [`FIRST_BATCH`] itens assim que eles existem, depois, em discos lentos, um a cada
//! [`PROGRESS_EVERY`], e por fim a pasta completa.

use crate::entry::{sort_entries, Entry, Sort};
use rayon::prelude::*;
use std::fs::DirEntry;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Itens do primeiro quadro: cobre a tela mais alta com folga.
pub const FIRST_BATCH: usize = 256;
/// Em discos lentos (rede, USB), quanto esperar entre um snapshot parcial e o próximo.
pub const PROGRESS_EVERY: Duration = Duration::from_millis(120);
/// Metadados lidos por rodada paralela.
const CHUNK: usize = 8_192;
/// Abaixo disso a leitura é sequencial: distribuir entre threads custaria mais que o `stat`.
const PARALLEL_FROM: usize = 512;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub entries: Arc<Vec<Entry>>,
    /// `false` enquanto a pasta ainda está sendo lida.
    pub complete: bool,
    /// Tempo desde o início da listagem até este snapshot (inclui ordenar).
    pub elapsed: Duration,
    pub error: Option<String>,
}

/// Lê `path` e chama `emit` com snapshots ordenados por `sort`; para quando `emit` devolve `false`
/// (a aba mudou de pasta ou fechou). O último snapshot sempre tem `complete == true`, inclusive em erro.
pub fn scan(path: &Path, sort: Sort, mut emit: impl FnMut(Snapshot) -> bool) {
    let start = Instant::now();
    let finish = |entries: Vec<Entry>, error: Option<String>, emit: &mut dyn FnMut(Snapshot) -> bool| {
        emit(Snapshot { entries: Arc::new(entries), complete: true, elapsed: start.elapsed(), error });
    };
    let dir = match std::fs::read_dir(path) {
        Ok(d) => d,
        Err(e) => return finish(Vec::new(), Some(e.to_string()), &mut emit),
    };

    let mut pending: Vec<DirEntry> = Vec::with_capacity(FIRST_BATCH);
    let mut done: Vec<Entry> = Vec::new();
    let mut first_sent = false;
    let mut last_emit = start;
    // Cada snapshot parcial copia a lista; o intervalo cresce com o custo dessa cópia para que pastas enormes
    // em discos lentos não gastem mais tempo mostrando progresso do que lendo.
    let mut every = PROGRESS_EVERY;

    for item in dir.flatten() {
        pending.push(item);
        let first_due = !first_sent && pending.len() + done.len() >= FIRST_BATCH;
        if first_due || pending.len() >= CHUNK {
            stat_all(&mut pending, &mut done);
            let progress_due = last_emit.elapsed() >= every;
            if first_due || progress_due {
                first_sent = true;
                let cost = Instant::now();
                sort_entries(&mut done, sort);
                let partial = Snapshot { entries: Arc::new(done.clone()), complete: false, elapsed: start.elapsed(), error: None };
                if !emit(partial) {
                    return;
                }
                every = every.max(cost.elapsed() * 8);
                last_emit = Instant::now();
            }
        }
    }
    stat_all(&mut pending, &mut done);
    sort_entries(&mut done, sort);
    finish(done, None, &mut emit);
}

/// Lista tudo de uma vez e devolve só o snapshot final (benchmarks e testes).
pub fn scan_all(path: &Path, sort: Sort) -> Snapshot {
    let mut last = None;
    scan(path, sort, |s| {
        if s.complete {
            last = Some(s);
        }
        true
    });
    last.expect("scan sempre termina com um snapshot completo")
}

fn stat_all(pending: &mut Vec<DirEntry>, done: &mut Vec<Entry>) {
    if pending.len() >= PARALLEL_FROM {
        let mut batch: Vec<Entry> = pending.par_drain(..).filter_map(stat).collect();
        done.append(&mut batch);
    } else {
        done.extend(pending.drain(..).filter_map(stat));
    }
}

/// No Windows os metadados já vêm da enumeração; em Unix é um `fstatat` relativo ao diretório aberto.
fn stat(item: DirEntry) -> Option<Entry> {
    let name = item.file_name().to_string_lossy().into_owned();
    let is_link = item.file_type().map(|t| t.is_symlink()).unwrap_or(false);
    let path = item.path();
    let meta = if is_link { std::fs::metadata(&path).or_else(|_| item.metadata()) } else { item.metadata() };
    let meta = meta.ok()?;
    Some(Entry::new(name, path, &meta, is_link))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primeiro_snapshot_sai_cedo_e_o_ultimo_esta_completo() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..1_000 {
            std::fs::write(dir.path().join(format!("f{i}")), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let mut snaps = vec![];
        scan(dir.path(), Sort::default(), |s| {
            snaps.push(s);
            true
        });
        let first = &snaps[0];
        assert!(!first.complete);
        assert_eq!(first.entries.len(), FIRST_BATCH);
        let last = snaps.last().unwrap();
        assert!(last.complete);
        assert_eq!(last.entries.len(), 1_001);
        assert_eq!(last.entries[0].name, "sub", "pastas primeiro");
        assert_eq!(last.entries[1].name, "f0");
        assert_eq!(last.entries[3].name, "f2", "ordem natural");
    }

    #[test]
    fn para_quando_pedido() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..600 {
            std::fs::write(dir.path().join(format!("f{i}")), b"").unwrap();
        }
        let mut calls = 0;
        scan(dir.path(), Sort::default(), |_| {
            calls += 1;
            false
        });
        assert_eq!(calls, 1);
    }

    #[test]
    fn erro_vira_snapshot_completo() {
        let s = scan_all(Path::new("/caminho/que/nao/existe/arbor"), Sort::default());
        assert!(s.complete && s.error.is_some() && s.entries.is_empty());
    }
}
