//! arbor: explorer de arquivos rápido e multiplataforma.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod commands;
mod fuzzy;
mod launch;
mod media;
mod model;
mod ops;
mod view;
mod watch;

use app::App;

fn main() -> iced::Result {
    iced::application(App::boot, App::update, view::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(view::theme)
        .window_size((1280.0, 800.0))
        .antialiasing(true)
        .run()
}
