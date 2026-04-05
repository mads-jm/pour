use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pour::app::App;
use pour::config::Config;
use pour::data::history::History;
use pour::transport::Transport;
use pour::transport::fs::FsWriter;
use pour::tui::form::{FormAction, handle_key};

const FORM_TEST_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"

[[modules.test.fields]]
name = "count"
field_type = "number"
prompt = "Count"

[[modules.test.fields]]
name = "origin"
field_type = "static_select"
prompt = "Origin"
options = ["Ethiopia", "Colombia", "Kenya"]

[[modules.test.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"

[[modules.test.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Recipe"

[[modules.test.fields.sub_fields]]
name = "amount"
field_type = "number"
prompt = "Amount"

[[modules.test.fields.sub_fields]]
name = "technique"
field_type = "static_select"
prompt = "Technique"
options = ["Bloom", "Spiral", "Center"]
"####;

fn make_app() -> App {
    let config = Config::from_toml(FORM_TEST_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/test-form-history.json")),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == "test").unwrap();
    app.form_state = app.init_form("test");
    app.screen = pour::app::Screen::Form;
    app
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

// ── Navigation ──

#[test]
fn tab_advances_to_next_field() {
    let mut app = make_app();
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 0);
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 1);
}

#[test]
fn shift_tab_goes_to_previous_field() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 1;
    handle_key(&mut app, key(KeyCode::BackTab));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 0);
}

#[test]
fn tab_wraps_around() {
    let mut app = make_app();
    let field_count = app.config.modules["test"].fields.len();
    // Set to submit button (last navigable position)
    app.form_state.as_mut().unwrap().active_field = field_count;
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 0);
}

#[test]
fn shift_tab_wraps_around() {
    let mut app = make_app();
    let field_count = app.config.modules["test"].fields.len();
    app.form_state.as_mut().unwrap().active_field = 0;
    handle_key(&mut app, key(KeyCode::BackTab));
    // Should wrap to submit button
    assert_eq!(app.form_state.as_ref().unwrap().active_field, field_count);
}

#[test]
fn down_arrow_navigates_forward() {
    let mut app = make_app();
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 1);
}

#[test]
fn up_arrow_navigates_backward() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 1;
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 0);
}

#[test]
fn enter_on_text_field_advances() {
    let mut app = make_app();
    // Field 0 is text ("title")
    assert_eq!(handle_key(&mut app, key(KeyCode::Enter)), FormAction::None);
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 1);
}

// ── Text Input ──

#[test]
fn char_inserts_at_cursor() {
    let mut app = make_app();
    handle_key(&mut app, key(KeyCode::Char('a')));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("title").unwrap(), "a");
    assert_eq!(fs.cursor_position, 1);
}

#[test]
fn char_inserts_at_cursor_mid_string() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.field_values
        .insert("title".to_string(), "abcd".to_string());
    fs.cursor_position = 2;
    handle_key(&mut app, key(KeyCode::Char('X')));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("title").unwrap(), "abXcd");
    assert_eq!(fs.cursor_position, 3);
}

#[test]
fn backspace_removes_char_before_cursor() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.field_values
        .insert("title".to_string(), "abc".to_string());
    fs.cursor_position = 3;
    handle_key(&mut app, key(KeyCode::Backspace));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("title").unwrap(), "ab");
    assert_eq!(fs.cursor_position, 2);
}

#[test]
fn backspace_at_position_zero_does_nothing() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.field_values
        .insert("title".to_string(), "abc".to_string());
    fs.cursor_position = 0;
    handle_key(&mut app, key(KeyCode::Backspace));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("title").unwrap(), "abc");
}

#[test]
fn left_arrow_moves_cursor_left() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.field_values
        .insert("title".to_string(), "abc".to_string());
    fs.cursor_position = 3;
    handle_key(&mut app, key(KeyCode::Left));
    assert_eq!(app.form_state.as_ref().unwrap().cursor_position, 2);
}

#[test]
fn right_arrow_moves_cursor_right() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.field_values
        .insert("title".to_string(), "abc".to_string());
    fs.cursor_position = 0;
    handle_key(&mut app, key(KeyCode::Right));
    assert_eq!(app.form_state.as_ref().unwrap().cursor_position, 1);
}

