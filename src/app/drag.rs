//! Gestos com o mouse, como no Explorer: retângulo de seleção a partir da área vazia e arrastar itens
//! para uma pasta (linha, barra lateral ou outro painel). Enquanto um gesto está ativo, `pointer::arm`
//! faz a assinatura de eventos mandar movimentos e o soltar do botão; fora disso, nenhuma mensagem.

use super::{App, Message, ROW_H};
use crate::columns::{Col, Columns};
use crate::{computer, pointer};
use arbor_core::ops::Op;
use iced::{Point, Rectangle, Task};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Distância (px) que separa um clique de um arraste.
const DRAG_START: f32 = 6.0;

#[derive(Debug, Default)]
pub enum Gesture {
    #[default]
    None,
    /// Botão apertado num item; vira `Drag` se o mouse andar. `collapse`: clique simples num item já
    /// selecionado só desfaz a seleção múltipla ao soltar (senão não daria para arrastar vários).
    Press {
        tab: u64,
        index: usize,
        at: Point,
        collapse: bool,
    },
    Drag {
        paths: Vec<PathBuf>,
        at: Point,
        over: Option<PathBuf>,
    },
    /// Arrastando a borda de uma coluna do cabeçalho: a largura acompanha o mouse.
    Resize {
        col: Col,
        from_x: f32,
        start_w: f32,
    },
    /// Coordenadas do conteúdo da lista/grade; `start` chega no primeiro movimento.
    Band {
        tab: u64,
        start: Option<Point>,
        end: Point,
        base: HashSet<PathBuf>,
    },
}

/// Layout da área rolável, para achar os itens dentro do retângulo.
pub struct Layout {
    pub grid: bool,
    pub cols: usize,
    pub cell_w: f32,
    pub cell_h: f32,
}

/// Índices (na ordem de exibição) que o retângulo `r` toca, em coordenadas do conteúdo.
pub fn band_hits(r: Rectangle, l: &Layout, n: usize) -> Vec<usize> {
    if n == 0 || r.height <= 0.0 && r.width <= 0.0 {
        return vec![];
    }
    if !l.grid {
        let first = (r.y / ROW_H).floor().max(0.0) as usize;
        let last = ((r.y + r.height) / ROW_H).floor().max(0.0) as usize;
        return (first..=last.min(n - 1)).filter(|&i| i < n).collect();
    }
    // A grade começa em (10, 4): o recuo da `scrollable` em `view::grid`.
    let (x0, y0) = (r.x - 10.0, r.y - 4.0);
    let c1 = (x0 / l.cell_w).floor().max(0.0) as usize;
    let c2 = ((x0 + r.width) / l.cell_w).floor().max(0.0) as usize;
    let r1 = (y0 / l.cell_h).floor().max(0.0) as usize;
    let r2 = ((y0 + r.height) / l.cell_h).floor().max(0.0) as usize;
    let mut out = vec![];
    for row in r1..=r2 {
        for col in c1..=c2.min(l.cols - 1) {
            let i = row * l.cols + col;
            if i < n {
                out.push(i);
            }
        }
    }
    out
}

fn rect(a: Point, b: Point) -> Rectangle {
    Rectangle { x: a.x.min(b.x), y: a.y.min(b.y), width: (a.x - b.x).abs(), height: (a.y - b.y).abs() }
}

/// Mesmo disco: soltar move; outro disco: copia. Ctrl força copiar, Shift força mover (como no Explorer).
pub fn same_volume(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        matches!((std::fs::metadata(a), std::fs::metadata(b)), (Ok(x), Ok(y)) if x.dev() == y.dev())
    }
    #[cfg(windows)]
    {
        let root = |p: &Path| p.components().next().map(|c| c.as_os_str().to_ascii_lowercase());
        root(a) == root(b)
    }
}

/// Não solta numa pasta que é um dos itens, que está dentro deles ou onde eles já estão.
pub fn valid_target(paths: &[PathBuf], dest: &Path) -> bool {
    !computer::is_virtual(dest) && paths.iter().all(|p| !dest.starts_with(p) && p.parent() != Some(dest))
}

impl App {
    pub fn band_rect(&self, tab: u64) -> Option<Rectangle> {
        match &self.gesture {
            Gesture::Band { tab: t, start: Some(s), end, .. } if *t == tab => Some(rect(*s, *end)),
            _ => None,
        }
    }

    pub fn dragging(&self) -> Option<(&[PathBuf], Point, Option<&Path>)> {
        match &self.gesture {
            Gesture::Drag { paths, at, over, .. } => Some((paths, *at, over.as_deref())),
            _ => None,
        }
    }

    /// Ctrl copia, Shift move; sem tecla, move no mesmo disco e copia para outro.
    pub fn drag_copies(&self, paths: &[PathBuf], dest: &Path) -> bool {
        let mods = self.modifiers;
        if mods.command() {
            true
        } else if mods.shift() {
            false
        } else {
            !paths.first().is_some_and(|p| same_volume(p, dest))
        }
    }

    fn layout(&self, grid: bool) -> Layout {
        Layout { grid, cols: self.grid_cols(), cell_w: self.cell_w(), cell_h: self.cell_h() }
    }

    pub(super) fn press_item(&mut self, id: u64, index: usize) -> Task<Message> {
        let mods = self.modifiers;
        let already = self.find_tab(id).and_then(|t| t.entry_at(index).map(|e| t.selected.contains(&e.path)));
        let collapse = !mods.command() && !mods.shift() && already == Some(true);
        let task = if collapse {
            let Some(tab) = self.find_tab(id) else { return Task::none() };
            tab.cursor = index;
            Task::none()
        } else {
            self.click(id, index)
        };
        self.gesture = Gesture::Press { tab: id, index, at: pointer::get(), collapse };
        pointer::arm(true);
        task
    }

