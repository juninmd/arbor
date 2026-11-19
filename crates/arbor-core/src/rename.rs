//! Renomear em lote: transforma um padrão em pares (origem, destino) e confere tudo antes de tocar no disco.
//!
//! Dois modos:
//! * **Modelo**: `Foto {n:3}` vira `Foto 001.jpg`, `Foto 002.jpg`... Marcadores: `{name}` (nome sem extensão),
//!   `{upper}` / `{lower}` (o nome em maiúsculas / minúsculas), `{ext}` (extensão), `{n}` (contador a partir
//!   de 1; `{n:3}` completa com zeros), `{date}` (data de modificação, `AAAA-MM-DD`). Se o modelo não usa
//!   `{ext}`, a extensão original é mantida.
//! * **Trocar texto**: `antigo => novo` troca todas as ocorrências no nome inteiro.

use chrono::{DateTime, Local};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Um item a renomear: caminho atual e data de modificação (para `{date}`).
#[derive(Debug, Clone)]
pub struct Item {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
    pub is_dir: bool,
}

/// Calcula os novos nomes. Itens cujo nome não muda ficam de fora. Erro se algum nome for inválido,
/// repetido ou já existir na pasta (fora do próprio lote).
pub fn plan(pattern: &str, items: &[Item]) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err("Escreva um modelo, como \"Foto {n:3}\", ou \"antigo => novo\"".into());
    }
    let replace = pattern.split_once("=>").map(|(a, b)| (a.trim(), b.trim()));
    if let Some((from, _)) = replace {
        if from.is_empty() {
            return Err("Diga o texto a trocar antes de =>".into());
        }
    }
    let mut pairs = Vec::new();
    let mut targets = HashSet::new();
    let sources: HashSet<&Path> = items.iter().map(|i| i.path.as_path()).collect();
    for (k, item) in items.iter().enumerate() {
        let name = file_name(&item.path);
        let new = match replace {
            Some((from, to)) => name.replace(from, to),
            None => render(pattern, item, &name, k + 1)?,
        };
        let new = new.trim().to_string();
        validate(&new)?;
        let to = item.path.with_file_name(&new);
        if !targets.insert(to.clone()) {
            return Err(format!("Dois itens ficariam com o nome \"{new}\"; use {{n}} no modelo"));
        }
        if to == item.path {
            continue;
        }
        let case_only = name.to_lowercase() == new.to_lowercase();
        if !case_only && !sources.contains(to.as_path()) && std::fs::symlink_metadata(&to).is_ok() {
            return Err(format!("\"{new}\" já existe nesta pasta"));
        }
        pairs.push((item.path.clone(), to));
    }
    Ok(pairs)
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn split(name: &str, is_dir: bool) -> (&str, Option<&str>) {
    if is_dir {
        return (name, None);
    }
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => (&name[..i], Some(&name[i + 1..])),
        _ => (name, None),
    }
}

fn render(pattern: &str, item: &Item, name: &str, n: usize) -> Result<String, String> {
    let (stem, ext) = split(name, item.is_dir);
    let mut out = String::new();
    let mut used_ext = false;
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            return Err("Falta fechar uma chave }".into());
        };
        let token = &rest[open + 1..open + close];
        match token {
            "name" => out.push_str(stem),
            "upper" => out.push_str(&stem.to_uppercase()),
            "lower" => out.push_str(&stem.to_lowercase()),
            "ext" => {
                used_ext = true;
                out.push_str(ext.unwrap_or_default());
            }
            "date" => {
                let date = item.modified.map(|t| DateTime::<Local>::from(t).format("%Y-%m-%d").to_string());
                out.push_str(&date.unwrap_or_else(|| "sem-data".into()));
            }
            "n" => out.push_str(&n.to_string()),
            t if t.starts_with("n:") => {
                let w: usize = t[2..].parse().map_err(|_| format!("{{{t}}}: use um número, como {{n:3}}"))?;
                out.push_str(&format!("{n:0w$}"));
            }
            other => {
                return Err(format!(
                    "Marcador desconhecido {{{other}}}. Use {{name}}, {{n}}, {{n:3}}, {{ext}}, {{date}}, {{upper}} ou {{lower}}"
                ))
            }
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    if !used_ext {
        if let Some(ext) = ext {
            out.push('.');
            out.push_str(ext);
        }
    }
    Ok(out)
}

fn validate(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." {
        return Err("O nome não pode ficar vazio".into());
    }
    if name.contains(['/', '\\']) {
        return Err("O nome não pode ter barras".into());
    }
    #[cfg(windows)]
    if name.contains(['<', '>', ':', '"', '|', '?', '*']) {
        return Err("O Windows não aceita < > : \" | ? * em nomes".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(dir: &Path, names: &[&str]) -> Vec<Item> {
        names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, "").unwrap();
                Item { path: p, modified: None, is_dir: false }
            })
            .collect()
    }

    fn names(pairs: &[(PathBuf, PathBuf)]) -> Vec<String> {
        pairs.iter().map(|(_, to)| file_name(to)).collect()
    }

    #[test]
    fn modelo_com_contador_mantem_extensao() {
        let d = tempfile::tempdir().unwrap();
        let it = items(d.path(), &["IMG_9.JPG", "IMG_10.jpg", "nota"]);
        assert_eq!(names(&plan("Foto {n:3}", &it).unwrap()), ["Foto 001.JPG", "Foto 002.jpg", "Foto 003"]);
        assert_eq!(names(&plan("{lower}.{ext}", &it[..1]).unwrap()), ["img_9.JPG"]);
        assert_eq!(names(&plan("{n}-{name}", &it[..2]).unwrap()), ["1-IMG_9.JPG", "2-IMG_10.jpg"]);
    }

    #[test]
    fn trocar_texto() {
        let d = tempfile::tempdir().unwrap();
        let it = items(d.path(), &["a_b_c.txt", "x.txt"]);
        // "x.txt" não muda e fica de fora.
        assert_eq!(names(&plan("_ => -", &it).unwrap()), ["a-b-c.txt"]);
    }

    #[test]
    fn recusa_nomes_repetidos_e_existentes() {
        let d = tempfile::tempdir().unwrap();
        let it = items(d.path(), &["a.txt", "b.txt"]);
        assert!(plan("igual", &it).unwrap_err().contains("{n}"));
        std::fs::write(d.path().join("c.txt"), "").unwrap();
        assert!(plan("c", &it[..1]).unwrap_err().contains("já existe"));
        assert!(plan("{oops}", &it).is_err());
        assert!(plan("a/b {n}", &it).is_err());
    }

    #[test]
    fn troca_dentro_do_lote_e_permitida() {
        let d = tempfile::tempdir().unwrap();
        let it = items(d.path(), &["1.txt", "2.txt"]);
        // 1→2 e 2→3: "2.txt" existe, mas é do próprio lote.
        let pairs = plan("{n}", &[it[1].clone(), it[0].clone()]).unwrap();
        assert_eq!(names(&pairs), ["1.txt", "2.txt"]);
    }
}