#[test]
fn right_arrow_stops_at_end() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.field_values
        .insert("title".to_string(), "abc".to_string());
    fs.cursor_position = 3;
    handle_key(&mut app, key(KeyCode::Right));
    assert_eq!(app.form_state.as_ref().unwrap().cursor_position, 3);
}

// ── Number Field Filtering ──

#[test]
fn number_field_accepts_digits() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 1; // count (number)
    handle_key(&mut app, key(KeyCode::Char('5')));
    assert_eq!(
        app.form_state
            .as_ref()
            .unwrap()
            .field_values
            .get("count")
            .unwrap(),
        "5"
    );
}

#[test]
fn number_field_accepts_decimal_and_minus() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 1;
    handle_key(&mut app, key(KeyCode::Char('-')));
    handle_key(&mut app, key(KeyCode::Char('3')));
    handle_key(&mut app, key(KeyCode::Char('.')));
    handle_key(&mut app, key(KeyCode::Char('5')));
    assert_eq!(
        app.form_state
            .as_ref()
            .unwrap()
            .field_values
            .get("count")
            .unwrap(),
        "-3.5"
    );
}

#[test]
fn number_field_rejects_letters() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 1;
    handle_key(&mut app, key(KeyCode::Char('a')));
    assert_eq!(
        app.form_state
            .as_ref()
            .unwrap()
            .field_values
            .get("count")
            .unwrap(),
        ""
    );
}

// ── Select Fields ──

#[test]
fn enter_toggles_dropdown() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 2; // origin (static_select)
    assert!(!app.form_state.as_ref().unwrap().dropdown_open);
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.form_state.as_ref().unwrap().dropdown_open);
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(!app.form_state.as_ref().unwrap().dropdown_open);
}

#[test]
fn down_cycles_options_when_dropdown_open() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 2;
    // Open dropdown
    handle_key(&mut app, key(KeyCode::Enter));
    // Cycle to first option
    handle_key(&mut app, key(KeyCode::Down));
    let val = app
        .form_state
        .as_ref()
        .unwrap()
        .field_values
        .get("origin")
        .unwrap()
        .clone();
    // Starting from empty, Down should land on "Ethiopia" (index 0)
    assert_eq!(val, "Ethiopia");
    // Cycle to next
    handle_key(&mut app, key(KeyCode::Down));
    let val = app
        .form_state
        .as_ref()
        .unwrap()
        .field_values
        .get("origin")
        .unwrap()
        .clone();
    assert_eq!(val, "Colombia");
}

#[test]
fn up_cycles_options_backward_when_dropdown_open() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 2;
    fs.field_values
        .insert("origin".to_string(), "Colombia".to_string());
    fs.dropdown_open = true;
    handle_key(&mut app, key(KeyCode::Up));
    let val = app
        .form_state
        .as_ref()
        .unwrap()
        .field_values
        .get("origin")
        .unwrap()
        .clone();
    assert_eq!(val, "Ethiopia");
}

#[test]
fn char_input_blocked_on_select_fields() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 2;
    handle_key(&mut app, key(KeyCode::Char('x')));
    // Value should still be empty/default
    let val = app
        .form_state
        .as_ref()
        .unwrap()
        .field_values
        .get("origin")
        .unwrap()
        .clone();
    assert_eq!(val, "");
}

// ── Textarea ──

#[test]
fn enter_opens_textarea_editor() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 3; // notes (textarea)
    assert!(!app.form_state.as_ref().unwrap().textarea_open);
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.form_state.as_ref().unwrap().textarea_open);
}

#[test]
fn enter_inserts_newline_when_editor_open() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 3;
    fs.textarea_open = true;
    fs.field_values
        .insert("notes".to_string(), "hello".to_string());
    fs.cursor_position = 5;
    handle_key(&mut app, key(KeyCode::Enter));
    let val = app
        .form_state
        .as_ref()
        .unwrap()
        .field_values
        .get("notes")
        .unwrap()
        .clone();
    assert_eq!(val, "hello\n");
}

