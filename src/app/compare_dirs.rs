//! Comparar e sincronizar as pastas dos dois painéis (Total Commander, Directory Opus, FreeCommander).
//!
//! "Comparar pastas" seleciona, em cada lado, o que só existe nele ou é mais novo; daí F5 copia para o outro.
//! "Sincronizar com o outro painel" faz isso de uma vez no sentido painel ativo → outro, sem apagar nada,
//! depois de confirmar (as versões antigas do outro lado são substituídas).

use super::trash_view::Irreversible;
use super::{App, Message, Overlay};
use arbor_core::compare::{compare, Diff};
use arbor_core::ops::{self, Conflict, Op};
use arbor_core::Entry;
use iced::Task;

fn visible(entries: &[Entry], show_hidden: bool) -> Vec<Entry> {
    entries.iter().filter(|e| show_hidden || !e.hidden).cloned().collect()
}

impl App {
    /// `None` com aviso quando não há duas pastas comuns lado a lado.
    fn diff_panes(&mut self) -> Result<Diff, Task<Message>> {
        if !self.dual {
            self.dual = true;
            return Err(self.notify("Painel duplo aberto: escolha a outra pasta e compare de novo", false));
        }
        let (l, r) = (self.panes[0].tab(), self.panes[1].tab());
        let comparable = |t: &super::Tab| !t.loading && t.search.is_none() && !super::computer::is_virtual(&t.path);
        if !comparable(l) || !comparable(r) {
            return Err(self.notify("Compare duas pastas comuns, já carregadas", false));
        }
        if l.path == r.path {
            return Err(self.notify("Os dois painéis estão na mesma pasta", false));
        }
        Ok(compare(&visible(&l.entries, self.show_hidden), &visible(&r.entries, self.show_hidden)))
    }

    pub(super) fn compare_dirs(&mut self) -> Task<Message> {
        let diff = match self.diff_panes() {
            Ok(d) => d,
            Err(task) => return task,
        };
        let (nl, nr, same, clash) = (diff.left.len(), diff.right.len(), diff.same, diff.kind_clash.len());
        self.panes[0].tab_mut().selected = diff.left.into_iter().collect();
        self.panes[1].tab_mut().selected = diff.right.into_iter().collect();
        let mut text = if nl + nr + clash == 0 {
            format!("Pastas iguais ({same} itens)")
        } else {
            format!("Esquerda: {nl} diferente(s) · Direita: {nr} diferente(s) · {same} igual(is). F5 copia a seleção")
        };
        if clash > 0 {
            text.push_str(&format!(" · {clash} nome(s) são pasta de um lado e arquivo do outro: resolva à mão"));
        }
        self.notify(text, false)
    }

    pub(super) fn sync_dirs(&mut self) -> Task<Message> {
        let diff = match self.diff_panes() {
            Ok(d) => d,
            Err(task) => return task,
        };
        let sources = if self.active == 0 { diff.left } else { diff.right };
        let dest = self.panes[self.other_pane()].tab().path.clone();
        if sources.is_empty() {
            return self.notify(format!("{} já tem tudo desta pasta", ops::name(&dest)), false);
        }
        let text = format!(
            "Copiar {} item(ns) novos ou mais recentes para \"{}\"? As versões antigas de lá serão substituídas; nada é apagado.",
            sources.len(),
            ops::name(&dest)
        );
        self.overlay = Overlay::Confirm { text, action: Irreversible::Sync { sources, dest } };
        Task::none()
    }

    pub(super) fn start_sync(&mut self, sources: Vec<std::path::PathBuf>, dest: std::path::PathBuf) -> Task<Message> {
        self.start(Op::Copy { sources, dest, conflict: Conflict::Replace })
    }
}
