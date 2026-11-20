//! Criar e conferir somas SHA-256 no formato do `sha256sum` (`<hex>  nome`), como o "Criar/verificar CRC" do
//! Total Commander. Um arquivo vira `nome.sha256`; vários viram `checksums.sha256` na pasta.

use crate::props::sha256;
use std::path::{Path, PathBuf};

/// Onde gravar as somas de `files` (todos da mesma pasta `dir`).
pub fn target(dir: &Path, files: &[PathBuf]) -> PathBuf {
    match files {
        [one] => {
            let mut name = one.file_name().unwrap_or_default().to_os_string();
            name.push(".sha256");
            dir.join(name)
        }
        _ => dir.join("checksums.sha256"),
    }
}

/// Calcula e grava; devolve o arquivo criado. Nomes ficam relativos a `dir` para a lista ir junto com a pasta.
pub fn create(dir: &Path, files: &[PathBuf]) -> Result<PathBuf, String> {
    let mut out = String::new();
    for f in files.iter().filter(|f| f.is_file()) {
        let rel = f.strip_prefix(dir).unwrap_or(f);
        out.push_str(&format!("{}  {}\n", sha256(f)?, rel.to_string_lossy().replace('\\', "/")));
    }
    if out.is_empty() {
        return Err("selecione arquivos (pastas não entram)".into());
    }
    let dest = target(dir, files);
    std::fs::write(&dest, out).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Linhas `<hex>  nome` ou `<hex> *nome` (modo binário); o resto é ignorado.
fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let (hex, name) = l.trim_end().split_once(' ')?;
            let name = name.strip_prefix(' ').or_else(|| name.strip_prefix('*')).unwrap_or(name);
            (hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) && !name.is_empty())
                .then(|| (hex.to_lowercase(), name.to_string()))
        })
        .collect()
}

/// Confere cada arquivo listado em `sums` (caminhos relativos à pasta dele) e resume o resultado.
pub fn verify(sums: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(sums).map_err(|e| e.to_string())?;
    let entries = parse(&text);
    if entries.is_empty() {
        return Err("nenhuma soma SHA-256 no arquivo".into());
    }
    let dir = sums.parent().unwrap_or(Path::new("."));
    let mut bad = vec![];
    for (hex, name) in &entries {
        match sha256(&dir.join(name)) {
            Ok(h) if &h == hex => {}
            Ok(_) => bad.push(format!("{name} (diferente)")),
            Err(_) => bad.push(format!("{name} (faltando)")),
        }
    }
    let ok = entries.len() - bad.len();
    if bad.is_empty() {
        Ok(format!("{ok} arquivo(s) conferem"))
    } else {
        Err(format!("{ok} ok, {} com problema: {}", bad.len(), bad.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cria_confere_e_acusa_alteracao() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a.txt"), d.path().join("b.txt"));
        std::fs::write(&a, "abc").unwrap();
        std::fs::write(&b, "x").unwrap();
        let one = create(d.path(), std::slice::from_ref(&a)).unwrap();
        assert_eq!(one, d.path().join("a.txt.sha256"));
        assert!(std::fs::read_to_string(&one)
            .unwrap()
            .starts_with("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  a.txt"));
        let many = create(d.path(), &[a.clone(), b.clone()]).unwrap();
        assert_eq!(verify(&many).unwrap(), "2 arquivo(s) conferem");
        std::fs::write(&b, "mudou").unwrap();
        std::fs::remove_file(&a).unwrap();
        let err = verify(&many).unwrap_err();
        assert!(err.contains("a.txt (faltando)") && err.contains("b.txt (diferente)"), "{err}");
    }

    #[test]
    fn le_modo_binario_e_ignora_lixo() {
        let h = "a".repeat(64);
        let p = parse(&format!("{h} *foto.jpg\n# comentário\nxyz  nada\n"));
        assert_eq!(p, [(h, "foto.jpg".to_string())]);
    }
}
