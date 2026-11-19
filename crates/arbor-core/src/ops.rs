//! Operações de arquivo em segundo plano (copiar, mover, lixeira, renomear, nova pasta) e o desfazer.
//!
//! A cópia é feita num arquivo temporário `.nome.arbor-part`, conferida pelo tamanho e só então renomeada
//! para o nome final: uma falha ou cancelamento nunca deixa um arquivo pela metade com o nome certo.

use futures_channel::mpsc;
use futures_core::Stream;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Op {
    Copy {
        sources: Vec<PathBuf>,
        dest: PathBuf,
    },
    Move {
        sources: Vec<PathBuf>,
        dest: PathBuf,
    },
    Trash {
        paths: Vec<PathBuf>,
    },
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    Mkdir {
        path: PathBuf,
    },
    /// Extrai membros de um compactado (`members` internos; vazio = tudo) para dentro de `dest`.
    Extract {
        archive: PathBuf,
        members: Vec<PathBuf>,
        dest: PathBuf,
    },
    /// Cria o zip `dest` com `sources`.
    Compress {
        sources: Vec<PathBuf>,
        dest: PathBuf,
    },
    Undo(Undo),
}

impl Op {
    pub fn label(&self) -> String {
        let n = |v: &Vec<PathBuf>| if v.len() == 1 { name(&v[0]) } else { format!("{} itens", v.len()) };
        match self {
            Op::Copy { sources, dest } => format!("Copiando {} para {}", n(sources), name(dest)),
            Op::Move { sources, dest } => format!("Movendo {} para {}", n(sources), name(dest)),
            Op::Trash { paths } => format!("Enviando {} para a lixeira", n(paths)),
            Op::Rename { from, .. } => format!("Renomeando {}", name(from)),
            Op::Mkdir { path } => format!("Criando {}", name(path)),
            Op::Extract { archive, .. } => format!("Extraindo {}", name(archive)),
            Op::Compress { dest, .. } => format!("Compactando {}", name(dest)),
            Op::Undo(_) => "Desfazendo".into(),
        }
    }
}

/// O que é preciso para reverter uma operação concluída.
#[derive(Debug, Clone, PartialEq)]
pub enum Undo {
    /// Itens criados (cópia, nova pasta): desfazer manda para a lixeira.
    Created(Vec<PathBuf>),
    /// Pares (origem, destino): desfazer move de volta.
    Moved(Vec<(PathBuf, PathBuf)>),
    /// Itens enviados para a lixeira: desfazer restaura.
    Trashed(Vec<PathBuf>),
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub summary: String,
    pub undo: Option<Undo>,
    /// Itens que devem ficar selecionados depois (o arquivo copiado, a pasta criada...).
    pub created: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum JobEvent {
    Progress { done: u64, total: u64, current: String },
    Finished(Result<Outcome, String>),
}

pub fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// Roda `op` numa thread e transmite o progresso.
pub fn spawn(op: Op, cancel: Arc<AtomicBool>) -> impl Stream<Item = JobEvent> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("arbor-job".into())
        .spawn(move || {
            let progress_tx = tx.clone();
            let mut ctx = Ctx::new(cancel, move |done, total, current: &str| {
                let _ = progress_tx.unbounded_send(JobEvent::Progress { done, total, current: current.to_string() });
            });
            let result = run(op, &mut ctx).map_err(|e| e.to_string());
            let _ = tx.unbounded_send(JobEvent::Finished(result));
        })
        .expect("thread de operação");
    rx
}

type Report<'a> = Box<dyn FnMut(u64, u64, &str) + Send + 'a>;

pub struct Ctx<'a> {
    cancel: Arc<AtomicBool>,
    done: u64,
    total: u64,
    last: Instant,
    report: Report<'a>,
}

impl<'a> Ctx<'a> {
    pub fn new(cancel: Arc<AtomicBool>, report: impl FnMut(u64, u64, &str) + Send + 'a) -> Self {
        Ctx { cancel, done: 0, total: 0, last: Instant::now(), report: Box::new(report) }
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn advance(&mut self, bytes: u64, current: &Path) {
        self.done += bytes;
        if self.last.elapsed().as_millis() >= 60 {
            self.last = Instant::now();
            (self.report)(self.done, self.total, &name(current));
        }
    }
}

fn cancelled_err() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "cancelado")
}

