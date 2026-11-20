//! Navegação sincronizada no painel duplo (Directory Opus, XYplorer): entrar numa subpasta de um lado entra
//! na subpasta de mesmo nome do outro, e subir sobe os dois. Útil para comparar duas cópias da mesma árvore.

use super::{App, Message};
use iced::Task;
use std::path::{Path, PathBuf};

/// Para onde o outro painel vai quando o ativo sai de `old` para `new`: só acompanha descer um nível
/// (se a subpasta existir do outro lado) e subir um nível; saltos maiores não têm par óbvio.
pub fn mirror(old: &Path, new: &Path, other: &Path, is_dir: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if new.parent() == Some(old) {
        let target = other.join(new.file_name()?);
        is_dir(&target).then_some(target)
    } else if old.parent() == Some(new) {
        other.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

impl App {
    pub(super) fn toggle_sync_nav(&mut self) -> Task<Message> {
        self.sync_nav = !self.sync_nav;
        if self.sync_nav && !self.dual {
            self.dual = true;
        }
        let text =
            if self.sync_nav { "Navegação sincronizada: os dois painéis andam juntos" } else { "Navegação sincronizada desligada" };
        self.notify(text, false)
    }

    /// Chamado depois que o painel `pane` saiu de `old` para `new`.
    pub(super) fn follow(&mut self, pane: usize, old: &Path, new: &Path) -> Task<Message> {
        if !self.sync_nav || !self.dual {
            return Task::none();
        }
        let other = 1 - pane;
        let other_path = self.panes[other].tab().path.clone();
        match mirror(old, new, &other_path, Path::is_dir) {
            Some(target) => self.navigate_one(other, target),
            None => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mirror;
    use std::path::{Path, PathBuf};

    #[test]
    fn desce_so_se_a_subpasta_existir_e_sobe_junto() {
        let exists = |p: &Path| p == Path::new("/b/src");
        let down = mirror(Path::new("/a"), Path::new("/a/src"), Path::new("/b"), exists);
        assert_eq!(down, Some(PathBuf::from("/b/src")));
        assert_eq!(mirror(Path::new("/a"), Path::new("/a/docs"), Path::new("/b"), exists), None);
        assert_eq!(mirror(Path::new("/a/src"), Path::new("/a"), Path::new("/b/src"), exists), Some(PathBuf::from("/b")));
        // Saltos (barra lateral, endereço) não arrastam o outro painel.
        assert_eq!(mirror(Path::new("/a"), Path::new("/x/y"), Path::new("/b"), exists), None);
    }
}
