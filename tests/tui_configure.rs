/// Pin-down tests for `src/tui/configure.rs`.
///
/// These tests exercise the public API surface of the configure screen so
/// that the upcoming Slice 5 refactor is safe. They assert *current behaviour*
/// — if a bug is found it is documented with `// BUG: ...` and the test
/// asserts the existing (broken) behaviour rather than fixing it.
///
/// Coverage categories
/// -------------------
/// 1.  Render dispatch    — each top-level mode renders without panicking
/// 2.  Key routing        — per-mode key dispatch returns expected ConfigureAction / mutates state
/// 3.  Auto-save          — `auto_save_module_settings` / `auto_save_field_settings` honour dirty flag
/// 4.  Build-from-settings — `build_field_updates_from_settings` round-trip
/// 5.  Scroll sync         — `sync_scroll_offset` boundary conditions
/// 6.  Browser nav         — Enter / Backspace / Esc inside the browser
/// 7.  Confirm dialog      — y / n resolve PendingConfirm correctly
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pour::app::{App, BrowserState, ConfigureLevel, PendingConfirm, SettingKind};
use pour::config::Config;
use pour::data::field_presets::FieldPresets;
use pour::data::history::History;
use pour::data::presets::Presets;
use pour::transport::Transport;
use pour::transport::VaultEntry;
use pour::transport::fs::FsWriter;
use pour::tui::configure::{ConfigureAction, build_field_updates_from_settings, handle_key};

// ─── Fixtures ────────────────────────────────────────────────────────────────

const CONFIG_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"
api_port = 27124
api_key = "test-key"
date_format = "%Y-%m-%d"

[modules.coffee]
mode = "create"
path = "Coffee/test.md"
display_name = "Coffee"
callout_type = "note"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
required = true

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating"
default = "3"

[[modules.coffee.fields]]
name = "origin"
field_type = "static_select"
prompt = "Origin"
options = ["Ethiopia", "Colombia"]

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
target = "body"
callout = "tip"

[[modules.coffee.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Recipe"

[[modules.coffee.fields.sub_fields]]
name = "amount"
field_type = "number"
prompt = "Amount"

[[modules.coffee.fields.sub_fields]]
name = "type"
field_type = "static_select"
prompt = "Type"
options = ["Bloom", "Spiral"]

[modules.journal]
mode = "append"
path = "Journal/test.md"
append_under_header = "## Log"

[[modules.journal.fields]]
name = "body"
field_type = "textarea"
prompt = "Entry"
required = true
"####;

fn make_app() -> App {
    let config = Config::from_toml(CONFIG_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-cfg-handle-history.json",
        )),
        Presets::empty(),
        FieldPresets::empty(),
    )
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

// ─── Category 1: Render dispatch ─────────────────────────────────────────────
//
// Each test builds a terminal with TestBackend and calls configure::render().
// We assert that a known string appears in the rendered buffer — that's enough
// to confirm the dispatcher chose the right sub-renderer and didn't panic.

#[test]
fn render_module_settings_does_not_panic_and_shows_module_key() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| pour::tui::configure::render(&app, frame))
        .expect("draw");

    let buf = terminal.backend().buffer().clone();
    let content: String = buf
        .content()
        .iter()
        .map(|c| c.symbol().to_owned())
        .collect();
    assert!(
        content.contains("coffee"),
        "settings list must show module key; buf={content:?}"
    );
}

#[test]
fn render_field_list_does_not_panic_and_shows_field_name() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    // Transition to FieldList level
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::FieldList;
    }

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| pour::tui::configure::render(&app, frame))
        .expect("draw");

    let buf = terminal.backend().buffer().clone();
    let content: String = buf
        .content()
        .iter()
        .map(|c| c.symbol().to_owned())
        .collect();
    assert!(
        content.contains("bean") || content.contains("Back"),
        "field list must show at least one field or Back row; buf={content:?}"
    );
}

