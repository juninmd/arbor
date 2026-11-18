//! Busca fuzzy (a mesma engine do Helix), usada no filtro da pasta e na paleta de comandos.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

pub struct Fuzzy {
    matcher: Matcher,
    pattern: Pattern,
    buf: Vec<char>,
}

impl Fuzzy {
    pub fn new(query: &str) -> Self {
        Fuzzy {
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            pattern: Pattern::parse(query, CaseMatching::Smart, Normalization::Smart),
            buf: Vec::new(),
        }
    }

    pub fn score(&mut self, haystack: &str) -> Option<u32> {
        self.pattern.score(Utf32Str::new(haystack, &mut self.buf), &mut self.matcher)
    }
}

/// Índices de `items` que casam com `query`, do melhor para o pior. Em empate vence o nome mais curto
/// (o mais parecido com o que foi digitado) e depois a ordem original.
pub fn rank<'a>(query: &str, items: impl Iterator<Item = &'a str>) -> Vec<usize> {
    let mut f = Fuzzy::new(query);
    let mut scored: Vec<(usize, u32, usize)> = items.enumerate().filter_map(|(i, s)| f.score(s).map(|sc| (i, sc, s.len()))).collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(i, ..)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordena_pelo_melhor() {
        let items = ["relatorio_final.pdf", "foto.jpg", "rel.txt", "Cargo.toml"];
        let r = rank("rel", items.iter().copied());
        assert_eq!(items[r[0]], "rel.txt");
        assert!(!r.contains(&1));
    }

    #[test]
    fn letras_salteadas() {
        let items = ["Cargo.toml", "cargo-lock"];
        assert_eq!(rank("ctml", items.iter().copied()), vec![0]);
    }
}
