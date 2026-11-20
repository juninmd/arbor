//! Selecionar por máscara, como o Num+ / Num− do Total Commander: `*.jpg;*.png` soma ou tira da seleção
//! os itens visíveis cujo nome casa. "Selecionar mesmo tipo" usa a extensão do item sob o cursor.

use super::{App, Message, PromptKind};
use arbor_core::search::Mask;
use iced::Task;

/// Máscara sugerida ao abrir o prompt: a extensão do item sob o cursor, ou tudo.
pub fn suggested(name: Option<&str>) -> String {
    match name.and_then(|n| n.rsplit_once('.')).filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty()) {
        Some((_, ext)) => format!("*.{ext}"),
        None => "*".into(),
    }
}

impl App {
    pub(super) fn open_mask_prompt(&mut self, kind: PromptKind) -> Task<Message> {
        let value = suggested(self.tab().current().map(|e| e.name.as_str()));
        self.open_prompt(kind, value)
    }

    /// Soma (`add`) ou tira da seleção os itens visíveis que casam com `mask`; devolve quantos casaram.
    pub(super) fn select_mask(&mut self, mask: &str, add: bool) -> Option<usize> {
        let mask = Mask::parse(mask)?;
        let tab = self.tab_mut();
        let hits: Vec<_> = tab.view.iter().map(|&i| &tab.entries[i]).filter(|e| mask.matches(&e.name)).map(|e| e.path.clone()).collect();
        let n = hits.len();
        for p in hits {
            if add {
                tab.selected.insert(p);
            } else {
                tab.selected.remove(&p);
            }
        }
        Some(n)
    }

    pub(super) fn submit_mask(&mut self, kind: PromptKind, value: &str) -> Task<Message> {
        let add = kind == PromptKind::SelectMask;
        match self.select_mask(value, add) {
            Some(0) => self.notify(format!("Nenhum item casa com {value}"), false),
            Some(_) | None => Task::none(),
        }
    }

    pub(super) fn select_same_kind(&mut self) -> Task<Message> {
        let Some(name) = self.tab().current().map(|e| e.name.clone()) else { return Task::none() };
        let mask = match suggested(Some(&name)).as_str() {
            "*" => name,
            m => m.to_string(),
        };
        self.select_mask(&mask, true);
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::suggested;

    #[test]
    fn mascara_sugerida_pela_extensao() {
        assert_eq!(suggested(Some("foto.JPG")), "*.JPG");
        assert_eq!(suggested(Some("arquivo.tar.gz")), "*.gz");
        // Nome sem extensão, ou só com ponto inicial (oculto no Unix), não tem tipo.
        assert_eq!(suggested(Some("Makefile")), "*");
        assert_eq!(suggested(Some(".bashrc")), "*");
        assert_eq!(suggested(None), "*");
    }
}
