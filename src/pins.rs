//! Pastas fixadas na barra lateral (o "Acesso rápido" do Explorer), uma por linha em `~/.config/arbor/pins`.

use std::path::{Path, PathBuf};

/// Alvo virtual da área "Solte para fixar" da barra lateral durante um arrasto.
pub const DROP: &str = "arbor:pin";

pub fn drop_target() -> PathBuf {
    PathBuf::from(DROP)
}

pub fn is_drop(p: &Path) -> bool {
    p.as_os_str() == DROP
}

/// Fixa as pastas ainda não fixadas, na ordem do arrasto; devolve quantas entraram.
pub fn pin_all(pins: &mut Vec<PathBuf>, dirs: &[PathBuf]) -> usize {
    let before = pins.len();
    for d in dirs {
        if !pins.contains(d) {
            pins.push(d.clone());
        }
    }
    pins.len() - before
}

pub fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor").join("pins"))
}

pub fn load(file: Option<&Path>) -> Vec<PathBuf> {
    let text = file.and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    let mut out: Vec<PathBuf> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let p = PathBuf::from(line);
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

pub fn save(file: Option<&Path>, pins: &[PathBuf]) -> Result<(), String> {
    let file = file.ok_or("sem pasta de configuração")?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text: String = pins.iter().map(|p| format!("{}\n", p.to_string_lossy())).collect();
    std::fs::write(file, text).map_err(|e| e.to_string())
}

/// Fixa ou desafixa `path`; devolve `true` se ficou fixado.
pub fn toggle(pins: &mut Vec<PathBuf>, path: &Path) -> bool {
    if let Some(i) = pins.iter().position(|p| p == path) {
        pins.remove(i);
        false
    } else {
        pins.push(path.to_path_buf());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixa_salva_e_recarrega_sem_duplicar() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("sub").join("pins");
        let mut pins = load(Some(&f));
        assert!(pins.is_empty(), "arquivo ausente é lista vazia");
        assert!(toggle(&mut pins, Path::new("/tmp/ação")));
        assert!(toggle(&mut pins, Path::new("/srv")));
        save(Some(&f), &pins).unwrap();
        std::fs::write(&f, std::fs::read_to_string(&f).unwrap() + "/srv\n\n").unwrap();
        assert_eq!(load(Some(&f)), [PathBuf::from("/tmp/ação"), PathBuf::from("/srv")]);
        assert!(!toggle(&mut pins, Path::new("/srv")));
        assert_eq!(pins, [PathBuf::from("/tmp/ação")]);
    }

    #[test]
    fn arrastar_fixa_so_o_que_falta_e_o_alvo_e_virtual() {
        let mut pins = vec![PathBuf::from("/a")];
        let n = pin_all(&mut pins, &[PathBuf::from("/b"), PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(n, 1);
        assert_eq!(pins, [PathBuf::from("/a"), PathBuf::from("/b")]);
        assert!(is_drop(&drop_target()) && !is_drop(Path::new("/a")));
    }
}
