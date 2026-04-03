use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pour::app::{App, Screen, SummaryState};
use pour::config::Config;
use pour::data::history::History;
use pour::transport::fs::FsWriter;
use pour::transport::{Transport, TransportMode};
use pour::tui::{Action, handle_event};

const TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####;

fn make_app() -> App {
    let config = Config::from_toml(TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/test-dispatch-history.json")),
    )
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

// ── Dashboard dispatch ──

#[test]
fn dashboard_quit() {
    let mut app = make_app();
    assert_eq!(app.screen, Screen::Dashboard);
    assert_eq!(
        handle_event(&mut app, key(KeyCode::Char('q'))),
        Action::Quit
    );
}

#[test]
fn dashboard_enter_navigates_to_form() {
    let mut app = make_app();
    let action = handle_event(&mut app, key(KeyCode::Enter));
    assert_eq!(action, Action::Navigate(Screen::Form));
    assert_eq!(app.screen, Screen::Form);
    assert!(app.form_state.is_some());
}

// ── Form dispatch ──

#[test]
fn form_esc_on_empty_cancels_to_dashboard() {
    let mut app = make_app();
    // Set up form screen
    app.form_state = app.init_form("test");
    app.screen = Screen::Form;

    let action = handle_event(&mut app, key(KeyCode::Esc));
    assert_eq!(action, Action::Navigate(Screen::Dashboard));
    assert_eq!(app.screen, Screen::Dashboard);
    assert!(app.form_state.is_none());
}

#[test]
fn form_tab_returns_none() {
    let mut app = make_app();
    app.form_state = app.init_form("test");
    app.screen = Screen::Form;

    assert_eq!(handle_event(&mut app, key(KeyCode::Tab)), Action::None);
}

#[test]
fn form_submit_on_submit_button() {
    let mut app = make_app();
    app.form_state = app.init_form("test");
    app.screen = Screen::Form;

    // Navigate to submit button (1 field + submit = active_field=1)
    let field_count = app.config.modules["test"].fields.len();
    app.form_state.as_mut().unwrap().active_field = field_count;

    assert_eq!(handle_event(&mut app, key(KeyCode::Enter)), Action::Submit);
}

// ── Summary dispatch ──

#[test]
fn summary_enter_returns_to_dashboard() {
    let mut app = make_app();
    app.screen = Screen::Summary;
    app.summary_state = Some(SummaryState {
        message: "ok".to_string(),
        file_path: Some("test.md".to_string()),
        transport_mode: TransportMode::FileSystem,
        auto_created_notes: vec![],
    });

    let action = handle_event(&mut app, key(KeyCode::Enter));
    assert_eq!(action, Action::Navigate(Screen::Dashboard));
    assert_eq!(app.screen, Screen::Dashboard);
    assert!(app.summary_state.is_none());
}

#[test]
fn summary_a_navigates_to_form() {
    let mut app = make_app();
    app.screen = Screen::Summary;
    app.summary_state = Some(SummaryState {
        message: "ok".to_string(),
        file_path: Some("test.md".to_string()),
        transport_mode: TransportMode::FileSystem,
        auto_created_notes: vec![],
    });

    let action = handle_event(&mut app, key(KeyCode::Char('a')));
    assert_eq!(action, Action::Navigate(Screen::Form));
    assert_eq!(app.screen, Screen::Form);
    assert!(app.form_state.is_some());
    assert!(app.summary_state.is_none());
}

#[test]
fn summary_q_quits() {
    let mut app = make_app();
    app.screen = Screen::Summary;
    app.summary_state = Some(SummaryState {
        message: "ok".to_string(),
        file_path: None,
        transport_mode: TransportMode::FileSystem,
        auto_created_notes: vec![],
    });

    assert_eq!(
        handle_event(&mut app, key(KeyCode::Char('q'))),
        Action::Quit
    );
}