pub fn run(op: Op, ctx: &mut Ctx) -> io::Result<Outcome> {
    match op {
        Op::Copy { sources, dest } => {
            ctx.total = sources.iter().map(|s| tree_size(s)).sum();
            let mut created = Vec::new();
            let result = (|| {
                for src in &sources {
                    check_not_inside(src, &dest)?;
                    let target = unique_dest(&dest, &name(src));
                    copy_tree(src, &target, ctx)?;
                    created.push(target);
                }
                Ok(())
            })();
            finish(result, created.len(), "copiado", Undo::Created(created.clone()), created)
        }
        Op::Move { sources, dest } => {
            ctx.total = sources.len() as u64;
            let mut moved = Vec::new();
            let result = (|| {
                for src in &sources {
                    if src.parent() == Some(dest.as_path()) {
                        continue;
                    }
                    check_not_inside(src, &dest)?;
                    let target = unique_dest(&dest, &name(src));
                    move_item(src, &target, ctx)?;
                    moved.push((src.clone(), target));
                }
                Ok(())
            })();
            let created = moved.iter().map(|(_, to)| to.clone()).collect();
            finish(result, moved.len(), "movido", Undo::Moved(moved), created)
        }
        Op::Trash { paths } => {
            trash::delete_all(&paths).map_err(io::Error::other)?;
            Ok(Outcome {
                summary: format!("{} na lixeira. Ctrl+Z desfaz.", count(paths.len())),
                undo: Some(Undo::Trashed(paths)),
                created: vec![],
            })
        }
        Op::Rename { from, to } => {
            if to.exists() && !same_file_ignoring_case(&from, &to) {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} já existe", name(&to))));
            }
            fs::rename(&from, &to)?;
            Ok(Outcome {
                summary: format!("Renomeado para {}", name(&to)),
                undo: Some(Undo::Moved(vec![(from, to.clone())])),
                created: vec![to],
            })
        }
        Op::Mkdir { path } => {
            fs::create_dir(&path)?;
            Ok(Outcome {
                summary: format!("Pasta {} criada", name(&path)),
                undo: Some(Undo::Created(vec![path.clone()])),
                created: vec![path],
            })
        }
        Op::Extract { archive, members, dest } => {
            // Pasta nova criada para a extração: ela é o "item criado" (desfazer a manda inteira para a lixeira).
            let fresh = fs::symlink_metadata(&dest).is_err();
            fs::create_dir_all(&dest)?;
            let created = crate::archive::extract(&archive, &members, &dest, |done, total| {
                ctx.total = total;
                ctx.done = 0;
                ctx.advance(done, &archive);
                !ctx.cancelled()
            });
            match created {
                Ok(created) => {
                    let n = created.len();
                    let created = if fresh { vec![dest] } else { created };
                    Ok(Outcome {
                        summary: format!("{} extraído(s) de {}", count(n), name(&archive)),
                        undo: Some(Undo::Created(created.clone())),
                        created,
                    })
                }
                Err(e) => {
                    if fresh {
                        let _ = fs::remove_dir_all(&dest);
                    }
                    Err(e)
                }
            }
        }
        Op::Compress { sources, dest } => {
            crate::archive::compress_zip(&sources, &dest, |done, total| {
                ctx.total = total;
                ctx.done = 0;
                ctx.advance(done, &dest);
                !ctx.cancelled()
            })?;
            Ok(Outcome { summary: format!("{} criado", name(&dest)), undo: Some(Undo::Created(vec![dest.clone()])), created: vec![dest] })
        }
        Op::Undo(undo) => run_undo(undo, ctx),
    }
}

fn count(n: usize) -> String {
    if n == 1 {
        "1 item".into()
    } else {
        format!("{n} itens")
    }
}

/// Mesmo cancelada ou com erro no meio, a parte que deu certo continua desfazível.
fn finish(result: io::Result<()>, n: usize, verb: &str, undo: Undo, created: Vec<PathBuf>) -> io::Result<Outcome> {
    let plural = if n == 1 { verb.to_string() } else { format!("{verb}s") };
    let undo = if n > 0 { Some(undo) } else { None };
    match result {
        Ok(()) => Ok(Outcome { summary: format!("{} {plural}", count(n)), undo, created }),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => {
            Ok(Outcome { summary: format!("Cancelado: {} {plural} antes de parar", count(n)), undo, created })
        }
        Err(e) if n > 0 => Ok(Outcome { summary: format!("Parou com erro depois de {} {plural}: {e}", count(n)), undo, created }),
        Err(e) => Err(e),
    }
}

