//! arbor: explorer de arquivos rápido e multiplataforma.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod aurora;
mod checksums;
mod clash;
mod colors;
mod columns;
mod commands;
mod complete;
mod computer;
mod diff;
mod discovery;
mod folder_views;
mod git_branch;
mod glass;
mod global_search;
mod home;
mod instance;
mod keymap;
mod launch;
mod layouts;
mod listing;
mod media;
mod network;
mod palettes;
mod pins;
mod pointer;
mod props;
mod session;
mod shelf;
mod shortcut;
mod sysclip;
mod theme;
mod thumbs;
mod trashbin;
mod update;
mod view;
mod view_modes;
mod watch;
mod window_chrome;

/// mimalloc: a listagem cria centenas de milhares de alocações pequenas em várias threads; com o
/// alocador do sistema elas disputam o mesmo lock e a pasta de 100 mil itens leva 4× mais.
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

use app::App;

fn main() -> iced::Result {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("arbor {}", update::CURRENT);
        return Ok(());
    }
    // Já tem uma janela aberta: manda a pasta para ela (vira uma aba nova) e sai.
    let target = instance::start_arg().and_then(|p| std::fs::canonicalize(p).ok());
    if instance::forward(target.as_deref()) {
        return Ok(());
    }
    instance::listen();
    thumbs::prune_cache();
    glass::log_to_stderr();
    glass::prepare();
    keymap::reload();
    colors::reload();
    let mut app = iced::application(App::boot, App::update, view::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(view::theme)
        .style(view::style)
        .default_font(theme::UI)
        .window(iced::window::Settings {
            size: iced::Size::new(1320.0, 840.0),
            min_size: Some(iced::Size::new(720.0, 460.0)),
            icon: window_icon(),
            // vidro nativo onde o sistema dá (src/glass.rs); no resto a janela fica opaca
            transparent: glass::wanted(),
            blur: glass::wanted(),
            // barra de título própria, com as abas (src/window_chrome.rs)
            decorations: !window_chrome::custom(),
            ..Default::default()
        })
        .antialiasing(true);
    for font in theme::FONTS {
        app = app.font(font);
    }
    app.run()
}

/// Ícone da janela gerado do mesmo SVG do logo (Windows e Linux; o macOS usa o do .app).
fn window_icon() -> Option<iced::window::Icon> {
    iced::window::icon::from_file_data(include_bytes!("../assets/icon-256.png"), None).ok()
}