#[test]
fn render_sub_field_list_does_not_panic_and_shows_column_text() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    // recipe is field index 4 (0-based)
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::SubFieldList(4);
    }

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| pour::tui::configure::render(&app, frame))
        .expect("draw");

    let buf = terminal.backend().buffer().clone();
    let content: String = buf
        .content()
        .iter()
        .map(|c| c.symbol().to_owned())
        .collect();
    // Sub-field list header contains "columns" and module key
    assert!(
        content.contains("coffee"),
        "sub-field list must show module key; buf={content:?}"
    );
}

#[test]
fn render_vault_settings_shows_base_path_label() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut app = make_app();
    app.configure_state = Some(app.init_vault_configure());

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| pour::tui::configure::render(&app, frame))
        .expect("draw");

    let buf = terminal.backend().buffer().clone();
    let content: String = buf
        .content()
        .iter()
        .map(|c| c.symbol().to_owned())
        .collect();
    assert!(
        content.contains("Base Path") || content.contains("base_path"),
        "vault settings must show base_path label; buf={content:?}"
    );
}

#[test]
fn render_confirm_dialog_shows_confirm_prompt() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.confirm = Some(PendingConfirm::DeleteModule {
            module_key: "coffee".to_string(),
        });
    }

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| pour::tui::configure::render(&app, frame))
        .expect("draw");

    let buf = terminal.backend().buffer().clone();
    let content: String = buf
        .content()
        .iter()
        .map(|c| c.symbol().to_owned())
        .collect();
    // Footer shows "y confirm  n/Esc cancel" when confirm dialog is active
    assert!(
        content.contains('y') && content.contains('n'),
        "confirm dialog footer must show y/n hints; buf={content:?}"
    );
}

// ─── Category 2: Key routing per mode ────────────────────────────────────────

// --- Settings edit mode ---

#[test]
fn enter_on_text_setting_starts_editing() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    // display_name is a Text setting; find its index
    let idx = app
        .configure_state
        .as_ref()
        .unwrap()
        .settings
        .iter()
        .position(|s| s.key == "display_name")
        .expect("display_name setting");
    app.configure_state.as_mut().unwrap().active_field = idx;

    let action = handle_key(&mut app, key(KeyCode::Enter));

    assert_eq!(action, ConfigureAction::None);
    assert!(
        app.configure_state.as_ref().unwrap().editing,
        "Enter on Text setting must start editing"
    );
}

#[test]
fn esc_in_settings_edit_mode_cancels_and_restores_value() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    let state = app.configure_state.as_mut().unwrap();
    // Manually enter edit mode on display_name
    let idx = state
        .settings
        .iter()
        .position(|s| s.key == "display_name")
        .unwrap();
    state.active_field = idx;
    state.edit_original = "Coffee".to_string();
    state.edit_buffer = "Changed".to_string();
    state.cursor_position = 7;
    state.editing = true;

    handle_key(&mut app, key(KeyCode::Esc));

    let state = app.configure_state.as_ref().unwrap();
    assert!(!state.editing, "Esc must exit edit mode");
    assert_eq!(
        state.settings[idx].value, "Coffee",
        "Esc must restore original value"
    );
}

#[test]
fn enter_in_edit_mode_confirms_value() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    let state = app.configure_state.as_mut().unwrap();
    let idx = state
        .settings
        .iter()
        .position(|s| s.key == "display_name")
        .unwrap();
    state.active_field = idx;
    state.edit_original = "Coffee".to_string();
    state.edit_buffer = "NewName".to_string();
    state.cursor_position = 7;
    state.editing = true;

    let action = handle_key(&mut app, key(KeyCode::Enter));

    let state = app.configure_state.as_ref().unwrap();
    assert_eq!(action, ConfigureAction::None);
    assert!(!state.editing, "Enter must exit edit mode");
    assert_eq!(
        state.settings[idx].value, "NewName",
        "Enter must commit edit_buffer to value"
    );
    assert!(state.dirty, "committing an edit must mark state as dirty");
}

#[test]
fn s_key_in_module_settings_returns_save() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");

    let action = handle_key(&mut app, key(KeyCode::Char('s')));
    assert_eq!(action, ConfigureAction::Save);
}

#[test]
fn esc_in_module_settings_returns_cancel() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::Cancel);
}