fn run_undo(undo: Undo, ctx: &mut Ctx) -> io::Result<Outcome> {
    match undo {
        Undo::Created(paths) => {
            let existing: Vec<_> = paths.into_iter().filter(|p| p.exists()).collect();
            if !existing.is_empty() {
                trash::delete_all(&existing).map_err(io::Error::other)?;
            }
            Ok(Outcome { summary: format!("Desfeito: {} na lixeira", count(existing.len())), undo: None, created: vec![] })
        }
        Undo::Moved(pairs) => {
            let mut back = Vec::new();
            for (from, to) in pairs.iter().rev() {
                if from.exists() && !same_file_ignoring_case(from, to) {
                    return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} já existe no lugar original", name(from))));
                }
                move_item(to, from, ctx)?;
                back.push(from.clone());
            }
            Ok(Outcome { summary: format!("Desfeito: {} de volta", count(back.len())), undo: None, created: back })
        }
        Undo::Trashed(paths) => restore(paths),
    }
}

#[cfg(any(windows, all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android"))))]
fn restore(paths: Vec<PathBuf>) -> io::Result<Outcome> {
    let items = trash::os_limited::list().map_err(io::Error::other)?;
    let mut wanted = Vec::new();
    for p in &paths {
        // O mais recente com o mesmo caminho original.
        if let Some(item) = items.iter().filter(|i| i.original_path() == *p).max_by_key(|i| i.time_deleted) {
            wanted.push(item.clone());
        }
    }
    let n = wanted.len();
    trash::os_limited::restore_all(wanted).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(Outcome { summary: format!("Desfeito: {} restaurado(s) da lixeira", count(n)), undo: None, created: paths })
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))))]
fn restore(_paths: Vec<PathBuf>) -> io::Result<Outcome> {
    Err(io::Error::other("restaurar da lixeira ainda não é suportado no macOS; use \"Colocar de volta\" no Finder"))
}

fn same_file_ignoring_case(a: &Path, b: &Path) -> bool {
    a.parent() == b.parent() && name(a).to_lowercase() == name(b).to_lowercase()
}

fn check_not_inside(src: &Path, dest: &Path) -> io::Result<()> {
    if src.is_dir() && dest.starts_with(src) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("não dá para colocar {} dentro dela mesma", name(src))));
    }
    Ok(())
}

/// `pasta/nome` se estiver livre; senão `nome (2).ext`, `nome (3).ext`...
pub fn unique_dest(dir: &Path, file_name: &str) -> PathBuf {
    let first = dir.join(file_name);
    if fs::symlink_metadata(&first).is_err() {
        return first;
    }
    let p = Path::new(file_name);
    let (stem, ext) = match (p.file_stem(), p.extension()) {
        (Some(s), Some(e)) if !file_name.starts_with('.') || s.to_string_lossy().contains('.') => {
            (s.to_string_lossy().into_owned(), format!(".{}", e.to_string_lossy()))
        }
        _ => (file_name.to_string(), String::new()),
    };
    (2..).map(|i| dir.join(format!("{stem} ({i}){ext}"))).find(|c| fs::symlink_metadata(c).is_err()).expect("sempre há um nome livre")
}

fn tree_size(p: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(p) else { return 0 };
    if meta.is_dir() {
        fs::read_dir(p).map(|rd| rd.flatten().map(|e| tree_size(&e.path())).sum()).unwrap_or(0)
    } else {
        meta.len()
    }
}

fn move_item(src: &Path, dst: &Path, ctx: &mut Ctx) -> io::Result<()> {
    if ctx.cancelled() {
        return Err(cancelled_err());
    }
    match fs::rename(src, dst) {
        Ok(()) => {
            ctx.advance(1, src);
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices || is_cross_device(&e) => {
            // Outro disco: copia com conferência e só então apaga a origem.
            copy_tree(src, dst, ctx)?;
            if fs::symlink_metadata(src)?.is_dir() {
                fs::remove_dir_all(src)
            } else {
                fs::remove_file(src)
            }
        }
        Err(e) => Err(e),
    }
}

fn is_cross_device(e: &io::Error) -> bool {
    #[cfg(unix)]
    const CODE: i32 = 18; // EXDEV
    #[cfg(windows)]
    const CODE: i32 = 17; // ERROR_NOT_SAME_DEVICE
    #[cfg(not(any(unix, windows)))]
    const CODE: i32 = -1;
    e.raw_os_error() == Some(CODE)
}

fn copy_tree(src: &Path, dst: &Path, ctx: &mut Ctx) -> io::Result<()> {
    if ctx.cancelled() {
        return Err(cancelled_err());
    }
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() && copy_symlink(src, dst).is_ok() {
        return Ok(());
    }
    if meta.is_dir() || (meta.file_type().is_symlink() && src.is_dir()) {
        fs::create_dir(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dst.join(entry.file_name()), ctx)?;
        }
        Ok(())
    } else {
        copy_file(src, dst, ctx)
    }
}

#[cfg(unix)]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(src)?, dst)
}

