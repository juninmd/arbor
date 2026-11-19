//! Área de transferência do sistema para arquivos: o que se copia no arbor cola no Explorer, Finder ou
//! Nautilus, e vice-versa. O `Clipboard` fica vivo no app porque no X11 é ele quem serve o conteúdo.

use std::path::PathBuf;

#[derive(Default)]
pub struct SysClip(Option<arboard::Clipboard>);

impl SysClip {
    fn get(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.0.is_none() {
            self.0 = arboard::Clipboard::new().ok();
        }
        self.0.as_mut()
    }

    pub fn set_files(&mut self, files: &[PathBuf]) -> Result<(), String> {
        let c = self.get().ok_or("área de transferência indisponível")?;
        c.set().file_list(files).map_err(|e| e.to_string())
    }

    /// Arquivos que outro app copiou; vazio se a área tiver outra coisa (texto, imagem) ou falhar.
    pub fn files(&mut self) -> Vec<PathBuf> {
        self.get().and_then(|c| c.get().file_list().ok()).unwrap_or_default()
    }
}

/// O que colar: o recorte interno vale se o sistema ainda tem os mesmos arquivos (ou nada legível);
/// se outro app copiou arquivos depois, eles ganham, como cópia.
pub fn choose(internal: Option<&(bool, Vec<PathBuf>)>, system: Vec<PathBuf>) -> Option<(bool, Vec<PathBuf>)> {
    match internal {
        Some((cut, items)) if system.is_empty() || system == *items => Some((*cut, items.clone())),
        _ if !system.is_empty() => Some((false, system)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorte_interno_vale_ate_outro_app_copiar() {
        let a = vec![PathBuf::from("/a")];
        let b = vec![PathBuf::from("/b")];
        assert_eq!(choose(Some(&(true, a.clone())), vec![]), Some((true, a.clone())));
        assert_eq!(choose(Some(&(true, a.clone())), a.clone()), Some((true, a.clone())));
        assert_eq!(choose(Some(&(true, a.clone())), b.clone()), Some((false, b.clone())), "de fora é sempre cópia");
        assert_eq!(choose(None, b.clone()), Some((false, b)));
        assert_eq!(choose(None, vec![]), None);
    }
}
