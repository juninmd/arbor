//! Busca no computador inteiro usando o índice que o sistema já mantém, sem construir um próprio (a fase 4 do
//! roteiro sem o custo): Everything (`es.exe`) no Windows, como fazem Directory Opus, Files e XYplorer;
//! Spotlight (`mdfind`) no macOS; `plocate`/`locate` no Linux. A consulta vai como argumento, nunca por shell.

use arbor_core::{sort_entries, Entry, Snapshot, Sort};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

/// Prefixo da consulta guardada na aba de resultados (como `search::FLAT`).
pub const PREFIX: &str = "\u{1}global:";
pub const MAX: usize = 2000;

pub fn query_of(search: &str) -> Option<&str> {
    search.strip_prefix(PREFIX)
}

/// Programa e argumentos da busca nesta plataforma, na ordem de preferência.
fn commands(query: &str) -> Vec<(&'static str, Vec<String>)> {
    let q = query.to_string();
    let max = MAX.to_string();
    if cfg!(windows) {
        vec![("es", vec!["-n".into(), max, q])]
    } else if cfg!(target_os = "macos") {
        vec![("mdfind", vec!["-name".into(), q])]
    } else {
        vec![("plocate", vec!["-i".into(), "-l".into(), max.clone(), q.clone()]), ("locate", vec!["-i".into(), "-l".into(), max, q])]
    }
}

/// Uma linha por caminho absoluto; o resto (avisos, linhas vazias) é ignorado.
fn parse(out: &str) -> Vec<PathBuf> {
    out.lines().map(str::trim).filter(|l| !l.is_empty()).map(PathBuf::from).filter(|p| p.is_absolute()).take(MAX).collect()
}

fn run(query: &str) -> Result<Vec<PathBuf>, String> {
    for (program, args) in commands(query) {
        match Command::new(program).args(&args).output() {
            Ok(out) if out.status.success() || !out.stdout.is_empty() => return Ok(parse(&String::from_utf8_lossy(&out.stdout))),
            Ok(_) => return Ok(vec![]),
            Err(_) => continue,
        }
    }
    Err(if cfg!(windows) {
        "Busca no computador usa o Everything: instale-o com o es.exe no PATH".into()
    } else {
        "Busca no computador usa o plocate/locate: instale-o e rode updatedb".into()
    })
}

fn entry(path: &Path) -> Option<Entry> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    Some(Entry::new(name, path.to_path_buf(), &meta, meta.file_type().is_symlink()))
}

pub fn snapshot(query: &str, sort: Sort) -> Snapshot {
    let start = Instant::now();
    let (mut entries, error) = match run(query) {
        Ok(paths) => {
            let n = paths.len();
            let e: Vec<Entry> = paths.iter().filter_map(|p| entry(p)).collect();
            let note = (n >= MAX).then(|| format!("Mostrando os primeiros {MAX} resultados; refine a busca"));
            (e, note.or_else(|| (n == 0).then(|| "Nada encontrado no índice do sistema".to_string())))
        }
        Err(e) => (vec![], Some(e)),
    };
    sort_entries(&mut entries, sort);
    Snapshot { entries: Arc::new(entries), complete: true, elapsed: start.elapsed(), error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_caminhos_absolutos_e_consulta_vai_como_argumento() {
        let abs = if cfg!(windows) { "C:\\a\\b.txt" } else { "/a/b.txt" };
        assert_eq!(parse(&format!("{abs}\n\nrelativo.txt\n  \n")), [PathBuf::from(abs)]);
        let q = "foo; rm -rf ~";
        assert!(commands(q).iter().all(|(_, args)| args.last().map(String::as_str) == Some(q)));
        assert_eq!(query_of(&format!("{PREFIX}relatório")), Some("relatório"));
    }
}
