//! Busca recursiva por nome: percorre a pasta e todas as subpastas em paralelo e transmite os resultados.
//!
//! Usa o mesmo [`Snapshot`] da listagem, então a interface mostra resultados como mostra uma pasta: o
//! primeiro lote sai assim que existe, depois um snapshot a cada [`PROGRESS_EVERY`], e por fim o completo.
//!
//! A consulta casa com o nome, sem diferenciar maiúsculas: palavras separadas precisam aparecer todas
//! (`relat 2025`); com `*` ou `?` vira um curinga sobre o nome inteiro (`*.pdf`, `IMG_????.jpg`).

use crate::entry::{sort_entries, Entry, Sort};
use crate::scan::{Snapshot, PROGRESS_EVERY};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

/// Mais que isso não ajuda ninguém a achar um arquivo e só pesa na interface.
pub const MAX_RESULTS: usize = 20_000;

/// Como a consulta casa com um nome.
#[derive(Debug, Clone)]
pub enum Query {
    Words(Vec<String>),
    Glob(Vec<char>),
}

impl Query {
    pub fn parse(q: &str) -> Option<Query> {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return None;
        }
        Some(if q.contains(['*', '?']) {
            Query::Glob(q.chars().collect())
        } else {
            Query::Words(q.split_whitespace().map(String::from).collect())
        })
    }

    pub fn matches(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        match self {
            Query::Words(words) => words.iter().all(|w| name.contains(w.as_str())),
            Query::Glob(pattern) => glob(pattern, &name.chars().collect::<Vec<_>>()),
        }
    }
}

/// Curinga clássico (`*` qualquer sequência, `?` um caractere), sem recursão exponencial.
fn glob(p: &[char], s: &[char]) -> bool {
    let (mut pi, mut si) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(st) = star {
            pi = st + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Busca `query` dentro de `root` e chama `emit` com snapshots ordenados; para quando `emit` devolve `false`.
/// O último snapshot sempre tem `complete == true`; `error` avisa se a lista foi cortada em [`MAX_RESULTS`].
pub fn search(root: &Path, query: &str, sort: Sort, show_hidden: bool, mut emit: impl FnMut(Snapshot) -> bool) {
    let start = Instant::now();
    let Some(query) = Query::parse(query) else {
        emit(Snapshot { entries: Arc::default(), complete: true, elapsed: start.elapsed(), error: None });
        return;
    };
    let stop = Arc::new(AtomicBool::new(false));
    let found = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel::<Entry>();
    let walker = {
        let (root, stop, found) = (root.to_path_buf(), stop.clone(), found.clone());
        std::thread::Builder::new()
            .name("arbor-search".into())
            .spawn(move || {
                let ctx = Walk { query: &query, show_hidden, stop: &stop, found: &found, tx };
                rayon::scope(|s| walk_dir(s, &ctx, &root));
            })
            .expect("thread de busca")
    };

    let mut results: Vec<Entry> = Vec::new();
    let mut last_emit = start;
    let mut sent_any = false;
    loop {
        match rx.recv_timeout(PROGRESS_EVERY) {
            Ok(e) => results.push(e),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let due = last_emit.elapsed() >= PROGRESS_EVERY || (!sent_any && results.len() >= crate::scan::FIRST_BATCH);
        if due {
            sort_entries(&mut results, sort);
            let partial = Snapshot { entries: Arc::new(results.clone()), complete: false, elapsed: start.elapsed(), error: None };
            if !emit(partial) {
                stop.store(true, Ordering::Relaxed);
                let _ = walker.join();
                return;
            }
            sent_any = true;
            last_emit = Instant::now();
        }
    }
    let _ = walker.join();
    results.extend(rx.try_iter());
    results.truncate(MAX_RESULTS);
    sort_entries(&mut results, sort);
    let error = (found.load(Ordering::Relaxed) >= MAX_RESULTS)
        .then(|| format!("Mostrando os primeiros {} resultados; refine a busca", crate::human_count(MAX_RESULTS)));
    emit(Snapshot { entries: Arc::new(results), complete: true, elapsed: start.elapsed(), error });
}

struct Walk<'a> {
    query: &'a Query,
    show_hidden: bool,
    stop: &'a AtomicBool,
    found: &'a AtomicUsize,
    tx: mpsc::Sender<Entry>,
}

fn walk_dir<'s>(scope: &rayon::Scope<'s>, ctx: &'s Walk<'s>, dir: &Path) {
    if ctx.stop.load(Ordering::Relaxed) {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for item in read.flatten() {
        if ctx.stop.load(Ordering::Relaxed) {
            return;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        let Ok(ft) = item.file_type() else { continue };
        let hidden = name.starts_with('.');
        if hidden && !ctx.show_hidden {
            continue;
        }
        // Links para pastas não são seguidos: evita voltas infinitas e sair da pasta buscada.
        if ft.is_dir() {
            let path = item.path();
            scope.spawn(move |s| walk_dir(s, ctx, &path));
        }
        if ctx.query.matches(&name) {
            let Ok(meta) = item.metadata() else { continue };
            if ctx.found.fetch_add(1, Ordering::Relaxed) >= MAX_RESULTS {
                ctx.stop.store(true, Ordering::Relaxed);
                return;
            }
            let _ = ctx.tx.send(Entry::new(name, item.path(), &meta, ft.is_symlink()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn curinga_e_palavras() {
        let q = Query::parse("*.PDF").unwrap();
        assert!(q.matches("Relatório.pdf") && !q.matches("a.pdf.txt"));
        assert!(Query::parse("img_????.jpg").unwrap().matches("IMG_0042.JPG"));
        let w = Query::parse("relat 2025").unwrap();
        assert!(w.matches("Relatorio-2025.xlsx") && !w.matches("relatorio-2024"));
        assert!(Query::parse("   ").is_none());
    }

    #[test]
    fn acha_em_subpastas_e_pula_ocultos() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("a/b/c")).unwrap();
        fs::create_dir_all(d.path().join(".git/objects")).unwrap();
        fs::write(d.path().join("a/b/c/nota.txt"), "").unwrap();
        fs::write(d.path().join("a/nota2.TXT"), "").unwrap();
        fs::write(d.path().join(".git/objects/nota.txt"), "").unwrap();
        fs::write(d.path().join("outra.md"), "").unwrap();
        let mut last = None;
        search(d.path(), "nota", Sort::default(), false, |s| {
            if s.complete {
                last = Some(s);
            }
            true
        });
        let s = last.unwrap();
        let mut names: Vec<_> = s.entries.iter().map(|e| e.name.clone()).collect();
        names.sort();
        assert_eq!(names, ["nota.txt", "nota2.TXT"]);
        let mut all = 0;
        search(d.path(), "nota", Sort::default(), true, |s| {
            all = s.entries.len();
            true
        });
        assert_eq!(all, 3);
    }
}
