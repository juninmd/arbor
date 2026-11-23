//! Pílulas da barra de comandos (estado git da pasta, itens ocultos) e a marca de alteração de cada item.

use crate::app::{Message, Tab};
use crate::commands::Cmd;
use crate::git::{Change, GitInfo};
use crate::theme::{self, icon, Icon};
use iced::widget::{button, column, container, row, text, tooltip, Row};
use iced::{Alignment, Color, Element, Theme};

/// Cor da marca: novo/adicionado verde, modificado âmbar, renomeado lilás, excluído coral, conflito rosa.
fn change_color(c: Change) -> Color {
    match c {
        Change::Untracked | Change::Added => theme::MINT,
        Change::Modified => theme::AMBER,
        Change::Renamed => theme::IRIS,
        Change::Deleted => theme::CORAL,
        Change::Conflict => theme::ROSE,
    }
}

/// Marca à direita do nome: letra no arquivo, ponto na pasta (a pior mudança lá dentro).
pub fn marker<'a>(tab: &Tab, path: &std::path::Path, is_dir: bool) -> Option<Element<'a, Message>> {
    let c = *tab.git.as_ref()?.children.get(path)?;
    let color = change_color(c);
    let mark = if is_dir { text("●").size(8) } else { text(c.letter()).font(theme::MONO).size(11) };
    Some(mark.style(move |_: &Theme| text::Style { color: Some(color) }).into())
}

fn tip<'a>(content: impl Into<Element<'a, Message>>, lines: Vec<String>) -> Element<'a, Message> {
    let body = column(lines.into_iter().map(|l| text(l).size(12).into())).spacing(3);
    tooltip(content, body, tooltip::Position::Bottom).padding(8).style(theme::tooltip).into()
}

fn summary(g: &GitInfo) -> Vec<String> {
    let mut lines = vec![format!("Branch {}", g.branch)];
    if let Some(w) = &g.worktree {
        lines.push(format!("Worktree vinculado \"{w}\""));
    }
    lines.push(if !g.upstream {
        "Sem branch remota".into()
    } else if g.ahead == 0 && g.behind == 0 {
        "Em dia com a remota".into()
    } else {
        format!("{} à frente, {} atrás da remota", g.ahead, g.behind)
    });
    if g.changed() == 0 {
        lines.push("Nada alterado".into());
    } else {
        lines.push(format!(
            "{} preparados · {} modificados · {} novos{}",
            g.staged,
            g.unstaged,
            g.untracked,
            if g.conflicts > 0 { format!(" · {} em conflito", g.conflicts) } else { String::new() }
        ));
        lines.push(format!("+{} −{} linhas em relação ao último commit", g.added_lines, g.removed_lines));
    }
    lines
}

fn colored<'a>(s: String, color: Color) -> Element<'a, Message> {
    text(s).font(theme::MONO).size(11).style(move |_: &Theme| text::Style { color: Some(color) }).into()
}

/// "⎇ main  ⧉ wt  ↑2 ↓1  +12 −3  6": cabe na barra; o detalhe fica na dica.
pub fn pill<'a>(tab: &Tab) -> Option<Element<'a, Message>> {
    let g = tab.git.as_ref()?;
    let ink2 = theme::now().ink2;
    let mut r = Row::new().spacing(7).align_y(Alignment::Center);
    r = r.push(icon(Icon::Branch, 13.0, theme::CORAL)).push(text(super::shorten(&g.branch, 28)).font(theme::UI_BOLD).size(11.5));
    if let Some(w) = &g.worktree {
        r = r.push(row![icon(Icon::Copy, 11.0, ink2), text(super::shorten(w, 18)).size(11)].spacing(3).align_y(Alignment::Center));
    }
    if g.ahead > 0 || g.behind > 0 {
        r = r.push(text(format!("↑{} ↓{}", g.ahead, g.behind)).font(theme::MONO).size(11).style(theme::ink2));
    }
    if g.added_lines > 0 || g.removed_lines > 0 {
        r = r.push(colored(format!("+{}", g.added_lines), theme::MINT)).push(colored(format!("−{}", g.removed_lines), theme::CORAL));
    }
    let n = g.changed();
    if n > 0 {
        let c = if g.conflicts > 0 { theme::ROSE } else { theme::AMBER };
        r = r.push(container(colored(n.to_string(), c)).padding([0, 5]).style(move |_: &Theme| container::Style {
            background: Some(theme::alpha(c, 0.14).into()),
            border: iced::border::rounded(5),
            ..Default::default()
        }));
    }
    let b = button(r).padding([3, 9]).style(theme::pill(theme::CORAL, false));
    Some(tip(b, summary(g)))
}

/// Link do remoto ("github.com/o/r"): um clique abre a página no navegador.
pub fn remote<'a>(tab: &Tab) -> Option<Element<'a, Message>> {
    let url = tab.git.as_ref()?.remote.as_deref()?;
    let label = url.split_once("://").map_or(url, |(_, rest)| rest);
    let content =
        row![icon(Icon::Link, 12.0, theme::now().ink2), text(super::shorten(label, 34)).size(11.5)].spacing(5).align_y(Alignment::Center);
    let b = button(content).padding([3, 9]).on_press(Message::Run(Cmd::OpenRemote)).style(theme::pill(theme::now().accent, false));
    Some(tip(b, vec![format!("Abrir no navegador: {url}")]))
}

/// "N ocultos": avisa que a pasta tem itens escondidos e liga/desliga com um clique (Ctrl+H).
pub fn hidden<'a>(tab: &Tab, showing: bool) -> Option<Element<'a, Message>> {
    if tab.hidden == 0 {
        return None;
    }
    let label = if tab.hidden == 1 { "1 oculto".to_string() } else { format!("{} ocultos", arbor_core::human_count(tab.hidden)) };
    let ink = if showing { theme::now().ink } else { theme::now().ink2 };
    let content = row![icon(Icon::Eye, 13.0, ink), text(label).size(11.5)].spacing(5).align_y(Alignment::Center);
    let b = button(content).padding([3, 9]).on_press(Message::Run(Cmd::ToggleHidden)).style(theme::pill(theme::now().accent, showing));
    let what = if showing { "Esconder os itens ocultos" } else { "Mostrar os itens ocultos" };
    Some(tip(b, vec![format!("{what}  {}", Cmd::ToggleHidden.shortcut())]))
}