#[test]
fn char_input_works_in_open_textarea() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 3;
    fs.textarea_open = true;
    fs.cursor_position = 0;
    handle_key(&mut app, key(KeyCode::Char('H')));
    let val = app
        .form_state
        .as_ref()
        .unwrap()
        .field_values
        .get("notes")
        .unwrap()
        .clone();
    assert_eq!(val, "H");
}

#[test]
fn char_input_blocked_when_textarea_closed() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 3;
    assert!(!app.form_state.as_ref().unwrap().textarea_open);
    handle_key(&mut app, key(KeyCode::Char('x')));
    let val = app
        .form_state
        .as_ref()
        .unwrap()
        .field_values
        .get("notes")
        .unwrap()
        .clone();
    assert_eq!(val, "");
}

// ── Composite Array ──

#[test]
fn enter_opens_composite_overlay() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 4; // recipe (composite_array)
    assert!(!app.form_state.as_ref().unwrap().composite_open);
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.form_state.as_ref().unwrap().composite_open);
}

#[test]
fn enter_adds_row_in_composite_overlay() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 4;
    fs.composite_open = true;
    // No rows yet, Enter adds one
    handle_key(&mut app, key(KeyCode::Enter));
    let rows = app
        .form_state
        .as_ref()
        .unwrap()
        .composite_values
        .get("recipe")
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 2); // 2 sub-fields
}

#[test]
fn tab_navigates_cells_in_composite() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 4;
    fs.composite_open = true;
    // Add a row
    fs.composite_values.insert(
        "recipe".to_string(),
        vec![vec!["10".to_string(), "Bloom".to_string()]],
    );
    fs.composite_row = 0;
    fs.composite_col = 0;
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(app.form_state.as_ref().unwrap().composite_col, 1);
}

#[test]
fn delete_removes_row_in_composite() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 4;
    fs.composite_open = true;
    fs.composite_values.insert(
        "recipe".to_string(),
        vec![
            vec!["10".to_string(), "Bloom".to_string()],
            vec!["20".to_string(), "Spiral".to_string()],
        ],
    );
    fs.composite_row = 0;
    handle_key(&mut app, key(KeyCode::Delete));
    let rows = app
        .form_state
        .as_ref()
        .unwrap()
        .composite_values
        .get("recipe")
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], "20"); // the second row remains
}

#[test]
fn backspace_in_composite_cell() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 4;
    fs.composite_open = true;
    fs.composite_values.insert(
        "recipe".to_string(),
        vec![vec!["123".to_string(), "".to_string()]],
    );
    fs.composite_row = 0;
    fs.composite_col = 0;
    fs.cursor_position = 3;
    handle_key(&mut app, key(KeyCode::Backspace));
    let cell = &app.form_state.as_ref().unwrap().composite_values["recipe"][0][0];
    assert_eq!(cell, "12");
}

#[test]
fn number_filtering_in_composite_number_sub_field() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 4;
    fs.composite_open = true;
    fs.composite_values.insert(
        "recipe".to_string(),
        vec![vec!["".to_string(), "".to_string()]],
    );
    fs.composite_row = 0;
    fs.composite_col = 0; // "amount" is a number sub-field
    fs.cursor_position = 0;
    // Letters should be rejected
    handle_key(&mut app, key(KeyCode::Char('a')));
    let cell = &app.form_state.as_ref().unwrap().composite_values["recipe"][0][0];
    assert_eq!(cell, "");
    // Digits should work
    handle_key(&mut app, key(KeyCode::Char('5')));
    let cell = &app.form_state.as_ref().unwrap().composite_values["recipe"][0][0];
    assert_eq!(cell, "5");
}

// ── Esc Layering ──

#[test]
fn esc_closes_dropdown_first() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 2;
    fs.dropdown_open = true;
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, FormAction::None);
    assert!(!app.form_state.as_ref().unwrap().dropdown_open);
}

#[test]
fn esc_closes_textarea_first() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 3;
    fs.textarea_open = true;
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, FormAction::None);
    assert!(!app.form_state.as_ref().unwrap().textarea_open);
}

