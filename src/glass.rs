//! Vidro nativo atrás da janela transparente:
//! - Windows 11: Mica; Windows 10: Acrylic. A swapchain DX12 vem de DirectComposition (patch em
//!   `patches/iced_wgpu`), a única forma de o conteúdo ter alfa por cima do material do sistema.
//! - macOS: NSVisualEffectView (vibrancy).
//! - Linux: transparência com desfoque do compositor (KDE desfoca; nos outros a área de trabalho aparece
//!   suave atrás da aurora).
//!
//! `ARBOR_NO_GLASS=1` desliga em qualquer sistema e a janela volta a ser opaca.

/// Se o vidro nativo deve ser pedido. Decide `transparent`/`blur` ao criar a janela.
pub fn wanted() -> bool {
    std::env::var_os("ARBOR_NO_GLASS").is_none()
}

/// Antes de criar a janela: no Windows, pede ao wgpu DX12 com swapchain de DirectComposition (com alfa).
/// Quem já definiu as variáveis do wgpu continua mandando.
pub fn prepare() {
    #[cfg(target_os = "windows")]
    if wanted() {
        for (key, value) in [("WGPU_BACKEND", "dx12"), ("WGPU_DX12_PRESENTATION_SYSTEM", "Visual")] {
            if std::env::var_os(key).is_none() {
                std::env::set_var(key, value);
            }
        }
    }
}

/// Aplica o material nativo depois que a janela existe. Devolve se o vidro ficou ativo.
pub fn apply(window: &dyn iced::window::Window) -> bool {
    if !wanted() {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        window_vibrancy::apply_mica(window, Some(true)).is_ok() || window_vibrancy::apply_acrylic(window, Some((14, 18, 36, 110))).is_ok()
    }
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        apply_vibrancy(window, NSVisualEffectMaterial::UnderWindowBackground, Some(NSVisualEffectState::Active), None).is_ok()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = window;
        true
    }
}
