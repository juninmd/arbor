//! Entradas de diretório, chave de ordenação natural pré-calculada e formatação.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_link: bool,
    pub hidden: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// Extensão em minúsculas, sem o ponto (guardada na própria entrada, sem alocação).
    pub ext: Ext,
    /// Nome em minúsculas com os números codificados para que `memcmp` dê a ordem natural
    /// ("img2" antes de "img10"). Calculado uma vez na listagem: ordenar 100 mil nomes vira comparar bytes.
    pub key: Box<[u8]>,
}

impl Entry {
    pub fn new(name: String, path: PathBuf, meta: &std::fs::Metadata, is_link: bool) -> Entry {
        let is_dir = meta.is_dir();
        let ext = if is_dir { Ext::default() } else { extension(&name) };
        Entry {
            hidden: is_hidden(&name, meta),
            size: if is_dir { 0 } else { meta.len() },
            modified: meta.modified().ok(),
            key: natural_key(&name),
            name,
            path,
            is_dir,
            is_link,
            ext,
        }
    }

    /// Entrada sem metadados do disco (testes e benchmarks).
    pub fn synthetic(name: &str, is_dir: bool, size: u64) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(name),
            is_dir,
            is_link: false,
            hidden: name.starts_with('.'),
            size,
            modified: None,
            ext: if is_dir { Ext::default() } else { extension(name) },
            key: natural_key(name),
        }
    }
}

fn extension(name: &str) -> Ext {
    Path::new(name).extension().and_then(|e| e.to_str()).map(Ext::new).unwrap_or_default()
}

/// Extensão curta (até 7 bytes) em minúsculas, guardada inline: 100 mil entradas economizam 100 mil alocações.
/// Extensões maiores contam como "sem extensão" para cor e ordenação.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ext {
    len: u8,
    buf: [u8; 7],
}

impl Ext {
    pub fn new(ext: &str) -> Ext {
        let lower = ext.to_lowercase();
        if lower.len() > 7 {
            return Ext::default();
        }
        let mut buf = [0u8; 7];
        buf[..lower.len()].copy_from_slice(lower.as_bytes());
        Ext { len: lower.len() as u8, buf }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buf[..self.len as usize]).unwrap_or_default()
    }
}

