//! Pastas fixadas na barra lateral (o "Acesso rápido" do Explorer), uma por linha em `~/.config/arbor/pins`.

use std::path::{Path, PathBuf};

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
}