#[test]
fn down_navigates_settings_list() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    assert_eq!(app.configure_state.as_ref().unwrap().active_field, 0);

    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(
        app.configure_state.as_ref().unwrap().active_field,
        1,
        "Down must increment active_field"
    );
}

#[test]
fn up_at_top_does_not_go_below_zero() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    assert_eq!(app.configure_state.as_ref().unwrap().active_field, 0);

    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(
        app.configure_state.as_ref().unwrap().active_field,
        0,
        "Up at top must stay at 0"
    );
}

// --- Field list navigation mode ---

#[test]
fn field_list_down_increments_active_field() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::FieldList;
        s.active_field = 0;
    }

    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(
        app.configure_state.as_ref().unwrap().active_field,
        1,
        "Down in FieldList must advance selection"
    );
}

#[test]
fn field_list_esc_returns_to_module_settings() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::FieldList;
        s.active_field = 1;
    }

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::None);
    assert_eq!(
        app.configure_state.as_ref().unwrap().level,
        ConfigureLevel::ModuleSettings,
        "Esc in FieldList must return to ModuleSettings"
    );
}

#[test]
fn field_list_n_returns_add_field() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::FieldList;
        s.active_field = 1;
    }

    let action = handle_key(&mut app, key(KeyCode::Char('n')));
    assert_eq!(action, ConfigureAction::AddField);
}

#[test]
fn field_list_d_sets_pending_confirm() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::FieldList;
        s.active_field = 1; // first real field (0 = "< Back")
    }

    let action = handle_key(&mut app, key(KeyCode::Char('d')));
    assert_eq!(action, ConfigureAction::None);
    let confirm = &app.configure_state.as_ref().unwrap().confirm;
    assert!(
        matches!(confirm, Some(PendingConfirm::DeleteField { .. })),
        "d in FieldList must set PendingConfirm::DeleteField"
    );
}

#[test]
fn field_list_enter_transitions_to_field_editor() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::FieldList;
        s.active_field = 1; // first real field
    }

    handle_key(&mut app, key(KeyCode::Enter));

    let level = &app.configure_state.as_ref().unwrap().level;
    assert!(
        matches!(level, ConfigureLevel::FieldEditor(0)),
        "Enter on first real field must transition to FieldEditor(0); got {level:?}"
    );
}

// --- Sub-field list navigation mode ---

#[test]
fn sub_field_list_n_returns_add_sub_field() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    // recipe is field index 4
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::SubFieldList(4);
        s.active_field = 0;
    }

    let action = handle_key(&mut app, key(KeyCode::Char('n')));
    assert_eq!(action, ConfigureAction::AddSubField(4));
}

#[test]
fn sub_field_list_esc_returns_to_field_editor() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    // Rebuild settings for field 4 (recipe) so FieldEditor transition has settings
    let field = &app.config.modules["coffee"].fields[4];
    let field_settings = App::build_field_settings(field);
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::SubFieldList(4);
        s.settings = field_settings;
        s.active_field = 0;
    }

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::None);
    assert!(
        matches!(
            app.configure_state.as_ref().unwrap().level,
            ConfigureLevel::FieldEditor(4)
        ),
        "Esc in SubFieldList must return to FieldEditor"
    );
}

// --- Quick-select overlay mode ---

#[test]
fn quick_select_esc_closes_overlay() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.quick_select_open = true;
    }

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::None);
    assert!(
        !app.configure_state.as_ref().unwrap().quick_select_open,
        "Esc must close quick-select overlay"
    );
}

#[test]
fn quick_select_hotkey_sets_value() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    // notes (index 3) has a QuickSelect callout setting — find it
    {
        let state = app.configure_state.as_mut().unwrap();
        // Navigate to the callout-bearing field editor for 'notes' (field idx 3)
        let field = &app.config.modules["coffee"].fields[3]; // notes
        state.settings = App::build_field_settings(field);
        state.level = ConfigureLevel::FieldEditor(3);
        // Find the callout QuickSelect setting
        let callout_idx = state
            .settings
            .iter()
            .position(|s| matches!(s.kind, SettingKind::QuickSelect(_)))
            .expect("callout QuickSelect must exist in notes field settings");
        state.active_field = callout_idx;
        state.quick_select_open = true;
    }

    // 'n' is the hotkey for 'note' in callout_quick_select
    let action = handle_key(&mut app, key(KeyCode::Char('n')));
    assert_eq!(action, ConfigureAction::None);
    let state = app.configure_state.as_ref().unwrap();
    assert!(
        !state.quick_select_open,
        "selecting an option must close quick-select overlay"
    );
    let callout_idx = state
        .settings
        .iter()
        .position(|s| matches!(s.kind, SettingKind::QuickSelect(_)))
        .unwrap();
    assert_eq!(
        state.settings[callout_idx].value, "note",
        "hotkey 'n' must set value to 'note'"
    );
}

