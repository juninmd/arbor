//! Compactados como pastas: listar, extrair e criar zip.
//!
//! Um caminho como `/Downloads/fotos.zip/2025/jan` aponta para dentro do arquivo: [`split`] separa o
//! compactado (`/Downloads/fotos.zip`) do caminho interno (`2025/jan`). A listagem lê o índice do arquivo e
//! sintetiza as pastas a partir dos caminhos dos membros (nem todo zip grava entradas de pasta).
//!
//! Formatos: zip, 7z, tar, tar.gz/tgz e tar.xz/txz, todos em Rust puro. Caminhos de membros com `..` ou
//! absolutos são ignorados (proteção contra "zip slip").

use crate::entry::Entry;
use crate::ops::{name, unique_dest};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    SevenZ,
    Tar,
    TarGz,
    TarXz,
}

/// Formato pelo nome do arquivo.
pub fn format_of(path: &Path) -> Option<Format> {
    let n = path.file_name()?.to_string_lossy().to_lowercase();
    Some(if n.ends_with(".zip") {
        Format::Zip
    } else if n.ends_with(".7z") {
        Format::SevenZ
    } else if n.ends_with(".tar.gz") || n.ends_with(".tgz") {
        Format::TarGz
    } else if n.ends_with(".tar.xz") || n.ends_with(".txz") {
        Format::TarXz
    } else if n.ends_with(".tar") {
        Format::Tar
    } else {
        return None;
    })
}

pub fn is_archive(path: &Path) -> bool {
    format_of(path).is_some()
}

/// Se `path` aponta para dentro de um compactado, devolve (compactado, caminho interno).
/// O próprio compactado (`a.zip`) também conta, com caminho interno vazio.
pub fn split(path: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut archive = PathBuf::new();
    let mut comps = path.components();
    while let Some(c) = comps.next() {
        archive.push(c);
        if matches!(c, Component::Normal(_)) && is_archive(&archive) && archive.is_file() {
            return Some((archive, comps.as_path().to_path_buf()));
        }
    }
    None
}