    pub(super) fn press_empty(&mut self, id: u64) -> Task<Message> {
        if let Some(pane) = self.pane_of(id) {
            self.active = pane;
        }
        let additive = self.modifiers.command();
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        if !additive {
            tab.selected.clear();
        }
        let base = tab.selected.clone();
        self.gesture = Gesture::Band { tab: id, start: None, end: Point::ORIGIN, base };
        pointer::arm(true);
        Task::none()
    }

    pub(super) fn band_move(&mut self, id: u64, p: Point) -> Task<Message> {
        let Gesture::Band { tab, start, end, base } = &mut self.gesture else { return Task::none() };
        if *tab != id {
            return Task::none();
        }
        let s = *start.get_or_insert(p);
        *end = p;
        let (r, base) = (rect(s, p), base.clone());
        let grid = self.find_tab(id).is_some_and(|t| t.grid);
        let layout = self.layout(grid);
        let Some(tab) = self.find_tab(id) else { return Task::none() };
        let hits = band_hits(r, &layout, tab.view.len());
        let mut sel = base;
        sel.extend(hits.iter().filter_map(|&i| tab.entry_at(i).map(|e| e.path.clone())));
        tab.selected = sel;
        if let Some(&last) = hits.last() {
            tab.cursor = last;
        }
        Task::none()
    }

    pub(super) fn pointer_moved(&mut self, at: Point) -> Task<Message> {
        match &mut self.gesture {
            Gesture::Press { tab, at: from, .. } if from.distance(at) > DRAG_START => {
                let id = *tab;
                let Some(t) = self.find_tab(id) else { return Task::none() };
                if computer::is_virtual(&t.path) {
                    return Task::none();
                }
                let paths = t.targets();
                self.gesture = Gesture::Drag { paths, at, over: None };
            }
            Gesture::Drag { at: pos, .. } => *pos = at,
            // A alça fica à esquerda da coluna (alinhadas à direita): puxar para a esquerda alarga.
            Gesture::Resize { col, from_x, start_w } => {
                let (col, w) = (*col, *start_w + *from_x - at.x);
                self.columns.set_width(col, w);
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn drag_over(&mut self, target: Option<PathBuf>) -> Task<Message> {
        if let Gesture::Drag { over, .. } = &mut self.gesture {
            *over = target;
        }
        Task::none()
    }

    pub(super) fn drag_leave(&mut self, from: PathBuf, back: Option<PathBuf>) -> Task<Message> {
        if let Gesture::Drag { over, .. } = &mut self.gesture {
            if over.as_ref() == Some(&from) {
                *over = back;
            }
        }
        Task::none()
    }

    pub(super) fn pointer_up(&mut self) -> Task<Message> {
        pointer::arm(false);
        match std::mem::take(&mut self.gesture) {
            Gesture::Press { tab, index, collapse: true, .. } => self.click(tab, index),
            Gesture::Drag { paths, over: Some(dest), .. } if valid_target(&paths, &dest) => {
                let copy = self.drag_copies(&paths, &dest);
                self.start(if copy { Op::Copy { sources: paths, dest } } else { Op::Move { sources: paths, dest } })
            }
            Gesture::Resize { .. } => self.save_columns(),
            _ => Task::none(),
        }
    }

    pub(super) fn resize_start(&mut self, col: Col) -> Task<Message> {
        self.gesture = Gesture::Resize { col, from_x: pointer::get().x, start_w: self.columns.width(col) };
        pointer::arm(true);
        Task::none()
    }

    pub(super) fn save_columns(&mut self) -> Task<Message> {
        match self.columns.save(Columns::file().as_deref()) {
            Ok(()) => Task::none(),
            Err(e) => self.notify(format!("Não salvou as colunas: {e}"), true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retangulo_na_lista_pega_as_linhas_tocadas() {
        let l = Layout { grid: false, cols: 1, cell_w: 0.0, cell_h: 0.0 };
        let r = Rectangle { x: 5.0, y: ROW_H * 1.5, width: 40.0, height: ROW_H * 2.0 };
        assert_eq!(band_hits(r, &l, 10), [1, 2, 3]);
        assert_eq!(band_hits(r, &l, 3), [1, 2], "não passa do fim da lista");
        assert!(band_hits(r, &l, 0).is_empty());
    }

    #[test]
    fn retangulo_na_grade_pega_colunas_e_linhas() {
        let l = Layout { grid: true, cols: 4, cell_w: 100.0, cell_h: 100.0 };
        let r = Rectangle { x: 150.0, y: 50.0, width: 100.0, height: 100.0 };
        assert_eq!(band_hits(r, &l, 20), [1, 2, 5, 6]);
        let wide = Rectangle { x: 10.0, y: 10.0, width: 5000.0, height: 1.0 };
        assert_eq!(band_hits(wide, &l, 20), [0, 1, 2, 3], "colunas além da última são ignoradas");
    }

    #[test]
    fn nao_solta_em_si_mesmo_nem_onde_ja_esta() {
        let items = vec![PathBuf::from("/a/b")];
        assert!(!valid_target(&items, Path::new("/a/b")));
        assert!(!valid_target(&items, Path::new("/a/b/c")));
        assert!(!valid_target(&items, Path::new("/a")));
        assert!(!valid_target(&items, &computer::path()));
        assert!(valid_target(&items, Path::new("/x")));
    }

    #[test]
    fn mesmo_volume_para_pasta_temporaria() {
        let t = std::env::temp_dir();
        assert!(same_volume(&t, &t));
    }
}
