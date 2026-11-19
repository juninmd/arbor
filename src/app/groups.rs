//! Linhas da lista em Detalhes com "Agrupar por": um cabeçalho antes de cada grupo. Sem agrupamento as
//! linhas são os próprios itens e nada disto é alocado.

use arbor_core::group::{group_of, today, Group, GroupBy};
use arbor_core::Entry;

pub enum Row {
    Header {
        label: String,
        count: usize,
    },
    /// Índice em `Tab::view`.
    Item(usize),
}

#[derive(Default)]
pub struct Grouped {
    pub rows: Vec<Row>,
    /// Linha de cada item de `view`.
    pub row_of: Vec<usize>,
}

/// Reordena `view` por grupo, estável para manter a ordenação escolhida dentro de cada grupo.
pub fn build(view: &mut [usize], entries: &[Entry], by: GroupBy) -> Grouped {
    let today = today();
    let mut keyed: Vec<(Group, usize)> = view.iter().map(|&i| (group_of(&entries[i], by, today), i)).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = Grouped { rows: Vec::with_capacity(keyed.len() + 8), row_of: Vec::with_capacity(keyed.len()) };
    let mut header = 0;
    for (k, (g, i)) in keyed.iter().enumerate() {
        if k == 0 || keyed[k - 1].0 != *g {
            header = out.rows.len();
            out.rows.push(Row::Header { label: g.label.clone(), count: 0 });
        }
        if let Row::Header { count, .. } = &mut out.rows[header] {
            *count += 1;
        }
        view[k] = *i;
        out.row_of.push(out.rows.len());
        out.rows.push(Row::Item(k));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_get_their_own_header_and_items_keep_sort_inside_groups() {
        let entries = vec![Entry::synthetic("b.txt", false, 1), Entry::synthetic("pasta", true, 0), Entry::synthetic("a.txt", false, 1)];
        // Ordem por nome vinda da classificação: a.txt, b.txt, pasta.
        let mut view = vec![2, 0, 1];
        let g = build(&mut view, &entries, GroupBy::Kind);
        assert_eq!(view, [1, 2, 0], "pasta primeiro, depois os .txt na ordem por nome");
        let labels: Vec<String> = g
            .rows
            .iter()
            .map(|r| match r {
                Row::Header { label, count } => format!("{label} ({count})"),
                Row::Item(i) => entries[view[*i]].name.clone(),
            })
            .collect();
        assert_eq!(labels, ["Pasta de arquivos (1)", "pasta", "Arquivo TXT (2)", "a.txt", "b.txt"]);
        assert_eq!(g.row_of, [1, 3, 4]);
    }

    #[test]
    fn empty_view_has_no_rows() {
        let g = build(&mut [], &[], GroupBy::Size);
        assert!(g.rows.is_empty() && g.row_of.is_empty());
    }
}
