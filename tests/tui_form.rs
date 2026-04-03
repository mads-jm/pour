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