/// Um membro do compactado.
#[derive(Debug, Clone)]
pub struct Member {
    /// Caminho interno, já limpo (só componentes normais).
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// Caminho interno seguro: sem raiz, sem `..`, sem `.`.
fn clean(raw: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in Path::new(&raw.replace('\\', "/")).components() {
        match c {
            Component::Normal(n) => out.push(n),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn invalid(e: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

fn tar_reader(archive: &Path, format: Format) -> io::Result<tar::Archive<Box<dyn Read>>> {
    let file = BufReader::new(File::open(archive)?);
    let reader: Box<dyn Read> = match format {
        Format::TarGz => Box::new(flate2::read::GzDecoder::new(file)),
        Format::TarXz => {
            // lzma-rs descompacta tudo de uma vez; tar.xz grandes ficam em memória (aceitável para navegar).
            let mut out = Vec::new();
            lzma_rs::xz_decompress(&mut BufReader::new(file), &mut out).map_err(invalid)?;
            Box::new(io::Cursor::new(out))
        }
        _ => Box::new(file),
    };
    Ok(tar::Archive::new(reader))
}

/// Todos os membros do compactado.
pub fn members(archive: &Path) -> io::Result<Vec<Member>> {
    let format = format_of(archive).ok_or_else(|| invalid("formato não suportado"))?;
    let mut out = Vec::new();
    match format {
        Format::Zip => {
            let mut zip = zip::ZipArchive::new(BufReader::new(File::open(archive)?)).map_err(invalid)?;
            for i in 0..zip.len() {
                let f = zip.by_index_raw(i).map_err(invalid)?;
                let Some(path) = f.enclosed_name().and_then(|p| clean(&p.to_string_lossy())) else { continue };
                let modified = f.last_modified().and_then(|d| {
                    let date = chrono::NaiveDate::from_ymd_opt(d.year() as i32, d.month() as u32, d.day() as u32)?;
                    let t = date.and_hms_opt(d.hour() as u32, d.minute() as u32, d.second() as u32)?;
                    Some(SystemTime::from(t.and_utc()))
                });
                out.push(Member { path, is_dir: f.is_dir(), size: f.size(), modified });
            }
        }
        Format::SevenZ => {
            let reader = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty()).map_err(invalid)?;
            for e in &reader.archive().files {
                let Some(path) = clean(e.name()) else { continue };
                let modified = e.has_last_modified_date.then(|| SystemTime::from(e.last_modified_date()));
                out.push(Member { path, is_dir: e.is_directory(), size: e.size(), modified });
            }
        }
        Format::Tar | Format::TarGz | Format::TarXz => {
            let mut tar = tar_reader(archive, format)?;
            for e in tar.entries()? {
                let e = e?;
                let Some(path) = e.path().ok().and_then(|p| clean(&p.to_string_lossy())) else { continue };
                let h = e.header();
                let modified = h.mtime().ok().map(|s| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(s));
                out.push(Member { path, is_dir: h.entry_type().is_dir(), size: h.size().unwrap_or(0), modified });
            }
        }
    }
    Ok(out)
}

/// Itens diretamente dentro de `inner` (vazio = raiz), como entradas de pasta. Pastas que só existem
/// implicitamente (`a/b/c.txt` sem entrada `a/`) também aparecem.
pub fn list(archive: &Path, inner: &Path) -> io::Result<Vec<Entry>> {
    let mut children: BTreeMap<String, (bool, u64, Option<SystemTime>)> = BTreeMap::new();
    for m in members(archive)? {
        let Ok(rest) = m.path.strip_prefix(inner) else { continue };
        let mut comps = rest.components();
        let Some(first) = comps.next() else { continue };
        let first = first.as_os_str().to_string_lossy().into_owned();
        let nested = comps.next().is_some();
        let slot = children.entry(first).or_insert((false, 0, None));
        if nested || m.is_dir {
            slot.0 = true;
        } else {
            *slot = (false, m.size, m.modified);
        }
        if slot.2.is_none() {
            slot.2 = m.modified;
        }
    }
    let base = archive.join(inner);
    Ok(children.into_iter().map(|(n, (is_dir, size, modified))| Entry::virtual_entry(&n, base.join(&n), is_dir, size, modified)).collect())
}

/// Extrai `wanted` (caminhos internos; vazio = tudo) para dentro de `dest`. Pastas levam o conteúdo junto.
/// Devolve os itens de primeiro nível criados em `dest`. `progress` recebe (bytes feitos, total).
pub fn extract(archive: &Path, wanted: &[PathBuf], dest: &Path, mut progress: impl FnMut(u64, u64) -> bool) -> io::Result<Vec<PathBuf>> {
    let all = members(archive)?;
    // O que extrair e onde: o membro `a/b/c.txt`, pedido como `a/b`, vai para `dest/b/c.txt`.
    let mut renames: Vec<(PathBuf, PathBuf)> = Vec::new(); // (prefixo interno pedido, destino no disco)
    if wanted.is_empty() {
        let mut tops: Vec<PathBuf> = all.iter().filter_map(|m| m.path.components().next().map(|c| PathBuf::from(c.as_os_str()))).collect();
        tops.sort();
        tops.dedup();
        for t in tops {
            let target = unique_dest(dest, &name(&t));
            renames.push((t, target));
        }
    } else {
        for w in wanted {
            let target = unique_dest(dest, &name(w));
            renames.push((w.clone(), target));
        }
    }
    let target_of = |p: &Path| -> Option<PathBuf> {
        renames.iter().find_map(|(prefix, to)| {
            p.strip_prefix(prefix).ok().map(|rest| if rest.as_os_str().is_empty() { to.clone() } else { to.join(rest) })
        })
    };
    let total: u64 = all.iter().filter(|m| target_of(&m.path).is_some()).map(|m| m.size).sum();
    let mut done = 0u64;
    let mut write = |path: &Path, is_dir: bool, size: u64, reader: &mut dyn Read| -> io::Result<()> {
        let Some(to) = target_of(path) else { return Ok(()) };
        if is_dir {
            return fs::create_dir_all(&to);
        }
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&to)?;
        io::copy(reader, &mut out)?;
        done += size;
        if !progress(done, total) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado"));
        }
        Ok(())
    };
    let format = format_of(archive).ok_or_else(|| invalid("formato não suportado"))?;
    match format {
        Format::Zip => {
            let mut zip = zip::ZipArchive::new(BufReader::new(File::open(archive)?)).map_err(invalid)?;
            for i in 0..zip.len() {
                let mut f = zip.by_index(i).map_err(invalid)?;
                let Some(path) = f.enclosed_name().and_then(|p| clean(&p.to_string_lossy())) else { continue };
                let (is_dir, size) = (f.is_dir(), f.size());
                write(&path, is_dir, size, &mut f)?;
            }
        }
        Format::SevenZ => {
            let mut reader = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty()).map_err(invalid)?;
            let mut err = None;
            reader
                .for_each_entries(|e, r| {
                    let Some(path) = clean(e.name()) else { return Ok(true) };
                    match write(&path, e.is_directory(), e.size(), r) {
                        Ok(()) => Ok(true),
                        Err(x) => {
                            err = Some(x);
                            Ok(false)
                        }
                    }
                })
                .map_err(invalid)?;
            if let Some(e) = err {
                return Err(e);
            }
        }
        Format::Tar | Format::TarGz | Format::TarXz => {
            let mut tar = tar_reader(archive, format)?;
            for e in tar.entries()? {
                let mut e = e?;
                let Some(path) = e.path().ok().and_then(|p| clean(&p.to_string_lossy())) else { continue };
                let kind = e.header().entry_type();
                if !(kind.is_dir() || kind.is_file()) {
                    continue; // links e especiais ficam de fora
                }
                let size = e.header().size().unwrap_or(0);
                write(&path, kind.is_dir(), size, &mut e)?;
            }
        }
    }
    Ok(renames.into_iter().map(|(_, to)| to).filter(|p| p.exists()).collect())
}

/// Cria `dest` (zip) com `sources` (arquivos e pastas) na raiz do zip.
pub fn compress_zip(sources: &[PathBuf], dest: &Path, mut progress: impl FnMut(u64, u64) -> bool) -> io::Result<()> {
    let part = dest.with_file_name(format!(".{}.arbor-part", name(dest)));
    let result = (|| {
        let mut files: Vec<(PathBuf, String, bool)> = Vec::new(); // (disco, nome no zip, é pasta)
        for s in sources {
            collect(s, &name(s), &mut files)?;
        }
        let total: u64 = files.iter().filter(|f| !f.2).filter_map(|f| fs::metadata(&f.0).ok()).map(|m| m.len()).sum();
        let mut zip = zip::ZipWriter::new(File::create(&part)?);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
        let mut done = 0;
        let mut buf = vec![0u8; 1 << 20];
        for (disk, inner, is_dir) in files {
            if is_dir {
                zip.add_directory(inner, opts).map_err(invalid)?;
                continue;
            }
            zip.start_file(inner, opts).map_err(invalid)?;
            let mut input = File::open(&disk)?;
            loop {
                let n = input.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buf[..n])?;
                done += n as u64;
                if !progress(done, total) {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado"));
                }
            }
        }
        let mut file = zip.finish().map_err(invalid)?;
        file.flush()?;
        file.sync_all()?;
        let _ = file.seek(io::SeekFrom::Start(0));
        if fs::symlink_metadata(dest).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} já existe", name(dest))));
        }
        fs::rename(&part, dest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

fn collect(path: &Path, inner: &str, out: &mut Vec<(PathBuf, String, bool)>) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        out.push((path.to_path_buf(), format!("{inner}/"), true));
        for e in fs::read_dir(path)? {
            let e = e?;
            collect(&e.path(), &format!("{inner}/{}", e.file_name().to_string_lossy()), out)?;
        }
    } else if meta.is_file() {
        out.push((path.to_path_buf(), inner.to_string(), false));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(dir: &Path) -> PathBuf {
        let src = dir.join("src");
        fs::create_dir_all(src.join("docs/2025")).unwrap();
        fs::write(src.join("docs/2025/a.txt"), "aaa").unwrap();
        fs::write(src.join("leia.md"), "oi").unwrap();
        let zip = dir.join("pacote.zip");
        compress_zip(&[src.join("docs"), src.join("leia.md")], &zip, |_, _| true).unwrap();
        zip
    }

    #[test]
    fn zip_ida_e_volta() {
        let d = tempfile::tempdir().unwrap();
        let zip = sample(d.path());
        let mut root: Vec<_> = list(&zip, Path::new("")).unwrap().into_iter().map(|e| (e.name, e.is_dir)).collect();
        root.sort();
        assert_eq!(root, [("docs".to_string(), true), ("leia.md".to_string(), false)]);
        let inner = list(&zip, Path::new("docs/2025")).unwrap();
        assert_eq!(inner[0].name, "a.txt");
        assert_eq!(inner[0].size, 3);
        assert_eq!(inner[0].path, zip.join("docs/2025/a.txt"));

        let out = d.path().join("out");
        fs::create_dir(&out).unwrap();
        let created = extract(&zip, &[PathBuf::from("docs/2025")], &out, |_, _| true).unwrap();
        assert_eq!(created, vec![out.join("2025")]);
        assert_eq!(fs::read_to_string(out.join("2025/a.txt")).unwrap(), "aaa");
        let all = extract(&zip, &[], &out, |_, _| true).unwrap();
        assert_eq!(all.len(), 2);
        assert!(out.join("docs/2025/a.txt").exists());
    }

    #[test]
    fn split_acha_o_compactado_no_caminho() {
        let d = tempfile::tempdir().unwrap();
        let zip = sample(d.path());
        assert_eq!(split(&zip.join("docs/2025")), Some((zip.clone(), PathBuf::from("docs/2025"))));
        assert_eq!(split(&zip), Some((zip.clone(), PathBuf::new())));
        assert_eq!(split(d.path()), None);
    }

    #[test]
    fn tar_gz_lista_e_extrai() {
        let d = tempfile::tempdir().unwrap();
        let tgz = d.path().join("x.tar.gz");
        {
            let enc = flate2::write::GzEncoder::new(File::create(&tgz).unwrap(), flate2::Compression::fast());
            let mut b = tar::Builder::new(enc);
            let data = b"conteudo";
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, "pasta/f.txt", &data[..]).unwrap();
            b.into_inner().unwrap().finish().unwrap();
        }
        assert_eq!(list(&tgz, Path::new("")).unwrap()[0].name, "pasta");
        let out = d.path().join("out");
        fs::create_dir(&out).unwrap();
        extract(&tgz, &[], &out, |_, _| true).unwrap();
        assert_eq!(fs::read_to_string(out.join("pasta/f.txt")).unwrap(), "conteudo");
    }

    #[test]
    fn sete_z_lista_e_extrai() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        fs::create_dir_all(src.join("pasta")).unwrap();
        fs::write(src.join("pasta/f.txt"), "sete").unwrap();
        let file = d.path().join("x.7z");
        sevenz_rust2::compress_to_path(&src, &file).unwrap();
        assert_eq!(list(&file, Path::new("")).unwrap()[0].name, "pasta");
        let out = d.path().join("out");
        fs::create_dir(&out).unwrap();
        extract(&file, &[PathBuf::from("pasta/f.txt")], &out, |_, _| true).unwrap();
        assert_eq!(fs::read_to_string(out.join("f.txt")).unwrap(), "sete");
    }

    #[test]
    fn caminhos_perigosos_sao_ignorados() {
        assert_eq!(clean("../../etc/passwd"), None);
        assert_eq!(clean("/abs"), None);
        assert_eq!(clean("./a\\b"), Some(PathBuf::from("a/b")));
    }
}