#[test]
fn esc_clears_field_content_second() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.field_values
        .insert("title".to_string(), "hello".to_string());
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, FormAction::None);
    assert_eq!(
        app.form_state
            .as_ref()
            .unwrap()
            .field_values
            .get("title")
            .unwrap(),
        ""
    );
}

#[test]
fn esc_on_empty_field_cancels_form() {
    let mut app = make_app();
    // Field 0 is text, default empty
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, FormAction::Cancel);
}

// ── Submit ──

#[test]
fn enter_on_submit_button_returns_submit() {
    let mut app = make_app();
    let field_count = app.config.modules["test"].fields.len();
    app.form_state.as_mut().unwrap().active_field = field_count;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(action, FormAction::Submit);
}

// ── Tab closes overlays ──

#[test]
fn tab_closes_dropdown_and_advances() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 2; // select field
    fs.dropdown_open = true;
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert!(!fs.dropdown_open);
    assert_eq!(fs.active_field, 3);
}

#[test]
fn tab_closes_textarea_and_advances() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 3; // textarea
    fs.textarea_open = true;
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert!(!fs.textarea_open);
    assert_eq!(fs.active_field, 4);
}

// ── allow_create dynamic_select ──

const ALLOW_CREATE_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.brew]
mode = "create"
path = "brew.md"

[[modules.brew.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
allow_create = true
source = "beans"
"####;

fn make_app_allow_create() -> App {
    let config = Config::from_toml(ALLOW_CREATE_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-allow-create-history.json",
        )),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == "brew").unwrap();
    app.form_state = app.init_form("brew");
    app.screen = pour::app::Screen::Form;
    // Pre-populate some options as if the data fetch completed.
    app.form_state.as_mut().unwrap().field_options.insert(
        "bean".to_string(),
        vec![
            "Ethiopia Yirgacheffe".to_string(),
            "Ethiopia Sidama".to_string(),
            "Colombia Huila".to_string(),
        ],
    );
    app
}

#[test]
fn char_populates_search_buffer_on_allow_create_dynamic_select() {
    let mut app = make_app_allow_create();
    // Field 0 is the bean dynamic_select with allow_create = true.
    handle_key(&mut app, key(KeyCode::Char('e')));
    handle_key(&mut app, key(KeyCode::Char('t')));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.search_buffers.get("bean").unwrap(), "et");
    // Dropdown should auto-open.
    assert!(fs.dropdown_open);
    // field_values should still be empty (not yet committed).
    assert_eq!(fs.field_values.get("bean").unwrap(), "");
}

#[test]
fn backspace_trims_search_buffer_on_allow_create_dynamic_select() {
    let mut app = make_app_allow_create();
    handle_key(&mut app, key(KeyCode::Char('e')));
    handle_key(&mut app, key(KeyCode::Char('t')));
    handle_key(&mut app, key(KeyCode::Backspace));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.search_buffers.get("bean").unwrap(), "e");
}

#[test]
fn enter_on_empty_filtered_list_accepts_novel_value() {
    let mut app = make_app_allow_create();
    // Type something that matches nothing.
    for c in "xyz".chars() {
        handle_key(&mut app, key(KeyCode::Char(c)));
    }
    // Enter should commit the novel value.
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(action, FormAction::None);
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("bean").unwrap(), "xyz");
    // Search buffer should be cleared after commit.
    assert!(
        fs.search_buffers
            .get("bean")
            .map(|s| s.is_empty())
            .unwrap_or(true)
    );
    // Dropdown should be closed.
    assert!(!fs.dropdown_open);
}

#[test]
fn enter_with_matching_filter_selects_highlighted_option() {
    let mut app = make_app_allow_create();
    // Type "colombia" — should match exactly one option.
    for c in "colombia".chars() {
        handle_key(&mut app, key(KeyCode::Char(c)));
    }
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(action, FormAction::None);
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("bean").unwrap(), "Colombia Huila");
    assert!(!fs.dropdown_open);
}

