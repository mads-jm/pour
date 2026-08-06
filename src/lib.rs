pub mod app;
pub mod autocreate;
pub mod config;
pub(crate) mod config_edit;
pub(crate) mod config_updates; // internal TUI configure helper — no external API
pub mod data;
pub mod hooks;
pub mod init;
pub mod output;
pub mod paths;
pub mod server;
pub mod transport;
pub mod tui;
pub mod util;
pub mod visibility;

use crossterm::event::{KeyEvent, KeyEventKind};

pub fn should_handle_key_event(key_event: KeyEvent) -> bool {
    key_event.kind == KeyEventKind::Press
}
