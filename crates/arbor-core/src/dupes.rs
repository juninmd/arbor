//! Localizar arquivos duplicados (Directory Opus, XYplorer, Spacedrive): agrupa por tamanho, depois por um
//! hash dos primeiros 64 KB, depois pelo conteúdo inteiro, e confirma byte a byte contra o primeiro do grupo.
//! Assim só lê por completo quem tem chance real de ser cópia, e nunca acusa falso positivo.

use crate::entry::Entry;
use rayon::prelude::*;
use std::collections::HashMap;
use std::hash::Hasher;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Consulta especial da aba de resultados (como `search::FLAT`): ninguém digita um caractere de controle.
pub const QUERY: &str = "\u{1}duplicados";

/// Mais que isso a varredura vira um índice, fora do escopo.
pub const MAX_FILES: usize = 500_000;
const HEAD: usize = 64 * 1024;

#[derive(Debug, Default)]
pub struct Dupes {
    /// Grupos de arquivos idênticos, do que mais desperdiça espaço para o que menos.
    pub groups: Vec<Vec<Entry>>,
    /// Bytes que sobrariam mantendo uma cópia de cada grupo.
    pub wasted: u64,
    pub truncated: bool,
}

fn collect(dir: &Path, show_hidden: bool, out: &Mutex<Vec<Entry>>, stop: &(dyn Fn() -> bool + Sync)) {
    if stop() || out.lock().map(|v| v.len() >= MAX_FILES).unwrap_or(true) {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut subdirs = vec![];
    let mut files = vec![];
    for item in read.flatten() {
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && !show_hidden {
            continue;
        }
        let Ok(ft) = item.file_type() else { continue };
        if ft.is_dir() {
            subdirs.push(item.path());
        } else if ft.is_file() {
            if let Ok(meta) = item.metadata() {
                if meta.len() > 0 {
                    files.push(Entry::new(name, item.path(), &meta, false));
                }
            }
        }
    }
    if let Ok(mut v) = out.lock() {
        v.extend(files);
    }
    subdirs.par_iter().for_each(|d| collect(d, show_hidden, out, stop));
}

fn hash(path: &Path, limit: Option<usize>) -> Option<u64> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut left = limit.unwrap_or(usize::MAX);
    while left > 0 {
        let want = buf.len().min(left);
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        h.write(&buf[..n]);
        left -= n;
    }
    Some(h.finish())
}

fn same_bytes(a: &Path, b: &Path) -> bool {
    let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
    let (mut ba, mut bb) = (vec![0u8; 64 * 1024], vec![0u8; 64 * 1024]);
    loop {
        let (Ok(na), Ok(nb)) = (fa.read(&mut ba), fb.read(&mut bb)) else { return false };
        // `read` pode devolver menos que o buffer; com arquivos do mesmo tamanho basta comparar o menor pedaço.
        if na != nb {
            return read_rest_equal(&ba[..na], &bb[..nb], &mut fa, &mut fb);
        }
        if na == 0 {
            return true;
        }
        if ba[..na] != bb[..nb] {
            return false;
        }
    }
}

fn read_rest_equal(a: &[u8], b: &[u8], fa: &mut std::fs::File, fb: &mut std::fs::File) -> bool {
    let (mut va, mut vb) = (a.to_vec(), b.to_vec());
    if fa.read_to_end(&mut va).is_err() || fb.read_to_end(&mut vb).is_err() {
        return false;
    }
    va == vb
}

/// Divide `files` em grupos com a mesma chave; descarta grupos de um só.
fn split<K: std::hash::Hash + Eq + Send>(files: Vec<Entry>, key: impl Fn(&Entry) -> Option<K> + Sync) -> Vec<Vec<Entry>> {
    let keyed: Vec<(K, Entry)> = files.into_par_iter().filter_map(|e| key(&e).map(|k| (k, e))).collect();
    let mut map: HashMap<K, Vec<Entry>> = HashMap::new();
    for (k, e) in keyed {
        map.entry(k).or_default().push(e);
    }
    map.into_values().filter(|g| g.len() > 1).collect()
}

pub fn find(root: &Path, show_hidden: bool, stop: &(dyn Fn() -> bool + Sync)) -> Option<Dupes> {
    let all = Mutex::new(Vec::new());
    collect(root, show_hidden, &all, stop);
    if stop() {
        return None;
    }
    let all = all.into_inner().ok()?;
    let truncated = all.len() >= MAX_FILES;
    let by_size = split(all, |e| Some(e.size));
    let mut groups = vec![];
    for g in by_size {
        if stop() {
            return None;
        }
        for g in split(g, |e| hash(&e.path, Some(HEAD))) {
            // Par: comparar direto lê cada arquivo uma vez; hash dos dois e depois a comparação leria duas.
            let direct = g[0].size as usize <= HEAD || g.len() == 2;
            let full = if direct { vec![g] } else { split(g, |e| hash(&e.path, None)) };
            for g in full {
                let first: PathBuf = g[0].path.clone();
                let confirmed: Vec<Entry> = g.into_iter().filter(|e| e.path == first || same_bytes(&first, &e.path)).collect();
                if confirmed.len() > 1 {
                    groups.push(confirmed);
                }
            }
        }
    }
    let waste = |g: &Vec<Entry>| g[0].size * (g.len() as u64 - 1);
    groups.sort_by_key(|g| std::cmp::Reverse(waste(g)));
    for g in &mut groups {
        g.sort_by(|a, b| a.path.cmp(&b.path));
    }
    let wasted = groups.iter().map(waste).sum();
    Some(Dupes { groups, wasted, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn acha_copias_em_subpastas_e_ignora_mesmo_tamanho_com_conteudo_diferente() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("a/b")).unwrap();
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let mut other = big.clone();
        other[150_000] ^= 1; // mesmo tamanho e mesmo começo, fim diferente
        fs::write(d.path().join("x.bin"), &big).unwrap();
        fs::write(d.path().join("a/b/x-copia.bin"), &big).unwrap();
        fs::write(d.path().join("a/quase.bin"), &other).unwrap();
        fs::write(d.path().join("p1.txt"), "oi").unwrap();
        fs::write(d.path().join("a/p2.txt"), "oi").unwrap();
        fs::write(d.path().join("vazio1"), "").unwrap();
        fs::write(d.path().join("vazio2"), "").unwrap();
        let r = find(d.path(), false, &|| false).unwrap();
        let names: Vec<Vec<String>> = r.groups.iter().map(|g| g.iter().map(|e| e.name.clone()).collect()).collect();
        assert_eq!(names, [vec!["x-copia.bin".to_string(), "x.bin".into()], vec!["p2.txt".into(), "p1.txt".into()]]);
        assert_eq!(r.wasted, 200_002);
        assert!(find(d.path(), false, &|| true).is_none());
    }
}