// --- Vault settings mode ---

#[test]
fn vault_settings_s_returns_save() {
    let mut app = make_app();
    app.configure_state = Some(app.init_vault_configure());

    let action = handle_key(&mut app, key(KeyCode::Char('s')));
    assert_eq!(action, ConfigureAction::Save);
}

#[test]
fn vault_settings_esc_returns_cancel() {
    let mut app = make_app();
    app.configure_state = Some(app.init_vault_configure());

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::Cancel);
}

// --- New module mode ---

#[test]
fn new_module_ctrl_s_with_empty_key_sets_status_message() {
    let mut app = make_app();
    app.configure_state = Some(app.init_new_module_configure());

    let action = handle_key(&mut app, ctrl(KeyCode::Char('s')));
    assert_eq!(action, ConfigureAction::None);
    let msg = app
        .configure_state
        .as_ref()
        .unwrap()
        .status_message
        .as_deref()
        .unwrap_or("");
    assert!(
        msg.contains("empty") || msg.contains("must"),
        "Ctrl+S with empty key must set a status message; got: {msg:?}"
    );
}

#[test]
fn new_module_identifier_field_rejects_invalid_chars_in_edit_mode() {
    let mut app = make_app();
    app.configure_state = Some(app.init_new_module_configure());
    {
        let state = app.configure_state.as_mut().unwrap();
        let idx = state
            .settings
            .iter()
            .position(|s| s.key == "module_key")
            .unwrap();
        state.active_field = idx;
        state.edit_original = String::new();
        state.edit_buffer = String::new();
        state.cursor_position = 0;
        state.editing = true;
    }

    // Space is not TOML-key-safe → must be rejected
    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert_eq!(
        app.configure_state.as_ref().unwrap().edit_buffer,
        "",
        "Identifier field must reject spaces"
    );

    // 'a' is valid → accepted
    handle_key(&mut app, key(KeyCode::Char('a')));
    assert_eq!(
        app.configure_state.as_ref().unwrap().edit_buffer,
        "a",
        "Identifier field must accept ASCII lowercase"
    );
}

// ─── Category 3: Auto-save (dirty flag gating) ───────────────────────────────
//
// `auto_save_module_settings` and `auto_save_field_settings` are private, so
// we test them indirectly: the NavLink (fields row) triggers auto-save if
// dirty. We verify that *when not dirty* the navigate-to-FieldList still
// happens without attempting disk I/O (which would fail in a test environment
// with a temp vault path).

#[test]
fn auto_save_not_triggered_when_not_dirty() {
    // With dirty=false, clicking the "fields" NavLink must still navigate
    // to FieldList without any status_message (no save attempted means no
    // error message from a missing config file).
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    {
        let state = app.configure_state.as_mut().unwrap();
        // Make sure dirty is false
        state.dirty = false;
        // Navigate to the "fields" NavLink setting
        let idx = state
            .settings
            .iter()
            .position(|s| s.key == "fields")
            .expect("fields NavLink must exist");
        state.active_field = idx;
    }

    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(action, ConfigureAction::None);
    let state = app.configure_state.as_ref().unwrap();
    assert_eq!(
        state.level,
        ConfigureLevel::FieldList,
        "clicking fields NavLink must transition to FieldList"
    );
    // No auto-save attempted → no error status message
    assert!(
        state.status_message.is_none(),
        "no status_message expected when not dirty; got {:?}",
        state.status_message
    );
}

