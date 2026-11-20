//! Painel de detalhes à direita (Alt+Shift+P): preview, nome e propriedades do item sob o cursor.

use super::{drive_of, entry_chip, entry_kind};
use crate::app::{App, Message};
use crate::media::Preview;
use crate::theme;
use arbor_core::{human_count, human_size};
use iced::widget::text::Wrapping;
use iced::widget::{button, center, column, container, image, row, scrollable, text, Column};
use iced::{Alignment, ContentFit, Element, Fill, Theme};

pub fn panel(app: &App) -> Element<'_, Message> {
    let tab = app.tab();
    let mut col = Column::new().spacing(14);
    let selected = tab.selected.len();
    match (tab.current(), &app.details) {
        _ if selected > 1 => {
            let bytes: u64 = tab.entries.iter().filter(|e| tab.selected.contains(&e.path)).map(|e| e.size).sum();
            col = col
                .push(text(format!("{} itens selecionados", human_count(selected))).font(theme::DISPLAY).size(15))
                .push(text(format!("{} no total (arquivos)", human_size(bytes))).size(12.5).style(theme::ink2));
        }
        (Some(e), Some(d)) if d.path == e.path => {
            let hero: Element<_> = match &d.preview {
                Some(Preview::Image { handle, .. }) => {
                    image(handle.clone()).content_fit(ContentFit::Contain).width(Fill).height(180).border_radius(10).into()
                }
                Some(Preview::Text { content, .. }) => {
                    let head: String = content.lines().take(14).collect::<Vec<_>>().join("\n");
                    container(text(head).font(theme::MONO).size(10.5).wrapping(Wrapping::None))
                        .padding(10)
                        .width(Fill)
                        .height(180)
                        .clip(true)
                        .style(|t: &Theme| iced::widget::container::Style {
                            background: Some(theme::alpha(theme::tokens(t).void, 0.45).into()),
                            border: iced::border::rounded(10),
                            ..Default::default()
                        })
                        .into()
                }
                _ => {
                    center(entry_chip(entry_kind(e, drive_of(app, tab, e)), 84.0, e.hidden, tab.repos.contains(&e.path))).height(150).into()
                }
            };
            col = col.push(hero).push(text(e.name.as_str()).font(theme::DISPLAY).size(15));
            if let Some(Preview::Folder { dirs, files, bytes, more }) = &d.preview {
                let plus = if *more { "+" } else { "" };
                col = col.push(
                    text(format!(
                        "{} · {} · {}{plus}",
                        super::props_view::count(*dirs, plus, "pasta", "pastas"),
                        super::props_view::count(*files, plus, "arquivo", "arquivos"),
                        human_size(*bytes)
                    ))
                    .size(12)
                    .style(theme::ink2),
                );
            }
            let mut table = Column::new().spacing(10);
            if let Some(c) = tab.git.as_ref().and_then(|g| g.children.get(&e.path)) {
                table = table.push(column![text("Git").size(11).style(theme::ink3), text(c.label()).size(12.5)].spacing(2));
            }
            for (k, v) in &d.props {
                table = table.push(column![text(k.as_str()).size(11).style(theme::ink3), text(v.as_str()).size(12.5)].spacing(2));
            }
            if let Some(Preview::Image { info, .. } | Preview::Other { info }) = &d.preview {
                for (k, v) in info.iter().filter(|(k, _)| !d.props.iter().any(|(p, _)| p == k)) {
                    table = table.push(column![text(k.as_str()).size(11).style(theme::ink3), text(v.as_str()).size(12.5)].spacing(2));
                }
            }
            if !e.is_dir {
                table = table.push(match &d.hash {
                    Some(h) => column![
                        text("SHA-256").size(11).style(theme::ink3),
                        text(h.as_str()).font(theme::MONO).size(11).wrapping(Wrapping::Glyph)
                    ]
                    .spacing(2)
                    .into(),
                    None => Element::from(
                        button(text("Calcular SHA-256").size(12))
                            .on_press(Message::HashFile(d.path.clone()))
                            .padding([6, 10])
                            .style(theme::ghost),
                    ),
                });
            }
            col = col.push(table);
        }
        (Some(_), _) => col = col.push(center(text("Carregando…").style(theme::ink3)).height(120)),
        (None, _) => {
            col = col.push(text(format!("{} ({} itens)", tab.title(), human_count(tab.view.len()))).font(theme::DISPLAY).size(15)).push(
                row![text("Selecione um item para ver o preview e as propriedades.").size(12.5).style(theme::ink2)]
                    .align_y(Alignment::Center),
            );
        }
    }
    let body = scrollable(col.padding(16)).height(Fill).style(theme::scrollbar);
    container(body).width(app.details_w).height(Fill).style(|t| theme::glass(t, false)).into()
}
