use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pour::tui::summary::{SummaryAction, handle_key};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn enter_returns_dashboard() {
    assert_eq!(handle_key(key(KeyCode::Enter)), SummaryAction::Dashboard);
}

#[test]
fn a_returns_another_entry() {
    assert_eq!(
        handle_key(key(KeyCode::Char('a'))),
        SummaryAction::AnotherEntry
    );
}

#[test]
fn q_returns_quit() {
    assert_eq!(handle_key(key(KeyCode::Char('q'))), SummaryAction::Quit);
}

#[test]
fn unrecognized_char_returns_none() {
    assert_eq!(handle_key(key(KeyCode::Char('x'))), SummaryAction::None);
}

#[test]
fn arrow_keys_return_none() {
    assert_eq!(handle_key(key(KeyCode::Up)), SummaryAction::None);
    assert_eq!(handle_key(key(KeyCode::Down)), SummaryAction::None);
}

#[test]
fn esc_returns_none() {
    assert_eq!(handle_key(key(KeyCode::Esc)), SummaryAction::None);
}