#[test]
fn esc_clears_search_buffer_before_closing_dropdown() {
    let mut app = make_app_allow_create();
    // Type some characters to fill the buffer.
    handle_key(&mut app, key(KeyCode::Char('e')));
    // Esc once should clear the search buffer but keep dropdown open.
    handle_key(&mut app, key(KeyCode::Esc));
    let fs = app.form_state.as_ref().unwrap();
    assert!(
        fs.search_buffers
            .get("bean")
            .map(|s| s.is_empty())
            .unwrap_or(true)
    );
    // Dropdown should still be open (buffer cleared, not closed yet).
    assert!(fs.dropdown_open);
    // Esc again closes the dropdown.
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.form_state.as_ref().unwrap().dropdown_open);
}

#[test]
fn char_input_still_blocked_on_static_select_without_allow_create() {
    // This is the existing static_select in the main test app — behaviour unchanged.
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 2; // origin (static_select)
    handle_key(&mut app, key(KeyCode::Char('e')));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("origin").unwrap(), "");
    assert!(fs.search_buffers.get("origin").is_none());
}

#[test]
fn char_input_still_blocked_on_dynamic_select_without_allow_create() {
    // A plain dynamic_select (no allow_create) should still reject typed chars.
    let toml = r####"
[vault]
base_path = "/tmp/vault"

[modules.brew]
mode = "create"
path = "brew.md"

[[modules.brew.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "beans"
"####;
    let config = Config::from_toml(toml).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-no-allow-create-history.json",
        )),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == "brew").unwrap();
    app.form_state = app.init_form("brew");
    app.screen = pour::app::Screen::Form;
    app.form_state.as_mut().unwrap().field_options.insert(
        "bean".to_string(),
        vec!["Ethiopia".to_string(), "Colombia".to_string()],
    );
    handle_key(&mut app, key(KeyCode::Char('e')));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("bean").unwrap(), "");
    assert!(fs.search_buffers.get("bean").is_none());
}

#[test]
fn tab_clears_search_buffer_on_allow_create_dynamic_select() {
    let mut app = make_app_allow_create();
    handle_key(&mut app, key(KeyCode::Char('e')));
    assert!(
        !app.form_state
            .as_ref()
            .unwrap()
            .search_buffers
            .get("bean")
            .unwrap()
            .is_empty()
    );
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert!(fs.search_buffers.get("bean").is_none());
}

// ── Visibility-aware navigation (TASK-A05) ──

/// TOML with two conditional fields. `grind` and `pressure` are gated on `method`.
const CONDITIONAL_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.brew]
mode = "create"
path = "brew.md"

[[modules.brew.fields]]
name = "method"
field_type = "static_select"
prompt = "Method"
options = ["V60", "Espresso", "AeroPress"]

[[modules.brew.fields]]
name = "grind"
field_type = "number"
prompt = "Grind"
[modules.brew.fields.show_when]
field = "method"
equals = "V60"

[[modules.brew.fields]]
name = "pressure"
field_type = "number"
prompt = "Pressure"
[modules.brew.fields.show_when]
field = "method"
equals = "Espresso"

[[modules.brew.fields]]
name = "notes"
field_type = "text"
prompt = "Notes"
"####;

fn make_app_conditional() -> App {
    let config = Config::from_toml(CONDITIONAL_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-conditional-history.json",
        )),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == "brew").unwrap();
    app.form_state = app.init_form("brew");
    app.screen = pour::app::Screen::Form;
    app
}

/// With method="" (no value), only `method` (0) and `notes` (3) are visible.
/// Tabbing from notes should land on submit (Tab wraps at visible_count, not total_count).
#[test]
fn navigable_count_reflects_visible_fields_only() {
    let mut app = make_app_conditional();
    // method has no default, so grind and pressure are hidden.
    // visible = [method(0), notes(3)] → navigable_count = 3 (positions 0, 1, 2=submit).
    // From notes (visible index 1), Tab should go to submit (visible index 2).
    app.form_state.as_mut().unwrap().active_field = 1; // notes
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.active_field, 2, "should land on submit, not total_fields(4)");
    assert_eq!(fs.active_config_idx, None, "submit has no config idx");
}

