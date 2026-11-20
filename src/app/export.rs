//! "Copiar listagem": os itens visíveis (ou os selecionados) como tabela separada por tabulação, que cola
//! direto em planilhas e documentos, como o "Imprimir/exportar pasta" do Directory Opus.

use super::{App, Message};
use arbor_core::{human_time, Entry};
use iced::Task;

/// Tabela com cabeçalho; tamanho em bytes para a planilha poder somar.
pub fn table<'a>(entries: impl Iterator<Item = &'a Entry>) -> String {
    let mut out = String::from("Nome\tTipo\tTamanho (bytes)\tModificado\tCaminho\n");
    for e in entries {
        let size = if e.is_dir && e.size == 0 { String::new() } else { e.size.to_string() };
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            clean(&e.name),
            crate::view::kind_label(e),
            size,
            human_time(e.modified),
            clean(&e.path.to_string_lossy())
        ));
    }
    out
}

impl App {
    pub(super) fn copy_listing(&mut self) -> Task<Message> {
        let tab = self.tab();
        let picked = !tab.selected.is_empty();
        let rows = tab.view.iter().map(|&i| &tab.entries[i]).filter(|e| !picked || tab.selected.contains(&e.path));
        let text = table(rows);
        let n = text.lines().count() - 1;
        Task::batch([iced::clipboard::write(text), self.notify(format!("Listagem de {n} item(ns) copiada (cola em planilhas)"), false)])
    }
}

#[cfg(test)]
mod tests {
    use super::table;
    use arbor_core::Entry;

    #[test]
    fn tabela_com_cabecalho_e_sem_tabulacao_nos_nomes() {
        let d = tempfile::tempdir().unwrap();
        // Tab is invalid in Windows file names: read metadata from a real file, name the entry with a tab.
        let real = d.path().join("ab.txt");
        std::fs::write(&real, "12345").unwrap();
        let e = Entry::new("a\tb.txt".into(), d.path().join("a\tb.txt"), &std::fs::metadata(&real).unwrap(), false);
        let t = table(std::iter::once(&e));
        let line = t.lines().nth(1).unwrap();
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 5);
        assert_eq!((cols[0], cols[1], cols[2]), ("a b.txt", "TXT", "5"));
    }
}
