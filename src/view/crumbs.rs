//! Caminho longo na barra de endereço: como no Explorer, os degraus do começo somem atrás de um "«"
//! (que lista as pastas escondidas) em vez de o caminho ser cortado sem aviso.

/// Largura estimada de um degrau: texto de 12,5 px mais a folga do botão e o "›".
pub fn crumb_width(label: &str) -> f32 {
    label.chars().count() as f32 * 7.2 + 30.0
}

/// Índice do primeiro degrau que cabe em `budget`, contando do fim; o último sempre aparece.
pub fn first_visible(widths: &[f32], budget: f32) -> usize {
    let mut used = 0.0;
    for (i, w) in widths.iter().enumerate().rev() {
        used += w;
        if used > budget && i + 1 < widths.len() {
            return i + 1;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabe_tudo_mostra_tudo() {
        assert_eq!(first_visible(&[50.0, 50.0, 50.0], 500.0), 0);
        assert_eq!(first_visible(&[], 10.0), 0);
    }

    #[test]
    fn caminho_longo_esconde_o_comeco_e_mantem_o_ultimo() {
        assert_eq!(first_visible(&[100.0, 100.0, 100.0, 100.0], 250.0), 2);
        assert_eq!(first_visible(&[100.0, 900.0], 50.0), 1, "o último aparece mesmo sem caber");
    }

    #[test]
    fn largura_cresce_com_o_nome_e_conta_unicode_por_letra() {
        assert!(crumb_width("Documentos") > crumb_width("Doc"));
        assert_eq!(crumb_width("ação"), crumb_width("acao"));
    }
}
