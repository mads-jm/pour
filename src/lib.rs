pub mod app;
pub mod config;
pub mod data;
pub mod init;
pub mod output;
pub mod transport;
pub mod tui;

use crossterm::event::{KeyEvent, KeyEventKind};

pub fn should_handle_key_event(key_event: KeyEvent) -> bool {
    key_event.kind == KeyEventKind::Press
}