#[cfg(windows)]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    let target = fs::read_link(src)?;
    if src.is_dir() {
        std::os::windows::fs::symlink_dir(target, dst)
    } else {
        std::os::windows::fs::symlink_file(target, dst)
    }
}

#[cfg(not(any(unix, windows)))]
fn copy_symlink(_src: &Path, _dst: &Path) -> io::Result<()> {
    Err(io::Error::other("sem suporte a links"))
}

fn copy_file(src: &Path, dst: &Path, ctx: &mut Ctx) -> io::Result<()> {
    let part = dst.with_file_name(format!(".{}.arbor-part", name(dst)));
    let result = (|| {
        let mut input = fs::File::open(src)?;
        let meta = input.metadata()?;
        let mut output = fs::File::create(&part)?;
        let mut buf = vec![0u8; 1 << 20];
        let mut written = 0u64;
        loop {
            if ctx.cancelled() {
                return Err(cancelled_err());
            }
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            output.write_all(&buf[..n])?;
            written += n as u64;
            ctx.advance(n as u64, src);
        }
        output.sync_all()?;
        if written != meta.len() {
            return Err(io::Error::other(format!("{}: copiados {written} de {} bytes", name(src), meta.len())));
        }
        if let Ok(modified) = meta.modified() {
            let _ = output.set_modified(modified);
        }
        drop(output);
        fs::set_permissions(&part, meta.permissions())?;
        if fs::symlink_metadata(dst).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} já existe", name(dst))));
        }
        fs::rename(&part, dst)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx<'static> {
        Ctx::new(Arc::new(AtomicBool::new(false)), |_, _, _| {})
    }

    #[test]
    fn nome_unico() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("a.txt"), "1").unwrap();
        fs::write(d.path().join("a (2).txt"), "1").unwrap();
        assert_eq!(unique_dest(d.path(), "a.txt"), d.path().join("a (3).txt"));
        assert_eq!(unique_dest(d.path(), "b.txt"), d.path().join("b.txt"));
        fs::create_dir(d.path().join(".config")).unwrap();
        assert_eq!(unique_dest(d.path(), ".config"), d.path().join(".config (2)"));
    }

    #[test]
    fn copia_arvore_e_desfaz() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("sub/x.bin"), vec![7u8; 3 << 20]).unwrap();
        let dest = d.path().join("dest");
        fs::create_dir(&dest).unwrap();
        let out = run(Op::Copy { sources: vec![src.clone()], dest: dest.clone() }, &mut ctx()).unwrap();
        assert_eq!(fs::read(dest.join("src/sub/x.bin")).unwrap().len(), 3 << 20);
        assert!(!dest.join("src/sub/.x.bin.arbor-part").exists());
        assert_eq!(out.undo, Some(Undo::Created(vec![dest.join("src")])));
        // Copiar de novo gera "src (2)".
        run(Op::Copy { sources: vec![src.clone()], dest: dest.clone() }, &mut ctx()).unwrap();
        assert!(dest.join("src (2)/sub/x.bin").exists());
    }

    #[test]
    fn nao_copia_pasta_para_dentro_dela() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a");
        fs::create_dir_all(src.join("b")).unwrap();
        assert!(run(Op::Copy { sources: vec![src.clone()], dest: src.join("b") }, &mut ctx()).is_err());
    }

    #[test]
    fn move_e_desfaz() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("f.txt");
        fs::write(&f, "oi").unwrap();
        let dest = d.path().join("dest");
        fs::create_dir(&dest).unwrap();
        let out = run(Op::Move { sources: vec![f.clone()], dest: dest.clone() }, &mut ctx()).unwrap();
        assert!(!f.exists() && dest.join("f.txt").exists());
        run(Op::Undo(out.undo.unwrap()), &mut ctx()).unwrap();
        assert!(f.exists() && !dest.join("f.txt").exists());
    }

    #[test]
    fn renomeia_e_desfaz() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        fs::write(&a, "").unwrap();
        let out = run(Op::Rename { from: a.clone(), to: d.path().join("b") }, &mut ctx()).unwrap();
        run(Op::Undo(out.undo.unwrap()), &mut ctx()).unwrap();
        assert!(a.exists());
    }

    #[test]
    fn cancelar_nao_deixa_arquivo_parcial() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("big");
        fs::write(&src, vec![1u8; 4 << 20]).unwrap();
        let dest = d.path().join("dest");
        fs::create_dir(&dest).unwrap();
        let cancel = Arc::new(AtomicBool::new(true));
        let mut c = Ctx::new(cancel, |_, _, _| {});
        let out = run(Op::Copy { sources: vec![src], dest: dest.clone() }, &mut c).unwrap();
        assert!(out.summary.starts_with("Cancelado"));
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
    }
}