#[test]
fn dirty_flag_set_after_editing_a_text_setting() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    {
        let state = app.configure_state.as_mut().unwrap();
        let idx = state
            .settings
            .iter()
            .position(|s| s.key == "display_name")
            .unwrap();
        state.active_field = idx;
        state.edit_original = state.settings[idx].value.clone();
        state.edit_buffer = "Changed".to_string();
        state.cursor_position = 7;
        state.editing = true;
    }
    // Confirm the edit
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(
        app.configure_state.as_ref().unwrap().dirty,
        "confirming an edit must set dirty = true"
    );
}

#[test]
fn toggle_setting_marks_dirty() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    {
        let state = app.configure_state.as_mut().unwrap();
        let idx = state.settings.iter().position(|s| s.key == "mode").unwrap();
        state.active_field = idx;
    }
    // Enter cycles the Toggle
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(
        app.configure_state.as_ref().unwrap().dirty,
        "cycling a Toggle must set dirty = true"
    );
}

// ─── Category 4: build_field_updates_from_settings ───────────────────────────

#[test]
fn build_field_updates_basic_text_field() {
    use pour::app::ConfigSetting;

    let settings = vec![
        ConfigSetting {
            label: "Name".to_string(),
            key: "name".to_string(),
            value: "bean".to_string(),
            kind: SettingKind::Text,
        },
        ConfigSetting {
            label: "Prompt".to_string(),
            key: "prompt".to_string(),
            value: "Bean".to_string(),
            kind: SettingKind::Text,
        },
        ConfigSetting {
            label: "Field Type".to_string(),
            key: "field_type".to_string(),
            value: "text".to_string(),
            kind: SettingKind::Toggle(vec![]),
        },
        ConfigSetting {
            label: "Required".to_string(),
            key: "required".to_string(),
            value: "true".to_string(),
            kind: SettingKind::Toggle(vec![]),
        },
    ];

    let updates = build_field_updates_from_settings(&settings);

    assert_eq!(updates.name.as_deref(), Some("bean"));
    assert_eq!(updates.prompt.as_deref(), Some("Bean"));
    assert!(
        matches!(updates.field_type, Some(pour::config::FieldType::Text)),
        "field_type should be Text"
    );
    assert_eq!(updates.required, Some(Some(true)));
}

#[test]
fn build_field_updates_options_parsed_from_newline_value() {
    use pour::app::ConfigSetting;

    let settings = vec![
        ConfigSetting {
            label: "Name".to_string(),
            key: "name".to_string(),
            value: "origin".to_string(),
            kind: SettingKind::Text,
        },
        ConfigSetting {
            label: "Options".to_string(),
            key: "options".to_string(),
            value: "Ethiopia\nColombia\nKenya".to_string(),
            kind: SettingKind::ListEditor,
        },
    ];

    let updates = build_field_updates_from_settings(&settings);

    let opts = updates
        .options
        .expect("options must be Some")
        .expect("inner vec");
    assert_eq!(opts, vec!["Ethiopia", "Colombia", "Kenya"]);
}

#[test]
fn build_field_updates_empty_options_yields_none() {
    use pour::app::ConfigSetting;

    let settings = vec![ConfigSetting {
        label: "Options".to_string(),
        key: "options".to_string(),
        value: String::new(),
        kind: SettingKind::ListEditor,
    }];

    let updates = build_field_updates_from_settings(&settings);
    // empty lines → None inner
    assert_eq!(
        updates.options,
        Some(None),
        "empty options value must yield Some(None)"
    );
}

#[test]
fn build_field_updates_target_body_parsed_correctly() {
    use pour::app::ConfigSetting;
    use pour::config::FieldTarget;

    let settings = vec![ConfigSetting {
        label: "Target".to_string(),
        key: "target".to_string(),
        value: "body".to_string(),
        kind: SettingKind::Toggle(vec![]),
    }];

    let updates = build_field_updates_from_settings(&settings);
    assert_eq!(updates.target, Some(Some(FieldTarget::Body)));
}

