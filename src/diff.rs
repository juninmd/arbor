//! Comparar dois arquivos (Total Commander "Comparar por conteúdo", Dolphin com o Kompare): primeiro uma
//! conferência byte a byte, que já responde "idênticos"; se diferem, abre a ferramenta de diff configurada
//! (`diff = programa` no arquivo `settings`) ou a primeira conhecida no PATH.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `None` se idênticos; senão a posição (em bytes) da primeira diferença.
pub fn first_difference(a: &Path, b: &Path) -> std::io::Result<Option<u64>> {
    let (mut fa, mut fb) = (std::fs::File::open(a)?, std::fs::File::open(b)?);
    let (mut ba, mut bb) = (vec![0u8; 64 * 1024], vec![0u8; 64 * 1024]);
    let mut offset = 0u64;
    loop {
        let na = read_full(&mut fa, &mut ba)?;
        let nb = read_full(&mut fb, &mut bb)?;
        if let Some(i) = ba[..na.min(nb)].iter().zip(&bb[..na.min(nb)]).position(|(x, y)| x != y) {
            return Ok(Some(offset + i as u64));
        }
        if na != nb {
            return Ok(Some(offset + na.min(nb) as u64));
        }
        if na == 0 {
            return Ok(None);
        }
        offset += na as u64;
    }
}

fn read_full(f: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

/// Ferramentas de diff gráficas conhecidas, na ordem de preferência.
const KNOWN: &[(&str, &[&str])] =
    &[("meld", &[]), ("kdiff3", &[]), ("kompare", &[]), ("WinMergeU", &[]), ("bcompare", &[]), ("opendiff", &[]), ("code", &["--diff"])];

fn in_path(program: &str) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ""] } else { &[""] };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|dir| exts.iter().map(move |e| dir.join(format!("{program}{e}"))))
        .find(|p| p.is_file())
}

/// Abre a ferramenta de diff com os dois arquivos; devolve o nome dela.
pub fn open_tool(a: &Path, b: &Path) -> Result<String, String> {
    if let Some(custom) = crate::session::setting("diff").filter(|s| !s.is_empty()) {
        Command::new(&custom).arg(a).arg(b).spawn().map_err(|e| format!("{custom}: {e}"))?;
        return Ok(custom);
    }
    for (name, args) in KNOWN {
        if let Some(exe) = in_path(name) {
            Command::new(exe).args(*args).arg(a).arg(b).spawn().map_err(|e| format!("{name}: {e}"))?;
            return Ok((*name).to_string());
        }
    }
    Err("nenhuma ferramenta de diff encontrada (instale Meld, WinMerge ou VS Code, ou defina diff = programa em settings)".into())
}

#[cfg(test)]
mod tests {
    use super::first_difference;

    #[test]
    fn acha_a_primeira_diferenca_e_tamanhos_diferentes() {
        let d = tempfile::tempdir().unwrap();
        let w = |n: &str, b: &[u8]| {
            let p = d.path().join(n);
            std::fs::write(&p, b).unwrap();
            p
        };
        let (a, b, c, e) = (w("a", b"abcdef"), w("b", b"abcdef"), w("c", b"abXdef"), w("e", b"abc"));
        assert_eq!(first_difference(&a, &b).unwrap(), None);
        assert_eq!(first_difference(&a, &c).unwrap(), Some(2));
        assert_eq!(first_difference(&a, &e).unwrap(), Some(3));
    }
}
