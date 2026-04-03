use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pour::app::App;
use pour::config::Config;
use pour::data::history::History;
use pour::transport::Transport;
use pour::transport::fs::FsWriter;
use pour::tui::dashboard::{DashboardAction, MoveDirection, handle_key};

const DASH_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/test.md"
display_name = "Coffee"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"

[modules.me]
mode = "append"
path = "Journal/test.md"
append_under_header = "## Log"

[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "Entry"
"####;

const EMPTY_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules]
"####;

fn make_app() -> App {
    let config = Config::from_toml(DASH_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/test-dash-history.json")),
    )
}

fn make_empty_app() -> App {
    let config = Config::from_toml(EMPTY_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/test-dash-history.json")),
    )
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

// ── Basic actions ──

#[test]
fn q_quits() {
    let mut app = make_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Char('q'))),
        DashboardAction::Quit
    );
}

#[test]
fn enter_selects_module() {
    let mut app = make_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Enter)),
        DashboardAction::SelectModule
    );
}

#[test]
fn enter_on_empty_modules_returns_none() {
    let mut app = make_empty_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Enter)),
        DashboardAction::None
    );
}

#[test]
fn e_opens_configure() {
    let mut app = make_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Char('e'))),
        DashboardAction::ConfigureModule
    );
}

#[test]
fn e_on_empty_modules_returns_none() {
    let mut app = make_empty_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Char('e'))),
        DashboardAction::None
    );
}

#[test]
fn v_opens_vault_settings() {
    let mut app = make_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Char('v'))),
        DashboardAction::ConfigureVault
    );
}

#[test]
fn n_opens_new_module() {
    let mut app = make_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Char('n'))),
        DashboardAction::NewModule
    );
}

#[test]
fn r_refreshes_transport() {
    let mut app = make_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Char('r'))),
        DashboardAction::RefreshTransport
    );
}

#[test]
fn unrecognized_key_returns_none() {
    let mut app = make_app();
    assert_eq!(
        handle_key(&mut app, key(KeyCode::Char('z'))),
        DashboardAction::None
    );
}

// ── Navigation ──

#[test]
fn down_advances_selection() {
    let mut app = make_app();
    assert_eq!(app.selected_module, 0);
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.selected_module, 1);
}

#[test]
fn up_wraps_to_last() {
    let mut app = make_app();
    assert_eq!(app.selected_module, 0);
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.selected_module, app.module_keys.len() - 1);
}

#[test]
fn down_wraps_to_first() {
    let mut app = make_app();
    app.selected_module = app.module_keys.len() - 1;
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.selected_module, 0);
}

// ── Ctrl+Arrow reordering ──

#[test]
fn ctrl_up_reorders_up() {
    let mut app = make_app();
    app.selected_module = 1;
    assert_eq!(
        handle_key(&mut app, ctrl_key(KeyCode::Up)),
        DashboardAction::ReorderModule(MoveDirection::Up)
    );
}

#[test]
fn ctrl_down_reorders_down() {
    let mut app = make_app();
    app.selected_module = 0;
    assert_eq!(
        handle_key(&mut app, ctrl_key(KeyCode::Down)),
        DashboardAction::ReorderModule(MoveDirection::Down)
    );
}

#[test]
fn ctrl_up_at_top_returns_none() {
    let mut app = make_app();
    app.selected_module = 0;
    assert_eq!(
        handle_key(&mut app, ctrl_key(KeyCode::Up)),
        DashboardAction::None
    );
}

#[test]
fn ctrl_down_at_bottom_returns_none() {
    let mut app = make_app();
    app.selected_module = app.module_keys.len() - 1;
    assert_eq!(
        handle_key(&mut app, ctrl_key(KeyCode::Down)),
        DashboardAction::None
    );
}

// ── Help overlay ──

#[test]
fn question_mark_opens_help() {
    let mut app = make_app();
    assert!(!app.help_open);
    let action = handle_key(&mut app, key(KeyCode::Char('?')));
    assert!(app.help_open);
    assert_eq!(action, DashboardAction::None);
}

#[test]
fn help_overlay_esc_closes() {
    let mut app = make_app();
    app.help_open = true;
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.help_open);
}

#[test]
fn help_overlay_question_mark_closes() {
    let mut app = make_app();
    app.help_open = true;
    handle_key(&mut app, key(KeyCode::Char('?')));
    assert!(!app.help_open);
}

#[test]
fn help_overlay_blocks_other_keys() {
    let mut app = make_app();
    app.help_open = true;
    // 'q' should NOT quit while help is open
    let action = handle_key(&mut app, key(KeyCode::Char('q')));
    assert_eq!(action, DashboardAction::None);
    assert!(app.help_open); // still open
}

// ── Startup warnings overlay ──

#[test]
fn startup_warnings_enter_dismisses() {
    let mut app = make_app();
    app.startup_warnings = vec!["module 'coffee': path not found".to_string()];
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.startup_warnings.is_empty());
}

#[test]
fn startup_warnings_block_other_keys() {
    let mut app = make_app();
    app.startup_warnings = vec!["warning".to_string()];
    let action = handle_key(&mut app, key(KeyCode::Char('q')));
    assert_eq!(action, DashboardAction::None);
    assert!(!app.startup_warnings.is_empty()); // not dismissed
}

#[test]
fn startup_warnings_e_opens_configure_for_warned_module() {
    let mut app = make_app();
    app.startup_warnings = vec!["module 'me': path does not exist".to_string()];
    let action = handle_key(&mut app, key(KeyCode::Char('e')));
    assert_eq!(action, DashboardAction::ConfigureModule);
    assert!(app.startup_warnings.is_empty());
    // Should have selected the 'me' module
    let me_idx = app.module_keys.iter().position(|k| k == "me").unwrap();
    assert_eq!(app.selected_module, me_idx);
}
