//! "Agrupar por" do Explorer: cada entrada cai num grupo com rótulo; os grupos têm ordem fixa (datas e
//! tamanhos) ou alfabética (tipo).

use crate::Entry;
use chrono::{Datelike, Duration, Local, NaiveDate};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupBy {
    Kind,
    Modified,
    Size,
}

/// Grupo de uma entrada: ordena por `rank` e depois pelo rótulo.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Group {
    pub rank: u8,
    pub label: String,
}

fn group(rank: u8, label: impl Into<String>) -> Group {
    Group { rank, label: label.into() }
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn group_of(e: &Entry, by: GroupBy, today: NaiveDate) -> Group {
    match by {
        GroupBy::Kind if e.is_dir => group(0, "Pasta de arquivos"),
        GroupBy::Kind if e.ext.is_empty() => group(2, "Arquivo"),
        GroupBy::Kind => group(1, format!("Arquivo {}", e.ext.to_uppercase())),
        GroupBy::Size => size_group(e),
        GroupBy::Modified => match e.modified {
            Some(t) => date_group(chrono::DateTime::<Local>::from(t).date_naive(), today),
            None => group(8, "Data desconhecida"),
        },
    }
}

fn size_group(e: &Entry) -> Group {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    match e.size {
        _ if e.is_dir => group(0, "Pastas"),
        0 => group(1, "Vazio (0 KB)"),
        s if s <= 16 * KB => group(2, "Minúsculo (0 - 16 KB)"),
        s if s <= MB => group(3, "Pequeno (16 KB - 1 MB)"),
        s if s <= 128 * MB => group(4, "Médio (1 - 128 MB)"),
        s if s <= GB => group(5, "Grande (128 MB - 1 GB)"),
        s if s <= 4 * GB => group(6, "Enorme (1 - 4 GB)"),
        _ => group(7, "Gigantesco (> 4 GB)"),
    }
}

/// Faixas de data do Explorer; a semana começa no domingo, como no calendário brasileiro.
pub fn date_group(day: NaiveDate, today: NaiveDate) -> Group {
    let week_start = today - Duration::days(today.weekday().num_days_from_sunday() as i64);
    let (y, m) = (today.year(), today.month());
    let last_month = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
    if day >= today {
        group(0, "Hoje")
    } else if day == today - Duration::days(1) {
        group(1, "Ontem")
    } else if day >= week_start {
        group(2, "No início desta semana")
    } else if day >= week_start - Duration::days(7) {
        group(3, "Semana passada")
    } else if (day.year(), day.month()) == (y, m) {
        group(4, "No início deste mês")
    } else if (day.year(), day.month()) == last_month {
        group(5, "Mês passado")
    } else if day.year() == y {
        group(6, "No início deste ano")
    } else {
        group(7, "Há muito tempo")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn dates_fall_in_explorer_ranges() {
        // Quinta-feira, 19/11/2026: a semana começou no domingo, 15/11.
        let today = d(2026, 11, 19);
        let label = |day| date_group(day, today).label;
        assert_eq!(label(d(2026, 11, 19)), "Hoje");
        assert_eq!(label(d(2026, 11, 25)), "Hoje", "data no futuro (relógio adiantado) não some");
        assert_eq!(label(d(2026, 11, 18)), "Ontem");
        assert_eq!(label(d(2026, 11, 15)), "No início desta semana");
        assert_eq!(label(d(2026, 11, 14)), "Semana passada");
        assert_eq!(label(d(2026, 11, 8)), "Semana passada");
        assert_eq!(label(d(2026, 11, 7)), "No início deste mês");
        assert_eq!(label(d(2026, 10, 31)), "Mês passado");
        assert_eq!(label(d(2026, 2, 1)), "No início deste ano");
        assert_eq!(label(d(2025, 12, 31)), "Há muito tempo");
    }

    #[test]
    fn january_last_month_is_previous_december() {
        assert_eq!(date_group(d(2026, 12, 2), d(2027, 1, 20)).label, "Mês passado");
    }

    #[test]
    fn groups_order_folders_first_and_sizes_ascending() {
        let today = d(2026, 11, 19);
        let g = |e: Entry, by| group_of(&e, by, today);
        assert!(g(Entry::synthetic("a", true, 0), GroupBy::Kind) < g(Entry::synthetic("b.pdf", false, 1), GroupBy::Kind));
        assert_eq!(g(Entry::synthetic("b.pdf", false, 1), GroupBy::Kind).label, "Arquivo PDF");
        assert!(g(Entry::synthetic("x", false, 10), GroupBy::Size) < g(Entry::synthetic("y", false, 5 << 20), GroupBy::Size));
        assert_eq!(g(Entry::synthetic("z", false, 0), GroupBy::Size).label, "Vazio (0 KB)");
        assert_eq!(g(Entry::synthetic("m", false, 0), GroupBy::Modified).label, "Data desconhecida");
    }
}
