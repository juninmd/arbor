//! Listas de salto do Explorer: o "›" da barra de endereço mostra as subpastas e o botão direito em
//! Voltar/Avançar mostra o histórico da aba.

use super::{tree, App, Message, Overlay};
use crate::computer;
use crate::pointer;
use iced::keyboard::key::Named;
use iced::{Point, Task};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpKind {
    Children,
    Back,
    Forward,
    /// Pastas escondidas atrás do "«" de um caminho longo (da mais funda para a raiz).
    Ancestors,
}

pub struct Jump {
    pub at: Point,
    pub kind: JumpKind,
    /// `None` enquanto as subpastas são lidas.
    pub items: Option<Vec<(String, PathBuf)>>,
    pub selected: Option<usize>,
    /// Descarta listagens atrasadas de um "›" anterior.
    pub generation: u64,
}

fn label(p: &std::path::Path) -> String {
    computer::virtual_title(p)
        .map(str::to_string)
        .or_else(|| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| p.display().to_string())
}

/// Do mais recente ao mais antigo, como no menu do Explorer.
pub fn history_items(stack: &[PathBuf]) -> Vec<(String, PathBuf)> {
    stack.iter().rev().map(|p| (label(p), p.clone())).collect()
}

impl App {
    pub(super) fn open_jump(&mut self, kind: JumpKind, dir: Option<PathBuf>) -> Task<Message> {
        let at = pointer::get();
        let generation = match &self.overlay {
            Overlay::Jump(j) => j.generation + 1,
            _ => 1,
        };
        let items = match kind {
            JumpKind::Back => Some(history_items(&self.tab().back)),
            JumpKind::Ancestors => dir.as_deref().map(|d| d.ancestors().map(|p| (label(p), p.to_path_buf())).collect()),
            JumpKind::Forward => Some(history_items(&self.tab().forward)),
            JumpKind::Children if dir.as_deref().is_some_and(computer::is) => {
                let places = self.places.iter().filter(|(_, p)| p.is_dir()).cloned();
                Some(places.chain(self.drives.iter().map(|d| (d.label.clone(), d.path.clone()))).collect())
            }
            JumpKind::Children => None,
        };
        if items.as_ref().is_some_and(Vec::is_empty) {
            return Task::none();
        }
        let pending = items.is_none();
        self.overlay = Overlay::Jump(Jump { at, kind, items, selected: None, generation });
        match dir.filter(|_| pending) {
            Some(dir) => {
                let hidden = self.show_hidden;
                Task::perform(
                    async move { tokio::task::spawn_blocking(move || tree::subdirs(&dir, hidden)).await.unwrap_or_default() },
                    move |dirs| Message::JumpListed { generation, dirs },
                )
            }
            None => Task::none(),
        }
    }

    pub(super) fn jump_listed(&mut self, generation: u64, dirs: Vec<PathBuf>) -> Task<Message> {
        if let Overlay::Jump(j) = &mut self.overlay {
            if j.generation == generation && j.items.is_none() {
                j.items = Some(dirs.iter().map(|p| (label(p), p.clone())).collect());
            }
        }
        Task::none()
    }

    pub(super) fn jump_pick(&mut self, index: usize) -> Task<Message> {
        let Overlay::Jump(j) = std::mem::replace(&mut self.overlay, Overlay::None) else { return Task::none() };
        let Some(target) = j.items.and_then(|items| items.into_iter().nth(index)).map(|(_, p)| p) else { return Task::none() };
        let pane = self.active;
        match j.kind {
            JumpKind::Children | JumpKind::Ancestors => self.navigate(pane, target),
            back_or_forward => {
                // Pula `index + 1` passos de uma vez, mantendo o resto do histórico como no Explorer.
                let back = back_or_forward == JumpKind::Back;
                let tab = self.tab_mut();
                let mut current = tab.path.clone();
                for _ in 0..=index {
                    let (from, to) = if back { (&mut tab.back, &mut tab.forward) } else { (&mut tab.forward, &mut tab.back) };
                    let Some(next) = from.pop() else { break };
                    to.push(std::mem::replace(&mut current, next));
                }
                tab.focus_on_load = vec![tab.path.clone()];
                self.load(pane, current, false)
            }
        }
    }

    pub(super) fn jump_key(&mut self, named: Option<Named>) -> Task<Message> {
        let Overlay::Jump(j) = &mut self.overlay else { return Task::none() };
        let n = j.items.as_ref().map_or(0, Vec::len);
        match named {
            Some(Named::Escape) => self.overlay = Overlay::None,
            Some(Named::ArrowDown) if n > 0 => j.selected = Some(j.selected.map_or(0, |i| (i + 1) % n)),
            Some(Named::ArrowUp) if n > 0 => j.selected = Some(j.selected.map_or(n - 1, |i| (i + n - 1) % n)),
            Some(Named::Enter) => {
                if let Some(i) = j.selected {
                    return self.jump_pick(i);
                }
            }
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historico_mostra_o_mais_recente_primeiro() {
        let stack = [PathBuf::from("/a"), PathBuf::from("/a/b"), computer::path()];
        let names: Vec<_> = history_items(&stack).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, [computer::TITLE, "b", "a"]);
    }
}
