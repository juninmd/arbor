//! Busca fuzzy (a engine do Helix, `nucleo`), usada no filtro da pasta e na paleta de comandos.
//!
//! Em listas grandes os candidatos são divididos entre os núcleos, cada um com seu `Matcher`: filtrar 100 mil
//! nomes cabe num quadro de 120 Hz.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rayon::prelude::*;

/// Abaixo disso uma thread só é mais rápida que dividir o trabalho.
const PARALLEL_FROM: usize = 4_096;

pub struct Fuzzy {
    matcher: Matcher,
    pattern: Pattern,
    buf: Vec<char>,
}

impl Fuzzy {
    pub fn new(query: &str) -> Self {
        Fuzzy::with_pattern(Pattern::parse(query, CaseMatching::Smart, Normalization::Smart))
    }

    fn with_pattern(pattern: Pattern) -> Self {
        Fuzzy { matcher: Matcher::new(Config::DEFAULT.match_paths()), pattern, buf: Vec::new() }
    }

    pub fn score(&mut self, haystack: &str) -> Option<u32> {
        self.pattern.score(Utf32Str::new(haystack, &mut self.buf), &mut self.matcher)
    }

    /// Posições (em caracteres) que casaram, para destacar na lista.
    pub fn indices(&mut self, haystack: &str) -> Vec<u32> {
        let mut out = Vec::new();
        self.pattern.indices(Utf32Str::new(haystack, &mut self.buf), &mut self.matcher, &mut out);
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// Índices de `items` que casam com `query`, do melhor para o pior. Em empate vence o nome mais curto
/// (o mais parecido com o que foi digitado) e depois a ordem original.
pub fn rank<'a>(query: &str, items: impl Iterator<Item = &'a str>) -> Vec<usize> {
    let items: Vec<&str> = items.collect();
    rank_slice(query, &items, |s| s)
}

/// Como [`rank`], mas sobre uma fatia de qualquer tipo, sem copiar os nomes. `name` extrai o texto de cada item.
pub fn rank_slice<'s, T: Sync>(query: &str, items: &'s [T], name: impl Fn(&'s T) -> &'s str + Sync) -> Vec<usize> {
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let score_range = |offset: usize, chunk: &'s [T]| -> Vec<(u32, u32, u32)> {
        let mut f = Fuzzy::with_pattern(pattern.clone());
        chunk
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                let s = name(it);
                f.score(s).map(|sc| (sc, s.len() as u32, (offset + i) as u32))
            })
            .collect()
    };
    let mut scored: Vec<(u32, u32, u32)> = if items.len() >= PARALLEL_FROM {
        let per = items.len().div_ceil(rayon::current_num_threads() * 4).max(1_024);
        items.par_chunks(per).enumerate().flat_map_iter(|(k, chunk)| score_range(k * per, chunk)).collect()
    } else {
        score_range(0, items)
    };
    let order = |a: &(u32, u32, u32), b: &(u32, u32, u32)| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2));
    if scored.len() >= PARALLEL_FROM {
        scored.par_sort_unstable_by(order);
    } else {
        scored.sort_unstable_by(order);
    }
    scored.into_iter().map(|(.., i)| i as usize).collect()
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

    #[test]
    fn paralelo_igual_ao_sequencial() {
        let names: Vec<String> = (0..20_000).map(|i| format!("arquivo-{i:05}-{}.rs", i % 7)).collect();
        let par = rank_slice("arq3", &names, |s| s.as_str());
        let mut f = Fuzzy::new("arq3");
        let mut seq: Vec<(u32, u32, u32)> =
            names.iter().enumerate().filter_map(|(i, s)| f.score(s).map(|sc| (sc, s.len() as u32, i as u32))).collect();
        seq.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        assert_eq!(par, seq.into_iter().map(|t| t.2 as usize).collect::<Vec<_>>());
        assert!(!par.is_empty());
    }

    #[test]
    fn indices_para_destaque() {
        assert_eq!(Fuzzy::new("crg").indices("Cargo.toml"), vec![0, 2, 3]);
    }
}
