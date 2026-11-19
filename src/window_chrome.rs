//! Janela sem a moldura do sistema (`decorations: false`): a barra de título é do arbor, com as abas,
//! e daqui saem arrastar, redimensionar pelas bordas e minimizar/maximizar/fechar.

use crate::app::Message;
use iced::window::{self, Direction};
use iced::Task;

#[derive(Debug, Clone, Copy)]
pub enum WindowAction {
    Drag,
    Minimize,
    ToggleMaximize,
    Close,
    Resize(Direction),
}

/// Desligável com `ARBOR_NATIVE_FRAME=1` (gerenciador de janelas que não deixa arrastar a janela).
pub fn custom() -> bool {
    std::env::var_os("ARBOR_NATIVE_FRAME").is_none()
}

pub fn run(action: WindowAction) -> Task<Message> {
    window::latest().and_then(move |id| match action {
        WindowAction::Drag => window::drag(id),
        WindowAction::Minimize => window::minimize(id, true),
        WindowAction::ToggleMaximize => window::toggle_maximize(id),
        WindowAction::Close => window::close(id),
        WindowAction::Resize(dir) => window::drag_resize(id, dir),
    })
}
