//! Janela "Propriedades": ícone e nome no topo, tabela de rótulo e valor, e Fechar.

use super::{entry_chip, kbd, kind_for, separator, sheet as card};
use crate::app::Message;
use crate::media::Preview;
use crate::theme;
use arbor_core::human_size;
use iced::widget::text::Wrapping;
use iced::widget::{button, center, column, image, row, scrollable, space, text, Column};
use iced::{Alignment, ContentFit, Element, Fill};
use std::path::Path;

const LABEL_W: f32 = 132.0;

fn line<'a>(k: String, v: String) -> Element<'a, Message> {
    row![text(k).size(12).style(theme::ink3).width(LABEL_W), text(v).size(12.5).width(Fill).wrapping(Wrapping::WordOrGlyph)]
        .spacing(12)
        .into()
}

fn count(n: usize, plus: &str, one: &str, many: &str) -> String {
    format!("{n}{plus} {}", if n == 1 && plus.is_empty() { one } else { many })
}

pub fn sheet<'a>(path: &'a Path, props: Option<&'a [(String, String)]>, preview: Option<&'a Preview>) -> Element<'a, Message> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
    let has = |key: &str| props.is_some_and(|p| p.iter().any(|(k, _)| k == key));
    let kind = props.and_then(|p| p.iter().find(|(k, _)| k == "Tipo")).map(|(_, v)| v.clone()).unwrap_or_default();
    let is_dir = matches!(preview, Some(Preview::Folder { .. })) || kind == "Pasta";
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let hero: Element<_> = match preview {
        Some(Preview::Image { handle, .. }) => {
            image(handle.clone()).width(56).height(56).content_fit(ContentFit::Cover).border_radius(12).into()
        }
        _ => entry_chip(kind_for(is_dir, &ext, path), 56.0, false, has("Repositório git")),
    };
    let head = row![
        hero,
        column![text(name).font(theme::DISPLAY).size(17).wrapping(Wrapping::WordOrGlyph), text(kind).size(12).style(theme::ink3)]
            .spacing(3)
            .width(Fill)
    ]
    .spacing(14)
    .align_y(Alignment::Center);

    let table: Element<_> = match props {
        None => center(text("Lendo…").size(12.5).style(theme::ink3)).height(120).into(),
        Some(list) => {
            let mut rows = Column::new().spacing(9);
            // Pasta: o conteúdo vem da contagem do preview (com teto; "+" quando passou dele).
            if let Some(Preview::Folder { dirs, files, bytes, more }) = preview {
                let plus = if *more { "+" } else { "" };
                rows = rows.push(line("Tamanho".into(), format!("{}{plus}", human_size(*bytes)))).push(line(
                    "Conteúdo".into(),
                    format!("{}, {}", count(*dirs, plus, "pasta", "pastas"), count(*files, plus, "arquivo", "arquivos")),
                ));
            }
            for (k, v) in list.iter().filter(|(k, _)| k != "Tipo") {
                rows = rows.push(line(k.clone(), v.clone()));
            }
            scrollable(rows).height(iced::Length::Shrink).style(theme::scrollbar).into()
        }
    };
    let footer = row![
        kbd("Esc".into()),
        text("fecha").size(11.5).style(theme::ink3),
        space().width(Fill),
        button(text("Fechar").size(13)).padding([7, 16]).on_press(Message::CloseOverlay).style(theme::primary),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    card(column![head, separator(), table, footer].spacing(16), 500.0)
}

#[cfg(test)]
mod tests {
    use super::count;

    #[test]
    fn folder_contents_use_singular_only_for_exactly_one() {
        assert_eq!(count(1, "", "pasta", "pastas"), "1 pasta");
        assert_eq!(count(0, "", "pasta", "pastas"), "0 pastas");
        assert_eq!(count(1, "+", "pasta", "pastas"), "1+ pastas", "contagem cortada é um mínimo");
    }
}
