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
    // Windows: a swapchain de DirectComposition ainda derruba o app em algumas máquinas
    // ("Invalid surface" no Surface::configure). Até validar, o Mica é opt-in (ARBOR_GLASS=1).
    if cfg!(target_os = "windows") {
        return std::env::var_os("ARBOR_GLASS").is_some();
    }
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

/// `ARBOR_LOG=1` manda os avisos do wgpu e do iced para o stderr; serve para diagnosticar o vidro
/// em máquinas onde a superfície não abre.
pub fn log_to_stderr() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.level() <= log::Level::Info
        }
        fn log(&self, r: &log::Record) {
            if self.enabled(r.metadata()) {
                eprintln!("[{} {}] {}", r.level(), r.target(), r.args());
            }
        }
        fn flush(&self) {}
    }
    if std::env::var_os("ARBOR_LOG").is_some() && log::set_logger(&Stderr).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}
