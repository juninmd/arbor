//! "Propriedades" de um item para o painel de detalhes: tipo, local, datas, atributos e permissões.

use arbor_core::{human_size, human_time};
use std::path::Path;

pub fn props(path: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(link) = std::fs::symlink_metadata(path) else { return vec![("Erro".into(), "não foi possível ler".into())] };
    let meta = std::fs::metadata(path).unwrap_or_else(|_| link.clone());
    let kind = if link.file_type().is_symlink() {
        "Atalho (link simbólico)".to_string()
    } else if meta.is_dir() {
        "Pasta".to_string()
    } else {
        match path.extension() {
            Some(e) => format!("Arquivo {}", e.to_string_lossy().to_uppercase()),
            None => "Arquivo".to_string(),
        }
    };
    out.push(("Tipo".into(), kind));
    if link.file_type().is_symlink() {
        if let Ok(target) = std::fs::read_link(path) {
            out.push(("Destino".into(), target.display().to_string()));
        }
    }
    if let Some(parent) = path.parent() {
        out.push(("Local".into(), parent.display().to_string()));
    }
    if !meta.is_dir() {
        out.push(("Tamanho".into(), format!("{} ({} bytes)", human_size(meta.len()), arbor_core::human_count(meta.len() as usize))));
    }
    for (label, t) in [("Criado", meta.created().ok()), ("Modificado", meta.modified().ok()), ("Acessado", meta.accessed().ok())] {
        if t.is_some() {
            out.push((label.into(), human_time(t)));
        }
    }
    out.push(("Somente leitura".into(), if meta.permissions().readonly() { "Sim" } else { "Não" }.into()));
    attributes(&meta, &mut out);
    out
}

#[cfg(unix)]
fn attributes(meta: &std::fs::Metadata, out: &mut Vec<(String, String)>) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let mode = meta.permissions().mode();
    let bits: String = (0..9).rev().map(|i| if mode & (1 << i) == 0 { '-' } else { b"xwr"[i % 3] as char }).collect();
    out.push(("Permissões".into(), format!("{bits} ({:o})", mode & 0o777)));
    out.push(("Dono".into(), format!("uid {} · gid {}", meta.uid(), meta.gid())));
}

#[cfg(windows)]
fn attributes(meta: &std::fs::Metadata, out: &mut Vec<(String, String)>) {
    use std::os::windows::fs::MetadataExt;
    let a = meta.file_attributes();
    let names = [(0x2, "oculto"), (0x4, "sistema"), (0x20, "arquivo"), (0x800, "compactado"), (0x4000, "criptografado")];
    let set: Vec<&str> = names.iter().filter(|(bit, _)| a & bit != 0).map(|(_, n)| *n).collect();
    if !set.is_empty() {
        out.push(("Atributos".into(), set.join(", ")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(p: &'a [(String, String)], k: &str) -> Option<&'a str> {
        p.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str())
    }

    #[test]
    fn arquivo_tem_tipo_tamanho_e_local() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("nota.txt");
        std::fs::write(&f, vec![b'a'; 2048]).unwrap();
        let p = props(&f);
        assert_eq!(get(&p, "Tipo"), Some("Arquivo TXT"));
        assert_eq!(get(&p, "Tamanho"), Some("2.0 KB (2.048 bytes)"));
        assert_eq!(get(&p, "Local"), Some(dir.path().to_string_lossy().as_ref()));
        assert_eq!(get(&p, "Somente leitura"), Some("Não"));
    }

    #[test]
    fn pasta_sem_tamanho_e_caminho_inexistente_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let p = props(dir.path());
        assert_eq!(get(&p, "Tipo"), Some("Pasta"));
        assert!(get(&p, "Tamanho").is_none());
        assert!(get(&props(&dir.path().join("x")), "Erro").is_some());
    }
}
