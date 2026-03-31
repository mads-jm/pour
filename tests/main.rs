use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

#[test]
fn handles_press_events_only() {
    let press = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert!(pour::should_handle_key_event(press));

    let repeat = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Repeat,
        state: KeyEventState::empty(),
    };
    assert!(!pour::should_handle_key_event(repeat));

    let release = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Release,
        state: KeyEventState::empty(),
    };
    assert!(!pour::should_handle_key_event(release));
}