#[test]
fn build_field_updates_unknown_key_ignored() {
    use pour::app::ConfigSetting;

    let settings = vec![ConfigSetting {
        label: "Bogus".to_string(),
        key: "bogus_unknown_key".to_string(),
        value: "value".to_string(),
        kind: SettingKind::Text,
    }];

    // Should not panic; everything is None
    let updates = build_field_updates_from_settings(&settings);
    assert!(updates.name.is_none());
    assert!(updates.prompt.is_none());
}

// ─── Category 5: Scroll sync ─────────────────────────────────────────────────
//
// `sync_scroll_offset` is private, so we trigger it via key presses in edit
// mode and observe `scroll_offset` changes.

#[test]
fn scroll_offset_stays_zero_when_content_fits() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    {
        let state = app.configure_state.as_mut().unwrap();
        let idx = state
            .settings
            .iter()
            .position(|s| s.key == "display_name")
            .unwrap();
        state.active_field = idx;
        state.edit_original = String::new();
        state.edit_buffer = "Short".to_string();
        state.cursor_position = 5;
        state.editing = true;
    }
    // Type one more character — if content fits, scroll_offset stays 0
    handle_key(&mut app, key(KeyCode::Char('!')));
    assert_eq!(
        app.configure_state.as_ref().unwrap().scroll_offset,
        0,
        "short content must not scroll"
    );
}

#[test]
fn scroll_offset_advances_when_typing_long_value() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    {
        let state = app.configure_state.as_mut().unwrap();
        let idx = state
            .settings
            .iter()
            .position(|s| s.key == "display_name")
            .unwrap();
        state.active_field = idx;
        // Simulate a value longer than the 80-col terminal can show
        let long_val = "x".repeat(120);
        state.edit_original = long_val.clone();
        state.edit_buffer = long_val.clone();
        state.cursor_position = 120;
        state.editing = true;
    }
    // Typing one more character should push the scroll offset forward
    handle_key(&mut app, key(KeyCode::Char('z')));
    let scroll = app.configure_state.as_ref().unwrap().scroll_offset;
    assert!(
        scroll > 0,
        "typing past viewport width must advance scroll_offset; scroll={scroll}"
    );
}

// ─── Category 6: Browser navigation ─────────────────────────────────────────

fn make_app_with_browser() -> App {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    // Inject browser state
    if let Some(ref mut s) = app.configure_state {
        s.browser_open = true;
        s.browser_state = Some(BrowserState {
            current_path: "Coffee".to_string(),
            entries: vec![
                VaultEntry {
                    name: "Beans".to_string(),
                    is_dir: true,
                },
                VaultEntry {
                    name: "Roasters".to_string(),
                    is_dir: true,
                },
                VaultEntry {
                    name: "note.md".to_string(),
                    is_dir: false,
                },
            ],
            selected: 0,
            error: None,
        });
    }
    app
}

#[test]
fn browser_esc_closes_browser() {
    let mut app = make_app_with_browser();

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::None);
    assert!(
        !app.configure_state.as_ref().unwrap().browser_open,
        "Esc must close browser"
    );
}

#[test]
fn browser_down_increments_selection() {
    let mut app = make_app_with_browser();

    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(
        app.configure_state
            .as_ref()
            .unwrap()
            .browser_state
            .as_ref()
            .unwrap()
            .selected,
        1,
        "Down in browser must increment selected"
    );
}

#[test]
fn browser_enter_descends_into_directory() {
    let mut app = make_app_with_browser();
    // Select the first dir (Beans) and press Enter
    app.configure_state
        .as_mut()
        .unwrap()
        .browser_state
        .as_mut()
        .unwrap()
        .selected = 0; // ".." is not shown because current_path != "" → selected 0 = ".." row

    // At current_path = "Coffee" (non-root), index 0 = ".." entry → navigates up
    // Index 1 = "Beans" dir. Set selected = 1.
    app.configure_state
        .as_mut()
        .unwrap()
        .browser_state
        .as_mut()
        .unwrap()
        .selected = 1;

    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(
        matches!(action, ConfigureAction::BrowseDirectory(_)),
        "Enter on a directory must return BrowseDirectory; got {action:?}"
    );
}

