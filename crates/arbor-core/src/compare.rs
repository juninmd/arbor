//! Comparar duas pastas lado a lado, como o "Comparar pastas" do Total Commander: diz, para cada lado, o que
//! só existe nele ou é mais novo nele. Compara um nível só (subpastas presentes nos dois lados contam como
//! iguais) e não lê conteúdo: tamanho e data bastam e cabem em milissegundos.

use crate::entry::Entry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// FAT e alguns compartilhamentos guardam a data com 2 s de resolução: menos que isso é "mesma hora".
const SLACK: Duration = Duration::from_secs(2);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Itens da esquerda que faltam à direita ou são mais novos (ou diferem no tamanho).
    pub left: Vec<PathBuf>,
    pub right: Vec<PathBuf>,
    pub same: usize,
    /// Mesmo nome, mas pasta de um lado e arquivo do outro (caminho da esquerda). Nenhum lado "ganha":
    /// sincronizar mandaria uma pasta inteira para a lixeira para pôr um arquivo no lugar (ou o contrário).
    pub kind_clash: Vec<PathBuf>,
}

/// Windows e macOS não diferenciam maiúsculas nos nomes; Linux diferencia.
fn key(name: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        name.to_lowercase()
    } else {
        name.to_string()
    }
}

/// Qual lado ganha: `Some(true)` esquerda, `Some(false)` direita, `None` iguais. Mesma hora com tamanho
/// diferente marca os dois, porque não dá para saber qual é o certo.
fn newer(a: Option<SystemTime>, b: Option<SystemTime>) -> Option<bool> {
    match (a, b) {
        (Some(a), Some(b)) if a > b + SLACK => Some(true),
        (Some(a), Some(b)) if b > a + SLACK => Some(false),
        _ => None,
    }
}

pub fn compare(left: &[Entry], right: &[Entry]) -> Diff {
    let others: HashMap<String, &Entry> = right.iter().map(|e| (key(&e.name), e)).collect();
    let mut diff = Diff::default();
    let mut seen = std::collections::HashSet::new();
    for l in left {
        let k = key(&l.name);
        let Some(r) = others.get(&k) else {
            diff.left.push(l.path.clone());
            continue;
        };
        seen.insert(k);
        if l.is_dir && r.is_dir {
            diff.same += 1;
            continue;
        }
        if l.is_dir != r.is_dir {
            diff.kind_clash.push(l.path.clone());
            continue;
        }
        match newer(l.modified, r.modified) {
            Some(true) => diff.left.push(l.path.clone()),
            Some(false) => diff.right.push(r.path.clone()),
            None if l.size != r.size => {
                diff.left.push(l.path.clone());
                diff.right.push(r.path.clone());
            }
            None => diff.same += 1,
        }
    }
    diff.right.extend(right.iter().filter(|r| !seen.contains(&key(&r.name))).map(|r| r.path.clone()));
    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn list(dir: &std::path::Path) -> Vec<Entry> {
        let mut v: Vec<Entry> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|d| Entry::new(d.file_name().to_string_lossy().into_owned(), d.path(), &d.metadata().unwrap(), false))
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    fn names(v: &[PathBuf]) -> Vec<String> {
        let mut n: Vec<String> = v.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        n.sort();
        n
    }

    #[test]
    fn so_de_um_lado_mais_novo_e_tamanho_diferente() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let old = SystemTime::now() - Duration::from_secs(3600);
        for (dir, name, body, when) in [
            (a.path(), "igual.txt", "x", old),
            (b.path(), "igual.txt", "x", old),
            (a.path(), "novo.txt", "v2", SystemTime::now()),
            (b.path(), "novo.txt", "v1", old),
            (a.path(), "tam.txt", "aa", old),
            (b.path(), "tam.txt", "a", old),
            (a.path(), "so-esq.txt", "", old),
            (b.path(), "so-dir.txt", "", old),
        ] {
            let p = dir.join(name);
            fs::write(&p, body).unwrap();
            fs::File::options().write(true).open(&p).unwrap().set_modified(when).unwrap();
        }
        fs::create_dir(a.path().join("pasta")).unwrap();
        fs::create_dir(b.path().join("pasta")).unwrap();
        let d = compare(&list(a.path()), &list(b.path()));
        assert_eq!(names(&d.left), ["novo.txt", "so-esq.txt", "tam.txt"]);
        assert_eq!(names(&d.right), ["so-dir.txt", "tam.txt"]);
        assert_eq!(d.same, 2);
    }

    #[test]
    fn pasta_e_arquivo_com_o_mesmo_nome_nao_entram_na_copia() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        fs::write(a.path().join("dados"), "arquivo novo").unwrap();
        fs::create_dir(b.path().join("dados")).unwrap();
        fs::write(b.path().join("dados/importante.txt"), "x").unwrap();
        let old = SystemTime::now() - Duration::from_secs(3600);
        fs::File::options().write(true).open(b.path().join("dados/importante.txt")).unwrap().set_modified(old).unwrap();
        let d = compare(&list(a.path()), &list(b.path()));
        // Antes, o arquivo mais novo da esquerda entrava em `left` e "Sincronizar" trocava a pasta por ele.
        assert!(d.left.is_empty() && d.right.is_empty(), "{d:?}");
        assert_eq!(names(&d.kind_clash), ["dados"]);
        assert_eq!(d.same, 0);
    }

    #[test]
    fn diferenca_de_um_segundo_e_a_mesma_hora() {
        let t = SystemTime::now();
        assert_eq!(newer(Some(t + Duration::from_secs(1)), Some(t)), None);
        assert_eq!(newer(Some(t + Duration::from_secs(3)), Some(t)), Some(true));
        assert_eq!(newer(None, Some(t)), None);
    }
}
