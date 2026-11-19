//! Vidro nativo atrás da janela: vibrancy (NSVisualEffectView) no macOS e transparência com desfoque do
//! compositor no Linux (KDE desfoca; nos outros a área de trabalho aparece suave atrás da aurora).
//!
//! No Windows o Mica/Acrylic precisa de uma swapchain com alfa, que o renderizador do iced 0.14 não pede
//! (DXGI sem DirectComposition); lá a janela fica opaca e o vidro é o da própria aurora.
//! `ARBOR_NO_GLASS=1` desliga em qualquer sistema.

/// Se o sistema oferece vidro nativo para esta janela. Decide `transparent`/`blur` ao criar a janela.
pub fn wanted() -> bool {
    cfg!(any(target_os = "macos", target_os = "linux")) && std::env::var_os("ARBOR_NO_GLASS").is_none()
}

/// Aplica o material nativo depois que a janela existe. Devolve se o vidro ficou ativo.
pub fn apply(window: &dyn iced::window::Window) -> bool {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        apply_vibrancy(window, NSVisualEffectMaterial::UnderWindowBackground, Some(NSVisualEffectState::Active), None).is_ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        wanted()
    }
}