impl std::ops::Deref for Ext {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Debug for Ext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

#[cfg(windows)]
fn is_hidden(name: &str, meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    name.starts_with('.') || meta.file_attributes() & HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &str, _meta: &std::fs::Metadata) -> bool {
    name.starts_with('.')
}

/// Cada sequência de dígitos vira `'0'`, o comprimento (sem zeros à esquerda) e os dígitos; o resto vai em
/// minúsculas. Assim números menores e mais curtos vêm antes, e dígitos continuam antes de letras.
pub fn natural_key(name: &str) -> Box<[u8]> {
    let mut out = Vec::with_capacity(name.len() + 4);
    let mut chars = name.chars().peekable();
    let mut buf = [0u8; 4];
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            // marcador, comprimento (preenchido depois) e os dígitos sem zeros à esquerda
            out.push(b'0');
            let len_at = out.len();
            out.push(0);
            let mut leading = c == '0';
            if !leading {
                out.push(c as u8);
            }
            while let Some(&d) = chars.peek() {
                if !d.is_ascii_digit() {
                    break;
                }
                chars.next();
                if leading && d == '0' {
                    continue;
                }
                leading = false;
                out.push(d as u8);
            }
            if out.len() == len_at + 1 {
                out.push(b'0'); // o número era só zeros
            }
            out[len_at] = (out.len() - len_at - 1).min(255) as u8;
        } else if c.is_ascii() {
            out.push(c.to_ascii_lowercase() as u8);
        } else {
            for l in c.to_lowercase() {
                out.extend_from_slice(l.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out.into_boxed_slice()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
    Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub key: SortKey,
    pub desc: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Sort { key: SortKey::Name, desc: false }
    }
}

/// Pastas antes de arquivos; nomes em ordem natural, sem diferenciar maiúsculas.
pub fn compare(a: &Entry, b: &Entry, sort: Sort) -> Ordering {
    b.is_dir.cmp(&a.is_dir).then_with(|| {
        let by_name = || a.key.cmp(&b.key).then_with(|| a.name.cmp(&b.name));
        let ord = match sort.key {
            SortKey::Name => by_name(),
            SortKey::Size => a.size.cmp(&b.size).then_with(by_name),
            SortKey::Modified => a.modified.cmp(&b.modified).then_with(by_name),
            SortKey::Kind => a.ext.cmp(&b.ext).then_with(by_name),
        };
        if sort.desc {
            ord.reverse()
        } else {
            ord
        }
    })
}

/// Ordena em paralelo quando a pasta é grande; abaixo disso o custo de dividir não compensa.
pub fn sort_entries(entries: &mut [Entry], sort: Sort) {
    if entries.len() >= 8_192 {
        use rayon::slice::ParallelSliceMut;
        entries.par_sort_unstable_by(|a, b| compare(a, b, sort));
    } else {
        entries.sort_unstable_by(|a, b| compare(a, b, sort));
    }
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v < 10.0 {
        format!("{v:.1} {}", UNITS[i])
    } else {
        format!("{v:.0} {}", UNITS[i])
    }
}

pub fn human_time(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return String::new() };
    let dt: chrono::DateTime<chrono::Local> = t.into();
    dt.format("%d/%m/%Y %H:%M").to_string()
}

/// "1.234" com separador de milhar, para contagens na tela.
pub fn human_count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, dir: bool, size: u64) -> Entry {
        Entry::synthetic(name, dir, size)
    }

    fn names(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn pastas_primeiro_e_ordem_natural() {
        let mut v = [e("b10", false, 1), e("B2", false, 1), e("z", true, 0), e("a", false, 1)];
        sort_entries(&mut v, Sort::default());
        assert_eq!(names(&v), ["z", "a", "B2", "b10"]);
    }

    /// Mesma ordem do `natord` (a do protótipo), exceto zeros à esquerda: aqui "02" vale 2, como no Explorer.
    #[test]
    fn chave_natural_igual_ao_natord() {
        let mut v: Vec<Entry> =
            ["img10.png", "img2.png", "Img1", "a.b", "a-b", "a1b", "ab", "x100y", "x99y", "Ábaco", "zeta", "0", "007", "_x"]
                .iter()
                .map(|n| e(n, false, 0))
                .collect();
        sort_entries(&mut v, Sort::default());
        let mut expected: Vec<&str> = names(&v);
        expected.sort_by(|a, b| natord::compare_ignore_case(a, b).then(a.cmp(b)));
        assert_eq!(names(&v), expected);
    }

    #[test]
    fn ordena_por_tamanho_desc() {
        let mut v = [e("a", false, 1), e("b", false, 3), e("c", false, 2)];
        sort_entries(&mut v, Sort { key: SortKey::Size, desc: true });
        assert_eq!(v[0].name, "b");
    }

    #[test]
    fn ordena_muitos_em_paralelo() {
        let mut v: Vec<Entry> = (0..20_000).rev().map(|i| e(&format!("f{i}"), false, 0)).collect();
        sort_entries(&mut v, Sort::default());
        assert_eq!(v[0].name, "f0");
        assert_eq!(v[19_999].name, "f19999");
    }

    #[test]
    fn extensao_inline() {
        assert_eq!(Entry::synthetic("Foto.JPEG", false, 0).ext.as_str(), "jpeg");
        assert_eq!(Entry::synthetic("a.torrent", false, 0).ext.as_str(), "torrent");
        assert!(Entry::synthetic("a.muitolonga", false, 0).ext.is_empty());
        assert!(Entry::synthetic("README", false, 0).ext.is_empty());
    }

    #[test]
    fn tamanhos_e_contagens_legiveis() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(50 * 1024 * 1024), "50 MB");
        assert_eq!(human_count(100_000), "100.000");
        assert_eq!(human_count(999), "999");
    }
}