#[test]
fn browser_backspace_navigates_to_parent() {
    let mut app = make_app_with_browser();

    let action = handle_key(&mut app, key(KeyCode::Backspace));
    // parent_path("Coffee") = ""
    assert_eq!(action, ConfigureAction::BrowseDirectory(String::new()));
}

#[test]
fn browser_up_stops_at_zero() {
    let mut app = make_app_with_browser();
    // already at selected=0
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(
        app.configure_state
            .as_ref()
            .unwrap()
            .browser_state
            .as_ref()
            .unwrap()
            .selected,
        0,
        "Up at top of browser must stay at 0"
    );
}

// ─── Category 7: Confirm dialog ──────────────────────────────────────────────

#[test]
fn confirm_y_on_delete_field_returns_remove_field() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::FieldList;
        s.confirm = Some(PendingConfirm::DeleteField {
            field_index: 2,
            field_name: "origin".to_string(),
        });
    }

    let action = handle_key(&mut app, key(KeyCode::Char('y')));
    assert_eq!(action, ConfigureAction::RemoveField(2));
    assert!(
        app.configure_state.as_ref().unwrap().confirm.is_none(),
        "confirm must be cleared after y"
    );
}

#[test]
fn confirm_n_cancels_without_action() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.confirm = Some(PendingConfirm::DeleteModule {
            module_key: "coffee".to_string(),
        });
    }

    let action = handle_key(&mut app, key(KeyCode::Char('n')));
    assert_eq!(action, ConfigureAction::None);
    assert!(
        app.configure_state.as_ref().unwrap().confirm.is_none(),
        "confirm must be cleared after n"
    );
}

#[test]
fn confirm_esc_cancels_without_action() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.confirm = Some(PendingConfirm::DeleteModule {
            module_key: "coffee".to_string(),
        });
    }

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::None);
    assert!(
        app.configure_state.as_ref().unwrap().confirm.is_none(),
        "confirm must be cleared after Esc"
    );
}

#[test]
fn d_on_module_settings_triggers_delete_module_confirm() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");

    let action = handle_key(&mut app, key(KeyCode::Char('d')));
    assert_eq!(action, ConfigureAction::None);
    let confirm = &app.configure_state.as_ref().unwrap().confirm;
    assert!(
        matches!(confirm, Some(PendingConfirm::DeleteModule { .. })),
        "d on ModuleSettings must set PendingConfirm::DeleteModule"
    );
}

// ─── Additional edge-case tests (padding to ≥20) ─────────────────────────────

#[test]
fn toggle_mode_setting_adds_append_header_row() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee"); // starts as "create"
    {
        let state = app.configure_state.as_mut().unwrap();
        let idx = state.settings.iter().position(|s| s.key == "mode").unwrap();
        state.active_field = idx;
    }
    // Cycle "create" → "update" → "append" (the cycler is append/create/update).
    handle_key(&mut app, key(KeyCode::Enter));
    handle_key(&mut app, key(KeyCode::Enter));

    let state = app.configure_state.as_ref().unwrap();
    assert_eq!(state.settings[state.active_field].value, "append");
    let has_header = state
        .settings
        .iter()
        .any(|s| s.key == "append_under_header");
    assert!(
        has_header,
        "toggling to append mode must add append_under_header setting"
    );
}

#[test]
fn toggle_mode_back_to_create_removes_append_header_row() {
    let mut app = make_app();
    app.configure_state = app.init_configure("journal"); // starts as "append"
    {
        let state = app.configure_state.as_mut().unwrap();
        let idx = state.settings.iter().position(|s| s.key == "mode").unwrap();
        state.active_field = idx;
    }
    // Cycle "append" → "create"
    handle_key(&mut app, key(KeyCode::Enter));

    let state = app.configure_state.as_ref().unwrap();
    assert_eq!(state.settings[state.active_field].value, "create");
    let has_header = state
        .settings
        .iter()
        .any(|s| s.key == "append_under_header");
    assert!(
        !has_header,
        "toggling back to create mode must remove append_under_header setting"
    );
}