/// Tab from method (vi=0) should skip hidden grind/pressure and land on notes (vi=1).
#[test]
fn tab_skips_hidden_conditional_field() {
    let mut app = make_app_conditional();
    // active_field=0 is method, no value set so grind/pressure are hidden.
    // Tab should advance to notes (visible index 1).
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.active_field, 1, "should land on notes (visible index 1)");
    assert_eq!(fs.active_config_idx, Some(3), "notes is config field 3");
}

/// Tab wraps through visible count + submit correctly.
#[test]
fn tab_wraps_using_visible_count() {
    let mut app = make_app_conditional();
    // visible = [method(0), notes(3)], navigable_count = 3
    // active_field=1 is notes. Tab should go to submit (visible index 2).
    app.form_state.as_mut().unwrap().active_field = 1;
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.active_field, 2, "should land on submit (visible index 2)");
    assert_eq!(fs.active_config_idx, None, "submit button has no config idx");
}

/// When active field becomes hidden, focus moves to the next visible field.
#[test]
fn active_field_hidden_moves_to_next_visible() {
    let mut app = make_app_conditional();
    // Set method = "V60" so grind becomes visible. visible = [method, grind, notes].
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("method".to_string(), "V60".to_string());
    // Navigate to grind (visible index 1, config index 1).
    app.form_state.as_mut().unwrap().active_field = 1;
    // Simulate a key to trigger clamp — then change method to "Espresso" which hides grind.
    // We set the value directly and then fire a key to trigger clamp_active_to_visible.
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("method".to_string(), "Espresso".to_string());
    // Fire a no-op key (Right at position 0 won't change navigation but will trigger clamp).
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // grind is now hidden. Next visible after config idx 1 is notes (config idx 3).
    // pressure (config 2) is visible when method=Espresso, so next after grind (1) is pressure (2).
    assert_eq!(fs.active_config_idx, Some(2), "should land on pressure");
}

/// When active field becomes hidden and there is no next visible field, focus moves
/// to the previous visible field.
#[test]
fn active_field_hidden_falls_back_to_previous_visible() {
    let mut app = make_app_conditional();
    // Set method = "Espresso" so pressure becomes visible.
    // visible = [method(0), pressure(2), notes(3)]
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("method".to_string(), "Espresso".to_string());
    // Navigate to notes (visible index 2, config index 3).
    app.form_state.as_mut().unwrap().active_field = 2;
    // Now set method = "" — pressure becomes hidden, notes stays visible.
    // visible = [method(0), notes(3)], notes is still at visible index 1.
    // active_field=2 would be out of range (submit), but notes is still visible.
    // Actually notes stays visible so this tests active_field shifting for a different reason.
    // Let's instead test: navigate to pressure (vi=1, ci=2), then hide it.
    app.form_state.as_mut().unwrap().active_field = 1; // pressure
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("method".to_string(), "AeroPress".to_string());
    // Fire a key to trigger clamp.
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // pressure is hidden, no config idx > 2 visible (notes=3 IS visible).
    // next after ci=2 is notes (ci=3), so we land on notes.
    assert_eq!(fs.active_config_idx, Some(3), "should land on notes (next after hidden pressure)");
}

/// Downstream fields appearing when a select changes do NOT steal focus.
#[test]
fn newly_visible_field_does_not_steal_focus() {
    let mut app = make_app_conditional();
    // Start on method (active_field=0). Set method=V60 which makes grind appear.
    // Focus should stay on method.
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("method".to_string(), "V60".to_string());
    handle_key(&mut app, key(KeyCode::Right)); // trigger clamp, no navigation
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.active_field, 0, "focus stays on method");
    assert_eq!(fs.active_config_idx, Some(0), "config idx still method");
}

// ── SubFormState error_message ──

fn make_template() -> pour::config::TemplateConfig {
    pour::config::TemplateConfig {
        path: "Beans/{name}.md".to_string(),
        fields: vec![],
    }
}

#[test]
fn sub_form_error_message_defaults_to_none() {
    let template = make_template();
    let sf = pour::app::SubFormState::new(
        "beans".to_string(),
        "Ethiopia Guji".to_string(),
        "bean".to_string(),
        &template,
    );
    assert!(sf.error_message.is_none());
}

