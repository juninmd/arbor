//! Autocompletar da barra de endereço (Ctrl+L): subpastas que começam com o que foi digitado.

use std::path::{Path, PathBuf, MAIN_SEPARATOR};

const MAX: usize = 8;

/// `input` como o usuário digitou (aceita `~` e caminho relativo a `base`). Devolve até 8 pastas.
pub fn suggestions(input: &str, base: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let expanded = match (input.strip_prefix('~'), home) {
        (Some(rest), Some(h)) => h.join(rest.trim_start_matches(['/', '\\'])),
        _ => PathBuf::from(input),
    };
    let full = if expanded.is_relative() { base.join(&expanded) } else { expanded };
    let ends_with_sep = input.is_empty() || input.ends_with(['/', '\\']);
    let (dir, prefix) = if ends_with_sep {
        (full.as_path(), String::new())
    } else {
        match (full.parent(), full.file_name()) {
            (Some(d), Some(n)) => (d, n.to_string_lossy().to_lowercase()),
            _ => return vec![],
        }
    };
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<(Box<[u8]>, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            let hidden_ok = !name.starts_with('.') || prefix.starts_with('.');
            (hidden_ok && lower.starts_with(&prefix) && e.path().is_dir()).then(|| (arbor_core::entry::natural_key(&name), e.path()))
        })
        .collect();
    out.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().take(MAX).map(|(_, p)| p).collect()
}

/// Texto que vai para a caixa ao aceitar a sugestão: o caminho com a barra no fim, pronto para continuar.
pub fn accept(path: &Path) -> String {
    let mut s = path.to_string_lossy().into_owned();
    if !s.ends_with(MAIN_SEPARATOR) {
        s.push(MAIN_SEPARATOR);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sugere_subpastas_pelo_prefixo_sem_arquivos_nem_ocultas() {
        let tmp = tempfile::tempdir().unwrap();
        for d in ["Documentos", "docs", "Downloads", ".dotfiles", "música"] {
            std::fs::create_dir(tmp.path().join(d)).unwrap();
        }
        std::fs::write(tmp.path().join("doc.txt"), "").unwrap();
        let base = tmp.path();
        let names = |v: Vec<PathBuf>| v.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();
        assert_eq!(names(suggestions("do", base, None)), ["docs", "Documentos", "Downloads"]);
        assert_eq!(names(suggestions(".d", base, None)), [".dotfiles"]);
        assert_eq!(names(suggestions("MÚ", base, None)), ["música"]);
        let abs = format!("{}{}", base.display(), MAIN_SEPARATOR);
        assert_eq!(suggestions(&abs, base, None).len(), 4, "com barra no fim lista tudo, menos ocultas");
        assert_eq!(names(suggestions("~/Dow", Path::new("/"), Some(base))), ["Downloads"]);
        assert!(suggestions("nao-existe/x", base, None).is_empty());
    }

    #[test]
    fn aceitar_poe_barra_no_fim() {
        assert!(accept(Path::new("/a/b")).ends_with(MAIN_SEPARATOR));
        assert_eq!(accept(Path::new("/")), MAIN_SEPARATOR.to_string());
    }
}