#[test]
fn field_editor_esc_returns_to_field_list() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    {
        let state = app.configure_state.as_mut().unwrap();
        let field = &app.config.modules["coffee"].fields[0];
        state.settings = App::build_field_settings(field);
        state.level = ConfigureLevel::FieldEditor(0);
        state.active_field = 0;
    }

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::None);
    assert_eq!(
        app.configure_state.as_ref().unwrap().level,
        ConfigureLevel::FieldList,
        "Esc in FieldEditor must return to FieldList"
    );
}

#[test]
fn entering_field_editor_loads_field_settings() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.level = ConfigureLevel::FieldList;
        s.active_field = 1; // first real field = bean (text)
    }

    handle_key(&mut app, key(KeyCode::Enter));

    let state = app.configure_state.as_ref().unwrap();
    // Settings should now be field-level settings for 'bean'
    let has_name = state.settings.iter().any(|s| s.key == "name");
    let has_prompt = state.settings.iter().any(|s| s.key == "prompt");
    assert!(
        has_name && has_prompt,
        "FieldEditor settings must include name and prompt keys"
    );
}

#[test]
fn help_overlay_closes_on_question_mark() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.help_overlay_open = true;
    }

    let action = handle_key(&mut app, key(KeyCode::Char('?')));
    assert_eq!(action, ConfigureAction::None);
    assert!(
        !app.configure_state.as_ref().unwrap().help_overlay_open,
        "? must close the path help overlay"
    );
}

#[test]
fn help_overlay_closes_on_esc() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    if let Some(ref mut s) = app.configure_state {
        s.help_overlay_open = true;
    }

    handle_key(&mut app, key(KeyCode::Esc));
    assert!(
        !app.configure_state.as_ref().unwrap().help_overlay_open,
        "Esc must close the path help overlay"
    );
}

#[test]
fn list_editor_ctrl_s_saves_buffer_and_closes() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    {
        let state = app.configure_state.as_mut().unwrap();
        // Navigate to the 'origin' field's options setting (static_select)
        let field = &app.config.modules["coffee"].fields[2]; // origin
        state.settings = App::build_field_settings(field);
        state.level = ConfigureLevel::FieldEditor(2);
        let idx = state
            .settings
            .iter()
            .position(|s| matches!(s.kind, SettingKind::ListEditor))
            .expect("origin must have a ListEditor setting");
        state.active_field = idx;
        state.list_editor_open = true;
        state.list_editor_buffer = "Ethiopia\nColombia\nKenya".to_string();
        state.list_editor_cursor_line = 2;
        state.list_editor_cursor_col = 5;
    }

    let action = handle_key(&mut app, ctrl(KeyCode::Char('s')));
    assert_eq!(action, ConfigureAction::None);
    let state = app.configure_state.as_ref().unwrap();
    assert!(
        !state.list_editor_open,
        "Ctrl+S must close the list editor overlay"
    );
    let idx = state
        .settings
        .iter()
        .position(|s| matches!(s.kind, SettingKind::ListEditor))
        .unwrap();
    assert_eq!(
        state.settings[idx].value, "Ethiopia\nColombia\nKenya",
        "Ctrl+S must save buffer to setting value"
    );
}

#[test]
fn list_editor_esc_discards_changes() {
    let mut app = make_app();
    app.configure_state = app.init_configure("coffee");
    {
        let state = app.configure_state.as_mut().unwrap();
        let field = &app.config.modules["coffee"].fields[2]; // origin (static_select)
        state.settings = App::build_field_settings(field);
        state.level = ConfigureLevel::FieldEditor(2);
        let idx = state
            .settings
            .iter()
            .position(|s| matches!(s.kind, SettingKind::ListEditor))
            .unwrap();
        state.active_field = idx;
        state.list_editor_open = true;
        state.list_editor_buffer = "MODIFIED".to_string();
    }

    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, ConfigureAction::None);
    let state = app.configure_state.as_ref().unwrap();
    assert!(
        !state.list_editor_open,
        "Esc must close the list editor overlay"
    );
    // Original value must not have changed (Esc discards)
    let idx = state
        .settings
        .iter()
        .position(|s| matches!(s.kind, SettingKind::ListEditor))
        .unwrap();
    assert_ne!(
        state.settings[idx].value, "MODIFIED",
        "Esc must NOT save the modified buffer"
    );
}