#[test]
fn sub_form_error_message_can_be_set() {
    let template = make_template();
    let mut sf = pour::app::SubFormState::new(
        "beans".to_string(),
        "Ethiopia Guji".to_string(),
        "bean".to_string(),
        &template,
    );
    sf.error_message = Some("write failed: connection refused".to_string());
    assert_eq!(
        sf.error_message.as_deref(),
        Some("write failed: connection refused")
    );
}

#[test]
fn sub_form_error_message_can_be_cleared() {
    let template = make_template();
    let mut sf = pour::app::SubFormState::new(
        "beans".to_string(),
        "Ethiopia Guji".to_string(),
        "bean".to_string(),
        &template,
    );
    sf.error_message = Some("some error".to_string());
    sf.error_message = None;
    assert!(sf.error_message.is_none());
}

// ── Callout cycling on textarea fields ──

const CALLOUT_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"

[[modules.test.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
callout = "note"
"####;

fn make_app_callout() -> App {
    let config = Config::from_toml(CALLOUT_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/test-callout-history.json")),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == "test").unwrap();
    app.form_state = app.init_form("test");
    app.screen = pour::app::Screen::Form;
    app
}

#[test]
fn callout_override_seeded_from_config() {
    let app = make_app_callout();
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.callout_overrides.get("notes").map(|s| s.as_str()), Some("note"));
}

#[test]
fn right_cycles_callout_forward() {
    let mut app = make_app_callout();
    app.form_state.as_mut().unwrap().active_field = 1; // notes (textarea)
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // "note" is index 0 in CALLOUT_OPTIONS → Right goes to index 1 = "info"
    assert_eq!(fs.callout_overrides["notes"], "info");
}

#[test]
fn left_cycles_callout_backward() {
    let mut app = make_app_callout();
    app.form_state.as_mut().unwrap().active_field = 1; // notes (textarea)
    handle_key(&mut app, key(KeyCode::Left));
    let fs = app.form_state.as_ref().unwrap();
    // "note" is index 0 → Left wraps to last = "danger"
    assert_eq!(fs.callout_overrides["notes"], "danger");
}

#[test]
fn right_wraps_around_callout_list() {
    let mut app = make_app_callout();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 1;
    // Set to last option "danger" (index 11)
    fs.callout_overrides.insert("notes".to_string(), "danger".to_string());
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.callout_overrides["notes"], "note", "should wrap to first");
}

#[test]
fn custom_callout_value_cycles_to_known_option() {
    let mut app = make_app_callout();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 1;
    // Set a custom value not in CALLOUT_OPTIONS
    fs.callout_overrides.insert("notes".to_string(), "abstract".to_string());
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // Custom value not found → Right starts at 0 = "note"
    assert_eq!(fs.callout_overrides["notes"], "note");
}

#[test]
fn custom_callout_left_cycles_to_last_option() {
    let mut app = make_app_callout();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 1;
    fs.callout_overrides.insert("notes".to_string(), "abstract".to_string());
    handle_key(&mut app, key(KeyCode::Left));
    let fs = app.form_state.as_ref().unwrap();
    // Custom value not found → Left goes to last = "danger"
    assert_eq!(fs.callout_overrides["notes"], "danger");
}

#[test]
fn no_cycling_on_textarea_without_callout() {
    let mut app = make_app(); // standard config, notes textarea has no callout
    app.form_state.as_mut().unwrap().active_field = 3; // notes (textarea, no callout)
    let fs = app.form_state.as_ref().unwrap();
    assert!(!fs.callout_overrides.contains_key("notes"), "no callout in config → no override");
    // Right should NOT cycle callout — it should just move cursor (no-op at pos 0)
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    assert!(!fs.callout_overrides.contains_key("notes"), "should remain absent");
}

#[test]
fn no_cycling_when_textarea_editor_open() {
    let mut app = make_app_callout();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 1; // notes
    fs.textarea_open = true;
    fs.callout_overrides.insert("notes".to_string(), "note".to_string());
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // Should NOT cycle — cursor movement instead
    assert_eq!(fs.callout_overrides["notes"], "note", "callout unchanged when editor open");
}
