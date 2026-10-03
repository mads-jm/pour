use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pour::app::App;
use pour::config::Config;
use pour::data::field_presets::FieldPresets;
use pour::data::history::History;
use pour::data::presets::Presets;
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
        Presets::empty(),
        FieldPresets::empty(),
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
    // active_field=1 is "title" (the first real field; 0 is the preset row)
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 1);
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 2);
}

#[test]
fn shift_tab_goes_to_previous_field() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 2; // count
    handle_key(&mut app, key(KeyCode::BackTab));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 1); // title
}

#[test]
fn tab_wraps_around() {
    let mut app = make_app();
    let field_count = app.config.modules["test"].fields.len();
    // Set to submit button: preset(0) + fields(1..=field_count) + submit(field_count+1)
    app.form_state.as_mut().unwrap().active_field = field_count + 1;
    handle_key(&mut app, key(KeyCode::Tab));
    // Wraps to preset row (0)
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 0);
}

#[test]
fn shift_tab_wraps_around() {
    let mut app = make_app();
    let field_count = app.config.modules["test"].fields.len();
    // BackTab from preset row (0) wraps to submit (field_count+1)
    app.form_state.as_mut().unwrap().active_field = 0;
    handle_key(&mut app, key(KeyCode::BackTab));
    // Should wrap to submit button
    assert_eq!(
        app.form_state.as_ref().unwrap().active_field,
        field_count + 1
    );
}

#[test]
fn down_arrow_navigates_forward() {
    let mut app = make_app();
    // Starting at active_field=1 (title). Down goes to 2 (count).
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 2);
}

#[test]
fn up_arrow_navigates_backward() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 2; // count
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 1); // title
}

#[test]
fn enter_on_text_field_advances() {
    let mut app = make_app();
    // Field 1 is "title" (text); Enter advances to next field (2 = count)
    assert_eq!(handle_key(&mut app, key(KeyCode::Enter)), FormAction::None);
    assert_eq!(app.form_state.as_ref().unwrap().active_field, 2);
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
    app.form_state.as_mut().unwrap().active_field = 2; // count (number) is at visual index 2
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
    app.form_state.as_mut().unwrap().active_field = 2; // count
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
    app.form_state.as_mut().unwrap().active_field = 2; // count
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
    app.form_state.as_mut().unwrap().active_field = 3; // origin (static_select) is at visual index 3
    assert!(!app.form_state.as_ref().unwrap().dropdown_open);
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.form_state.as_ref().unwrap().dropdown_open);
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(!app.form_state.as_ref().unwrap().dropdown_open);
}

#[test]
fn down_cycles_options_when_dropdown_open() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 3; // origin
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
    fs.active_field = 3; // origin is at visual index 3
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
    app.form_state.as_mut().unwrap().active_field = 3; // origin
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
    app.form_state.as_mut().unwrap().active_field = 4; // notes (textarea) at visual index 4
    assert!(!app.form_state.as_ref().unwrap().textarea_open);
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.form_state.as_ref().unwrap().textarea_open);
}

#[test]
fn enter_inserts_newline_when_editor_open() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 4; // notes
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
    fs.active_field = 4; // notes
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
    app.form_state.as_mut().unwrap().active_field = 4; // notes
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
    app.form_state.as_mut().unwrap().active_field = 5; // recipe (composite_array) at visual index 5
    assert!(!app.form_state.as_ref().unwrap().composite_open);
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.form_state.as_ref().unwrap().composite_open);
}

#[test]
fn enter_adds_row_in_composite_overlay() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 5; // recipe
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
    fs.active_field = 5; // recipe
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
    fs.active_field = 5; // recipe
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
    fs.active_field = 5; // recipe
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
    fs.active_field = 5; // recipe
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
    fs.active_field = 3; // origin (static_select)
    fs.dropdown_open = true;
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(action, FormAction::None);
    assert!(!app.form_state.as_ref().unwrap().dropdown_open);
}

#[test]
fn esc_closes_textarea_first() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 4; // notes (textarea)
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
    // Submit is at field_count+1 (preset row at 0, fields at 1..=field_count, submit at field_count+1)
    app.form_state.as_mut().unwrap().active_field = field_count + 1;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert_eq!(action, FormAction::Submit);
}

// ── Tab closes overlays ──

#[test]
fn tab_closes_dropdown_and_advances() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 3; // origin (static_select) at visual index 3
    fs.dropdown_open = true;
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert!(!fs.dropdown_open);
    assert_eq!(fs.active_field, 4); // advances to notes
}

#[test]
fn tab_closes_textarea_and_advances() {
    let mut app = make_app();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 4; // notes (textarea) at visual index 4
    fs.textarea_open = true;
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert!(!fs.textarea_open);
    assert_eq!(fs.active_field, 5); // advances to recipe
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
        Presets::empty(),
        FieldPresets::empty(),
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
    app.form_state.as_mut().unwrap().active_field = 3; // origin (static_select) at visual index 3
    handle_key(&mut app, key(KeyCode::Char('e')));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.field_values.get("origin").unwrap(), "");
    assert!(!fs.search_buffers.contains_key("origin"));
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
        Presets::empty(),
        FieldPresets::empty(),
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
    assert!(!fs.search_buffers.contains_key("bean"));
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
    assert!(!fs.search_buffers.contains_key("bean"));
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
        Presets::empty(),
        FieldPresets::empty(),
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
    // visible = [method(0), notes(3)] → visible_count=2.
    // Layout: preset(0), method(1), notes(2), submit(3).
    // From notes (active_field=2), Tab should go to submit (active_field=3).
    app.form_state.as_mut().unwrap().active_field = 2; // notes
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(
        fs.active_field, 3,
        "should land on submit, not total_fields(4)"
    );
    assert_eq!(fs.active_config_idx, None, "submit has no config idx");
}

/// Tab from method (vi=1) should skip hidden grind/pressure and land on notes (vi=2).
#[test]
fn tab_skips_hidden_conditional_field() {
    let mut app = make_app_conditional();
    // init_form starts at active_field=1 (method, first real field).
    // Tab should advance to notes (active_field=2).
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.active_field, 2, "should land on notes (active_field=2)");
    assert_eq!(fs.active_config_idx, Some(3), "notes is config field 3");
}

/// Tab wraps through visible count + submit correctly.
#[test]
fn tab_wraps_using_visible_count() {
    let mut app = make_app_conditional();
    // visible = [method(0), notes(3)], visible_count=2, submit at active_field=3.
    // active_field=2 is notes. Tab should go to submit (active_field=3).
    app.form_state.as_mut().unwrap().active_field = 2; // notes
    handle_key(&mut app, key(KeyCode::Tab));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.active_field, 3, "should land on submit (active_field=3)");
    assert_eq!(
        fs.active_config_idx, None,
        "submit button has no config idx"
    );
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
    // Navigate to grind (active_field=2, config index 1).
    app.form_state.as_mut().unwrap().active_field = 2; // grind at visual index 2
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
    // grind is now hidden. With method=Espresso, visible = [method, pressure, notes].
    // active_field was 2, which now points to pressure (ci=2).
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
    // Navigate to pressure (active_field=2, visible index 1, config index 2).
    app.form_state.as_mut().unwrap().active_field = 2; // pressure
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("method".to_string(), "AeroPress".to_string());
    // Fire a key to trigger clamp.
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // pressure is hidden, next after ci=2 is notes (ci=3), so we land on notes.
    assert_eq!(
        fs.active_config_idx,
        Some(3),
        "should land on notes (next after hidden pressure)"
    );
}

/// Downstream fields appearing when a select changes do NOT steal focus.
#[test]
fn newly_visible_field_does_not_steal_focus() {
    let mut app = make_app_conditional();
    // Start on method (active_field=1, the first real field). Set method=V60 which makes grind appear.
    // Focus should stay on method.
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("method".to_string(), "V60".to_string());
    handle_key(&mut app, key(KeyCode::Right)); // trigger clamp, no navigation
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.active_field, 1, "focus stays on method");
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
        Presets::empty(),
        FieldPresets::empty(),
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
    assert_eq!(
        fs.callout_overrides.get("notes").map(|s| s.as_str()),
        Some("note")
    );
}

#[test]
fn right_cycles_callout_forward() {
    let mut app = make_app_callout();
    app.form_state.as_mut().unwrap().active_field = 2; // notes (textarea) at visual index 2
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // "note" is index 0 in CALLOUT_OPTIONS → Right goes to index 1 = "info"
    assert_eq!(fs.callout_overrides["notes"], "info");
}

#[test]
fn left_cycles_callout_backward() {
    let mut app = make_app_callout();
    app.form_state.as_mut().unwrap().active_field = 2; // notes (textarea) at visual index 2
    handle_key(&mut app, key(KeyCode::Left));
    let fs = app.form_state.as_ref().unwrap();
    // "note" is index 0 → Left wraps to last = "danger"
    assert_eq!(fs.callout_overrides["notes"], "danger");
}

#[test]
fn right_wraps_around_callout_list() {
    let mut app = make_app_callout();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 2; // notes at visual index 2
    // Set to last option "danger" (index 11)
    fs.callout_overrides
        .insert("notes".to_string(), "danger".to_string());
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(
        fs.callout_overrides["notes"], "note",
        "should wrap to first"
    );
}

#[test]
fn custom_callout_value_cycles_to_known_option() {
    let mut app = make_app_callout();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 2; // notes at visual index 2
    // Set a custom value not in CALLOUT_OPTIONS
    fs.callout_overrides
        .insert("notes".to_string(), "abstract".to_string());
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // Custom value not found → Right starts at 0 = "note"
    assert_eq!(fs.callout_overrides["notes"], "note");
}

#[test]
fn custom_callout_left_cycles_to_last_option() {
    let mut app = make_app_callout();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 2; // notes at visual index 2
    fs.callout_overrides
        .insert("notes".to_string(), "abstract".to_string());
    handle_key(&mut app, key(KeyCode::Left));
    let fs = app.form_state.as_ref().unwrap();
    // Custom value not found → Left goes to last = "danger"
    assert_eq!(fs.callout_overrides["notes"], "danger");
}

#[test]
fn no_cycling_on_textarea_without_callout() {
    let mut app = make_app(); // standard config, notes textarea has no callout
    app.form_state.as_mut().unwrap().active_field = 4; // notes (textarea, no callout) at visual index 4
    let fs = app.form_state.as_ref().unwrap();
    assert!(
        !fs.callout_overrides.contains_key("notes"),
        "no callout in config → no override"
    );
    // Right should NOT cycle callout — it should just move cursor (no-op at pos 0)
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    assert!(
        !fs.callout_overrides.contains_key("notes"),
        "should remain absent"
    );
}

#[test]
fn no_cycling_when_textarea_editor_open() {
    let mut app = make_app_callout();
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 2; // notes at visual index 2
    fs.textarea_open = true;
    fs.callout_overrides
        .insert("notes".to_string(), "note".to_string());
    handle_key(&mut app, key(KeyCode::Right));
    let fs = app.form_state.as_ref().unwrap();
    // Should NOT cycle — cursor movement instead
    assert_eq!(
        fs.callout_overrides["notes"], "note",
        "callout unchanged when editor open"
    );
}

// ── Multi-byte / Unicode cursor correctness ──────────────────────────────────
//
// These tests guard against the historical panic caused by treating
// `cursor_position` as a byte offset rather than a char-index.  Any attempt to
// call `String::remove` or `String::insert` on a non-char-boundary would panic;
// the tests below verify that emoji, CJK characters, and accented letters are
// all handled without panicking and that the cursor tracks chars, not bytes.

/// Insert an accented character ("é", 2 UTF-8 bytes) into a text field then
/// backspace it.  Cursor must return to 0 with an empty value.
#[test]
fn multibyte_insert_and_backspace_accent() {
    let mut app = make_app();
    {
        let fs = app.form_state.as_mut().unwrap();
        fs.active_field = 1; // "title" text field
        fs.cursor_position = 0;
    }
    // Insert 'é' (U+00E9, 2 bytes in UTF-8)
    handle_key(&mut app, key(KeyCode::Char('é')));
    {
        let fs = app.form_state.as_ref().unwrap();
        assert_eq!(fs.field_values["title"], "é");
        assert_eq!(fs.cursor_position, 1, "cursor should advance by 1 char");
    }
    // Backspace must remove the whole 2-byte sequence without panicking.
    handle_key(&mut app, key(KeyCode::Backspace));
    {
        let fs = app.form_state.as_ref().unwrap();
        assert_eq!(fs.field_values["title"], "");
        assert_eq!(fs.cursor_position, 0);
    }
}

/// Insert the "🎉" emoji (4 UTF-8 bytes) at position 0; cursor advances to 1;
/// backspace removes the whole emoji atomically.
#[test]
fn emoji_insert_and_backspace() {
    let mut app = make_app();
    {
        let fs = app.form_state.as_mut().unwrap();
        fs.active_field = 1; // "title" text field
        fs.cursor_position = 0;
    }
    // Insert the 4-byte emoji '🎉'
    handle_key(&mut app, key(KeyCode::Char('🎉')));
    {
        let fs = app.form_state.as_ref().unwrap();
        assert_eq!(fs.field_values["title"], "🎉");
        // char-index advances by exactly 1
        assert_eq!(fs.cursor_position, 1);
    }
    // Backspace must not panic and must leave an empty string.
    handle_key(&mut app, key(KeyCode::Backspace));
    {
        let fs = app.form_state.as_ref().unwrap();
        assert_eq!(fs.field_values["title"], "");
        assert_eq!(fs.cursor_position, 0);
    }
}

/// Insert two CJK characters ("中文") then press Left.  The cursor must move
/// back by one char (to position 1), not by one byte.
#[test]
fn cjk_arrow_left_moves_one_char() {
    let mut app = make_app();
    {
        let fs = app.form_state.as_mut().unwrap();
        fs.active_field = 1; // "title" text field
        fs.cursor_position = 0;
    }
    handle_key(&mut app, key(KeyCode::Char('中')));
    handle_key(&mut app, key(KeyCode::Char('文')));
    {
        let fs = app.form_state.as_ref().unwrap();
        assert_eq!(fs.cursor_position, 2, "two chars inserted → cursor at 2");
    }
    handle_key(&mut app, key(KeyCode::Left));
    {
        let fs = app.form_state.as_ref().unwrap();
        assert_eq!(
            fs.cursor_position, 1,
            "Left should move one char back, not one byte"
        );
    }
}

/// Multiline textarea: line 1 contains a multi-byte char, cursor is at the
/// start of line 2.  Up arrow must land at a valid char-index on line 1.
#[test]
fn textarea_vertical_move_over_multibyte_line() {
    let mut app = make_app();
    {
        let fs = app.form_state.as_mut().unwrap();
        fs.active_field = 4; // "notes" textarea
        fs.textarea_open = true;
        // "🎉\nhi" — line 0 is 1 char (4 bytes), line 1 is 2 chars
        fs.field_values
            .insert("notes".to_string(), "🎉\nhi".to_string());
        // Place cursor at start of "hi" (char-index 2: 1 char + 1 newline)
        fs.cursor_position = 2;
    }
    handle_key(&mut app, key(KeyCode::Up));
    {
        let fs = app.form_state.as_ref().unwrap();
        // Moving up from col 0 of line 1 should land at col 0 of line 0 → char-index 0.
        // The col is min(0, line0_char_len=1) = 0, so cursor = 0.
        assert_eq!(
            fs.cursor_position, 0,
            "Up from line 1 col 0 should reach char-index 0 on the emoji line"
        );
        // Verify no panic occurred by checking the value is still intact.
        assert_eq!(fs.field_values["notes"], "🎉\nhi");
    }
}

/// Mixed ASCII + emoji string: cursor at position 3 (after "hi🎉"), inserting
/// an ASCII char must place it correctly at the char boundary.
#[test]
fn insert_ascii_after_emoji_in_mixed_string() {
    let mut app = make_app();
    {
        let fs = app.form_state.as_mut().unwrap();
        fs.active_field = 1; // "title" text field
        // Pre-populate "hi🎉" (3 chars: 'h', 'i', '🎉' — 6 bytes total)
        fs.field_values
            .insert("title".to_string(), "hi🎉".to_string());
        // Place cursor after the emoji (char-index 3)
        fs.cursor_position = 3;
    }
    handle_key(&mut app, key(KeyCode::Char('!')));
    {
        let fs = app.form_state.as_ref().unwrap();
        // '!' should be appended after the emoji, not inserted mid-byte.
        assert_eq!(fs.field_values["title"], "hi🎉!");
        assert_eq!(fs.cursor_position, 4);
    }
}

// ── toggle / counter widgets (habit capture v1) ──

const HABIT_FORM_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.habit]
mode = "update"
path = "daily/%Y%m%d.md"

[[modules.habit.fields]]
name = "cannabis"
field_type = "toggle"
prompt = "Partaken?"

[[modules.habit.fields]]
name = "water"
field_type = "counter"
prompt = "Water"
unit = "oz"
goal = 96
"####;

fn habit_app() -> App {
    let config = Config::from_toml(HABIT_FORM_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/test-habit-history.json")),
        Presets::empty(),
        FieldPresets::empty(),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == "habit").unwrap();
    app.form_state = app.init_form("habit");
    app.screen = pour::app::Screen::Form;
    app
}

fn value_of(app: &App, field: &str) -> String {
    app.form_state.as_ref().unwrap().field_values[field].clone()
}

#[test]
fn space_flips_a_toggle_field() {
    let mut app = habit_app();
    // active_field 1 == the first real field (0 is the preset row).
    app.form_state.as_mut().unwrap().active_field = 1;

    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert_eq!(value_of(&app, "cannabis"), "true");

    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert_eq!(value_of(&app, "cannabis"), "false");
}

#[test]
fn typing_into_a_toggle_does_nothing() {
    let mut app = habit_app();
    app.form_state.as_mut().unwrap().active_field = 1;

    handle_key(&mut app, key(KeyCode::Char('x')));
    handle_key(&mut app, key(KeyCode::Char('9')));
    assert_eq!(
        value_of(&app, "cannabis"),
        "",
        "a toggle has no text buffer to accumulate into"
    );
}

#[test]
fn a_counter_accepts_digits_and_the_set_prefix_only() {
    let mut app = habit_app();
    app.form_state.as_mut().unwrap().active_field = 2;

    for c in ['=', '1', '6', 'x', ' ', '.', '5'] {
        handle_key(&mut app, key(KeyCode::Char(c)));
    }
    assert_eq!(value_of(&app, "water"), "=16.5");
}

#[test]
fn counter_validation_rejects_junk_but_accepts_blank() {
    let mut app = habit_app();
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("water".to_string(), "lots".to_string());
    assert!(
        App::validate_form(
            &app.config.modules["habit"],
            app.form_state.as_ref().unwrap()
        )
        .iter()
        .any(|e| e.contains("Water")),
        "a junk counter token must fail validation"
    );

    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert("water".to_string(), String::new());
    assert!(
        App::validate_form(
            &app.config.modules["habit"],
            app.form_state.as_ref().unwrap()
        )
        .is_empty(),
        "blank means no change, not invalid"
    );
}

#[test]
fn rendering_an_update_form_without_a_read_shows_a_placeholder() {
    // A failed or slow read leaves `current_values` empty; the form must still
    // render (ADR-003 — never block on a round-trip).
    let mut app = habit_app();
    app.form_state.as_mut().unwrap().active_field = 2;

    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).expect("terminal");
    terminal
        .draw(|frame| pour::tui::form::render(&app, frame))
        .expect("render must not panic");

    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(rendered.contains("now —/96 oz"), "got: {rendered}");
    assert!(rendered.contains("[ ]"), "toggle renders a checkbox");
}

#[test]
fn rendering_an_update_form_after_a_read_shows_current_progress() {
    let mut app = habit_app();
    {
        let fs = app.form_state.as_mut().unwrap();
        fs.current_values
            .insert("water".to_string(), "64".to_string());
        fs.field_values
            .insert("cannabis".to_string(), "true".to_string());
    }

    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).expect("terminal");
    terminal
        .draw(|frame| pour::tui::form::render(&app, frame))
        .expect("render must not panic");

    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(rendered.contains("now 64/96 oz"), "got: {rendered}");
    assert!(rendered.contains("[x]"), "got: {rendered}");
}

// ── fetch_current_values: seeding from the note (habit capture v1) ──

/// A habit app whose transport points at `dir`, with `note` seeded at today's
/// daily path (`fetch_current_values` renders the path against the real clock).
fn habit_app_over(dir: &tempfile::TempDir, note: &str) -> App {
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    std::fs::create_dir_all(dir.path().join("daily")).unwrap();
    std::fs::write(dir.path().join(format!("daily/{today}.md")), note).unwrap();

    let toml = HABIT_FORM_TOML.replace(
        "/tmp/vault",
        &dir.path().to_string_lossy().replace('\\', "\\\\"),
    );
    let config = Config::from_toml(&toml).expect("parse");
    let mut app = App::new(
        config,
        Transport::Fs(FsWriter::new(dir.path().to_path_buf())),
        History::load_from(std::path::PathBuf::from("/tmp/test-habit-history.json")),
        Presets::empty(),
        FieldPresets::empty(),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == "habit").unwrap();
    app.form_state = app.init_form("habit");
    app.screen = pour::app::Screen::Form;
    app
}

fn note_today(dir: &tempfile::TempDir) -> String {
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    std::fs::read_to_string(dir.path().join(format!("daily/{today}.md"))).unwrap()
}

#[tokio::test]
async fn a_readable_toggle_is_seeded_from_the_note() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = habit_app_over(&dir, "---\ncannabis: true\nwater: 40\n---\n\nbody\n");

    pour::tui::fetch_current_values(&mut app, "habit").await;

    assert_eq!(value_of(&app, "cannabis"), "true");
    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(fs.current_values["water"], "40");
}

#[tokio::test]
async fn a_toggle_pour_cannot_read_is_left_unseeded_and_survives_the_submit() {
    // A hand-edited or pre-boolean value must not be coerced: toggles are
    // always seeded and therefore always re-written, so a default of `false`
    // here would silently destroy the note's real value on the very next
    // submit — even one where the user only touched the counter.
    let note = "---\ncannabis: maybe\nwater: null\n---\n\nbody\n";
    let dir = tempfile::tempdir().unwrap();
    let mut app = habit_app_over(&dir, note);

    pour::tui::fetch_current_values(&mut app, "habit").await;

    assert_eq!(
        value_of(&app, "cannabis"),
        "",
        "an unreadable toggle stays blank rather than coercing to false"
    );

    // Submit exactly as the form would, having touched only the counter.
    let fs = app.form_state.as_mut().unwrap();
    fs.field_values
        .insert("water".to_string(), "16".to_string());
    let field_values = fs.field_values.clone();
    pour::output::write_update(
        &app.transport,
        &app.config.modules["habit"],
        &field_values,
        app.config.vault.date_format.as_deref(),
        &dir.path().to_string_lossy(),
        chrono::Local::now(),
    )
    .await
    .expect("the counter still captures");

    assert_eq!(
        note_today(&dir),
        note.replace("water: null", "water: 16"),
        "the toggle the user never touched must be byte-identical"
    );
}

#[test]
fn an_unreadable_toggle_renders_as_unknown_not_unchecked() {
    let mut app = habit_app();
    {
        let fs = app.form_state.as_mut().unwrap();
        // What `fetch_current_values` leaves behind for `cannabis: maybe`:
        // present in the note, absent from the form's values.
        fs.current_values
            .insert("cannabis".to_string(), "maybe".to_string());
        fs.field_values
            .insert("cannabis".to_string(), String::new());
    }

    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).expect("terminal");
    terminal
        .draw(|frame| pour::tui::form::render(&app, frame))
        .expect("render must not panic");

    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        rendered.contains("[?]"),
        "an unknown state must not be drawn as unchecked, got: {rendered}"
    );
}

// ── footer hints name the key the active field actually responds to ──
//
// The generic "Enter interact" is wrong on a toggle: Enter advances, and space
// is the only key that flips it. A counter's `=` prefix ("set, don't add") is
// invisible until someone tells you. Both shipped in habit capture v1 with no
// hint, which is how `pour habit` turned into "I cannot log cannabis."

fn footer_of(app: &App) -> String {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).expect("terminal");
    terminal
        .draw(|frame| pour::tui::form::render(app, frame))
        .expect("render must not panic");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn a_focused_toggle_hints_space_not_enter() {
    let mut app = habit_app();
    app.form_state.as_mut().unwrap().active_field = 1; // cannabis

    let footer = footer_of(&app);
    assert!(
        footer.contains("space flip"),
        "a toggle must name the key that flips it, got: {footer}"
    );
    assert!(
        !footer.contains("Enter interact"),
        "Enter advances on a toggle; claiming it interacts sends the user nowhere"
    );
}

#[test]
fn a_focused_counter_hints_the_set_prefix() {
    let mut app = habit_app();
    app.form_state.as_mut().unwrap().active_field = 2; // water

    let footer = footer_of(&app);
    assert!(
        footer.contains("0-9 add"),
        "a bare number accumulates, got: {footer}"
    );
    assert!(
        footer.contains("= set"),
        "the `=` prefix sets instead of adding and is otherwise undiscoverable, got: {footer}"
    );
}

#[test]
fn other_field_types_keep_the_generic_interact_hint() {
    let mut app = make_app();
    app.form_state.as_mut().unwrap().active_field = 1; // title, a text field

    let footer = footer_of(&app);
    assert!(
        footer.contains("Enter interact"),
        "only toggle and counter override the generic hint, got: {footer}"
    );
}

#[test]
fn the_submit_button_keeps_the_generic_interact_hint() {
    // active_field_cfg is None on the submit row; the match must fall through
    // rather than dropping the hint entirely.
    let mut app = habit_app();
    app.form_state.as_mut().unwrap().active_field = 3; // submit button

    let footer = footer_of(&app);
    assert!(
        footer.contains("Enter interact"),
        "the submit button is still an Enter target, got: {footer}"
    );
}

// ── Text entry: textarea popout and single-line fields ──
//
// Each test renders the form into a `TestBackend` and reads cells and the
// cursor straight off the backend. The fields area at 80x24 is rows 3..=20
// (title 0-2, footer 21-23), so a field at list row `r` sits on frame row
// `3 + r`.

use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use unicode_width::UnicodeWidthStr;

const ENTRY_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.entry]
mode = "create"
path = "entry.md"

[[modules.entry.fields]]
name = "title"
field_type = "text"
prompt = "Title"

[[modules.entry.fields]]
name = "aside"
field_type = "textarea"
prompt = "Aside"
callout = "note"

[[modules.entry.fields]]
name = "body"
field_type = "textarea"
prompt = "Body"

[[modules.entry.fields]]
name = "mood"
field_type = "text"
prompt = "Mood"

[[modules.entry.fields]]
name = "place"
field_type = "text"
prompt = "Place"
icon = "📍"

[[modules.entry.fields]]
name = "cups"
field_type = "number"
prompt = "Cups"
"####;

// active_field slots in ENTRY_TOML (preset row is 0).
const TITLE: usize = 1;
const ASIDE: usize = 2;
const BODY: usize = 3;
const MOOD: usize = 4;
const PLACE: usize = 5;
const CUPS: usize = 6;

fn app_from(toml: &str, module: &str) -> App {
    let config = Config::from_toml(toml).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/test-entry-history.json")),
        Presets::empty(),
        FieldPresets::empty(),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == module).unwrap();
    app.form_state = app.init_form(module);
    app.screen = pour::app::Screen::Form;
    app
}

fn entry_app() -> App {
    app_from(ENTRY_TOML, "entry")
}

fn set_value(app: &mut App, field: &str, value: &str) {
    app.form_state
        .as_mut()
        .unwrap()
        .field_values
        .insert(field.to_string(), value.to_string());
}

/// Focus `slot` with the cursor at the end of its value, as Tab would.
fn focus(app: &mut App, slot: usize, field: &str) {
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = slot;
    fs.cursor_position = fs.field_values.get(field).map_or(0, |v| v.chars().count());
}

/// Give the form a selected preset whose description adds a second row.
fn select_described_preset(app: &mut App) {
    let fs = app.form_state.as_mut().unwrap();
    fs.preset_names = vec!["morning".to_string()];
    fs.preset_descriptions = vec![Some("a described preset".to_string())];
    fs.selected_preset_name = Some("morning".to_string());
}

struct Shot {
    buf: Buffer,
    /// `None` when the render hid the cursor.
    cursor: Option<Position>,
}

/// Render the form at `w`x`h`. The cursor reads `None` when the render did
/// not place it: the backend's position is primed with a sentinel that a
/// hidden cursor leaves untouched.
fn shoot(app: &App, w: u16, h: u16) -> Shot {
    let sentinel = Position::new(u16::MAX, u16::MAX);
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).expect("terminal");
    terminal
        .backend_mut()
        .set_cursor_position(sentinel)
        .unwrap();
    terminal
        .draw(|frame| pour::tui::form::render(app, frame))
        .expect("render must not panic");
    let cursor = terminal.backend_mut().get_cursor_position().unwrap();
    Shot {
        buf: terminal.backend().buffer().clone(),
        cursor: (cursor != sentinel).then_some(cursor),
    }
}

/// Cells `[x0, x1)` of row `y` as text, skipping the blank cell a wide glyph
/// leaves behind it.
fn cells(buf: &Buffer, y: u16, x0: u16, x1: u16) -> String {
    let mut out = String::new();
    let mut x = x0;
    while x < x1 {
        let sym = buf[(x, y)].symbol();
        out.push_str(sym);
        x += (sym.width() as u16).max(1);
    }
    out
}

fn row(buf: &Buffer, y: u16) -> String {
    cells(buf, y, 0, buf.area.width)
}

fn sym(buf: &Buffer, x: u16, y: u16) -> &str {
    buf[(x, y)].symbol()
}

/// The textarea popout's outer rect, found from its border. Panics with the
/// rendered screen when the border is missing or broken.
fn popout(shot: &Shot) -> Rect {
    let buf = &shot.buf;
    let screen = || {
        (0..buf.area.height)
            .map(|y| row(buf, y))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (x0, y0) = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .find(|&(x, y)| sym(buf, x, y) == "┌")
        .unwrap_or_else(|| panic!("no popout on screen:\n{}", screen()));
    let x1 = (x0 + 1..buf.area.width)
        .find(|&x| sym(buf, x, y0) == "┐")
        .unwrap_or_else(|| panic!("popout top border has no ┐:\n{}", screen()));
    let mut y1 = y0 + 1;
    while y1 < buf.area.height && sym(buf, x0, y1) == "│" {
        assert_eq!(
            sym(buf, x1, y1),
            "│",
            "popout right border broken on row {y1}:\n{}",
            screen()
        );
        y1 += 1;
    }
    assert!(
        y1 < buf.area.height && sym(buf, x0, y1) == "└" && sym(buf, x1, y1) == "┘",
        "popout bottom border broken at row {y1}:\n{}",
        screen()
    );
    for x in x0 + 1..x1 {
        // `▼` marks lines hidden below the popout.
        assert!(
            matches!(sym(buf, x, y1), "─" | "▼"),
            "popout bottom border overdrawn at column {x}:\n{}",
            screen()
        );
    }
    Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// Every row of the screen with the popout's cells masked out.
fn outside_popout(buf: &Buffer, p: Rect) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            if y < p.y || y >= p.bottom() {
                row(buf, y)
            } else {
                format!(
                    "{}#{}",
                    cells(buf, y, 0, p.x),
                    cells(buf, y, p.right(), buf.area.width)
                )
            }
        })
        .collect()
}

/// Content row `line` of the popout (inside its border).
fn popout_row(buf: &Buffer, p: Rect, line: u16) -> String {
    cells(buf, p.y + 1 + line, p.x + 1, p.right() - 1)
}

/// Assert the cursor sits inside the popout's content area.
fn cursor_in_popout(shot: &Shot, p: Rect, what: &str) -> Position {
    let c = shot
        .cursor
        .unwrap_or_else(|| panic!("{what}: cursor hidden"));
    assert!(
        c.x > p.x && c.x < p.right() - 1 && c.y > p.y && c.y < p.bottom() - 1,
        "{what}: cursor {c:?} outside popout content {p:?}"
    );
    c
}

const LONG_VALUE: &str =
    "ZEBRA one two three four five six seven eight nine ten eleven twelve QUOKKA\nsecond line YAK";

/// Words of `LONG_VALUE` that appear nowhere else on the form.
const VALUE_WORDS: [&str; 10] = [
    "ZEBRA", "three", "seven", "eight", "eleven", "twelve", "QUOKKA", "second", "YAK", "lines]",
];

fn assert_value_only_inside_popout(app: &App, what: &str) {
    let shot = shoot(app, 80, 24);
    let p = popout(&shot);
    for (y, text) in outside_popout(&shot.buf, p).iter().enumerate() {
        for word in VALUE_WORDS {
            assert!(
                !text.contains(word),
                "{what}: `{word}` drawn outside the popout on row {y}: {text:?}"
            );
        }
    }
}

#[test]
fn open_popout_hides_the_textarea_value_from_its_row() {
    // make_app: title, count, origin, then the `notes` textarea with no
    // callout and nothing taller than one row above it.
    let mut app = make_app();
    set_value(&mut app, "notes", LONG_VALUE);
    focus(&mut app, 4, "notes");
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(app.form_state.as_ref().unwrap().textarea_open);
    assert_value_only_inside_popout(&app, "textarea without callout");
}

#[test]
fn open_popout_hides_the_callout_textarea_value_from_its_rows() {
    for described in [false, true] {
        // make_app_callout: title, then the `notes` textarea with a callout.
        let mut app = make_app_callout();
        if described {
            select_described_preset(&mut app);
        }
        set_value(&mut app, "notes", LONG_VALUE);
        focus(&mut app, 2, "notes");
        handle_key(&mut app, key(KeyCode::Enter));
        assert_value_only_inside_popout(
            &app,
            &format!("callout textarea, preset description: {described}"),
        );
    }
}

#[test]
fn closing_the_popout_brings_the_row_preview_back() {
    let mut app = entry_app();
    set_value(&mut app, "body", "first line\nsecond");
    focus(&mut app, BODY, "body");
    handle_key(&mut app, key(KeyCode::Enter));
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.form_state.as_ref().unwrap().textarea_open);

    let shot = shoot(&app, 80, 24);
    // preset 0, title 1, aside 2-3, body 4.
    assert!(
        row(&shot.buf, 3 + 4).contains("Body : first line [2 lines] [v]"),
        "got: {:?}",
        row(&shot.buf, 3 + 4)
    );
}

/// `lead` text fields followed by a `tail` textarea, so the textarea sits on
/// list row `lead + 1`.
fn bottom_textarea_app(lead: usize) -> App {
    let mut toml = String::from(
        "[vault]\nbase_path = \"/tmp/vault\"\n\n[modules.long]\nmode = \"create\"\npath = \"long.md\"\n",
    );
    for i in 0..lead {
        toml.push_str(&format!(
            "\n[[modules.long.fields]]\nname = \"f{i}\"\nfield_type = \"text\"\nprompt = \"Field {i}\"\n"
        ));
    }
    toml.push_str(
        "\n[[modules.long.fields]]\nname = \"tail\"\nfield_type = \"textarea\"\nprompt = \"Tail\"\n",
    );
    let mut app = app_from(&toml, "long");
    set_value(&mut app, "tail", "one\ntwo\nthree");
    focus(&mut app, lead + 1, "tail");
    handle_key(&mut app, key(KeyCode::Enter));
    app
}

#[test]
fn footer_does_not_draw_inside_a_popout_at_the_bottom_of_the_form() {
    // The fields area is 18 rows (0..=17); the textarea lands on rows 15, 16
    // and 17, its last three.
    for lead in [14, 15, 16] {
        let app = bottom_textarea_app(lead);
        let shot = shoot(&app, 80, 24);
        let p = popout(&shot); // panics if the footer broke the border
        for y in p.y + 1..p.bottom() - 1 {
            let inside = cells(&shot.buf, y, p.x + 1, p.right() - 1);
            for hint in ["save", "navigate", "clear/back", "───"] {
                assert!(
                    !inside.contains(hint),
                    "textarea on row {}: footer `{hint}` inside the popout on row {y}: {inside:?}",
                    lead + 1
                );
            }
        }
        assert!(p.bottom() <= 24, "popout runs off the frame: {p:?}");
    }
}

#[test]
fn rows_under_the_popout_show_nothing_beside_it() {
    // A textarea first, so nothing above it is taller than one row, and long
    // values on the rows the popout covers.
    let toml = r#"
[vault]
base_path = "/tmp/vault"

[modules.side]
mode = "create"
path = "side.md"

[[modules.side.fields]]
name = "body"
field_type = "textarea"
prompt = "Body"

[[modules.side.fields]]
name = "mood"
field_type = "text"
prompt = "Mood"

[[modules.side.fields]]
name = "place"
field_type = "text"
prompt = "Place"
"#;
    let mut app = app_from(toml, "side");
    set_value(&mut app, "mood", &"m".repeat(75));
    set_value(&mut app, "place", &"p".repeat(75));
    set_value(&mut app, "body", "short");
    focus(&mut app, 1, "body");
    handle_key(&mut app, key(KeyCode::Enter));

    let shot = shoot(&app, 80, 24);
    let p = popout(&shot);
    for y in p.y..p.bottom() {
        let left = cells(&shot.buf, y, 0, p.x);
        let right = cells(&shot.buf, y, p.right(), 80);
        assert!(
            left.trim().is_empty() && right.trim().is_empty(),
            "row {y} shows form text beside the popout: left {left:?}, right {right:?}"
        );
    }
}

#[test]
fn popout_opens_directly_below_the_active_textarea() {
    // (preset description?, expected list row of the body textarea)
    // preset 0[-1], title, aside (2 rows), body.
    for (described, body_row) in [(false, 4u16), (true, 5u16)] {
        let mut app = entry_app();
        if described {
            select_described_preset(&mut app);
        }
        set_value(&mut app, "body", "text");
        focus(&mut app, BODY, "body");
        handle_key(&mut app, key(KeyCode::Enter));

        let shot = shoot(&app, 80, 24);
        let p = popout(&shot);
        assert_eq!(
            p.y,
            3 + body_row + 1,
            "described {described}: popout top border should sit right under the body row"
        );
        assert!(
            row(&shot.buf, 3 + body_row).contains("Body :"),
            "described {described}: body not on row {body_row}: {:?}",
            row(&shot.buf, 3 + body_row)
        );
    }

    // The preset description alone moves a textarea with nothing taller above.
    let mut app = entry_app();
    select_described_preset(&mut app);
    set_value(&mut app, "aside", "text");
    focus(&mut app, ASIDE, "aside");
    handle_key(&mut app, key(KeyCode::Enter));
    let shot = shoot(&app, 80, 24);
    let p = popout(&shot);
    // preset 0-1, title 2, aside header 3.
    assert_eq!(
        p.y,
        3 + 4,
        "popout top border should sit right under the aside header"
    );
    assert!(row(&shot.buf, 3 + 3).contains("Aside :"));
}

#[test]
fn single_line_cursor_lands_on_the_active_row_below_tall_rows() {
    for (described, mood_row) in [(false, 5u16), (true, 6u16)] {
        let mut app = entry_app();
        if described {
            select_described_preset(&mut app);
        }
        set_value(&mut app, "mood", "calm");
        focus(&mut app, MOOD, "mood");

        let shot = shoot(&app, 80, 24);
        assert!(row(&shot.buf, 3 + mood_row).contains("Mood : calm"));
        assert_eq!(
            shot.cursor,
            Some(Position::new(
                ("▸ Mood : ".width() + "calm".width()) as u16,
                3 + mood_row
            )),
            "described {described}"
        );
    }
}

#[test]
fn popout_cursor_tracks_multibyte_and_wide_characters() {
    let value = "café\n日本 🫘 x\nlast";
    // (cursor char index, line, cell offset within the line, glyph left of it)
    let cases: [(usize, u16, u16, Option<&str>); 9] = [
        (4, 0, 4, Some("é")),
        (5, 1, 0, None),
        (6, 1, 2, Some("日")),
        (7, 1, 4, Some("本")),
        (8, 1, 5, Some(" ")),
        (9, 1, 7, Some("🫘")),
        (11, 1, 9, Some("x")),
        (12, 2, 0, None),
        (14, 2, 2, Some("a")),
    ];
    for (idx, line, col, before) in cases {
        let mut app = entry_app();
        set_value(&mut app, "body", value);
        focus(&mut app, BODY, "body");
        handle_key(&mut app, key(KeyCode::Enter));
        app.form_state.as_mut().unwrap().cursor_position = idx;

        let shot = shoot(&app, 80, 24);
        let p = popout(&shot);
        let c = cursor_in_popout(&shot, p, &format!("cursor_position {idx}"));
        assert_eq!(
            c,
            Position::new(p.x + 1 + col, p.y + 1 + line),
            "cursor_position {idx}"
        );
        if let Some(glyph) = before {
            let w = glyph.width() as u16;
            assert_eq!(sym(&shot.buf, c.x - w, c.y), glyph, "cursor_position {idx}");
        }
    }
}

/// Assert the cursor's line is on screen in the popout and is `expected`.
fn assert_cursor_line_visible(app: &App, expected: &str, what: &str) -> Shot {
    let shot = shoot(app, 80, 24);
    let p = popout(&shot);
    let c = cursor_in_popout(&shot, p, what);
    let text = popout_row(&shot.buf, p, c.y - p.y - 1);
    assert!(
        text.trim_end() == expected.trim_end(),
        "{what}: cursor row shows {text:?}, expected {expected:?}"
    );
    shot
}

#[test]
fn popout_scrolls_to_keep_the_cursor_line_visible() {
    let mut app = entry_app();
    focus(&mut app, BODY, "body");
    handle_key(&mut app, key(KeyCode::Enter));

    // body sits on row 4, so the popout gets the 10-row maximum: 8 lines.
    let lines: Vec<String> = (1..=12).map(|n| format!("line {n}")).collect();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            handle_key(&mut app, key(KeyCode::Enter));
            assert_cursor_line_visible(&app, "", &format!("Enter before {line}"));
        }
        for (j, c) in line.char_indices() {
            handle_key(&mut app, key(KeyCode::Char(c)));
            let typed = &line[..j + c.len_utf8()];
            assert_cursor_line_visible(&app, typed, &format!("typing {line}"));
        }
    }

    // Twelve lines in eight rows: hidden above, nothing below.
    let shot = shoot(&app, 80, 24);
    let p = popout(&shot);
    let top = cells(&shot.buf, p.y, p.x, p.right());
    let bottom = cells(&shot.buf, p.bottom() - 1, p.x, p.right());
    assert!(top.contains('▲'), "lines hidden above: {top:?}");
    assert!(!bottom.contains('▼'), "nothing hidden below: {bottom:?}");

    handle_key(&mut app, key(KeyCode::Backspace));
    assert_cursor_line_visible(&app, "line 1", "Backspace");
    handle_key(&mut app, key(KeyCode::Char('2')));

    for n in (1..=11).rev() {
        handle_key(&mut app, key(KeyCode::Up));
        assert_cursor_line_visible(&app, &format!("line {n}"), "Up");
    }
    let shot = shoot(&app, 80, 24);
    let p = popout(&shot);
    let top = cells(&shot.buf, p.y, p.x, p.right());
    let bottom = cells(&shot.buf, p.bottom() - 1, p.x, p.right());
    assert!(!top.contains('▲'), "nothing hidden above: {top:?}");
    assert!(bottom.contains('▼'), "lines hidden below: {bottom:?}");

    for n in 2..=12 {
        handle_key(&mut app, key(KeyCode::Down));
        assert_cursor_line_visible(&app, &format!("line {n}"), "Down");
    }
}

#[test]
fn cursor_is_visible_when_the_popout_opens_on_a_wide_last_line() {
    let mut app = entry_app();
    set_value(&mut app, "body", &format!("short\n{}", "w".repeat(100)));
    focus(&mut app, BODY, "body");
    handle_key(&mut app, key(KeyCode::Enter));

    let shot = shoot(&app, 80, 24);
    let p = popout(&shot);
    let c = cursor_in_popout(&shot, p, "on open");
    assert_eq!(c.y, p.y + 2, "cursor on the second line");
    assert_eq!(sym(&shot.buf, c.x - 1, c.y), "w");

    handle_key(&mut app, key(KeyCode::Up));
    let shot = shoot(&app, 80, 24);
    let c = cursor_in_popout(&shot, p, "after Up");
    assert_eq!(c.y, p.y + 1, "cursor on the first line");

    handle_key(&mut app, key(KeyCode::Down));
    let shot = shoot(&app, 80, 24);
    let c = cursor_in_popout(&shot, p, "after Down");
    assert_eq!(c.y, p.y + 2, "cursor back on the second line");
}

#[test]
fn single_line_cursor_counts_display_width_and_the_icon() {
    // place: "▸ " + "📍 " + "Place" + " " + ": "
    let prefix = ("▸ ".width() + "📍 Place : ".width()) as u16;
    let value = "日本 é!";
    for (idx, cell) in [(0, 0u16), (1, 2), (2, 4), (3, 5), (4, 6), (5, 7)] {
        let mut app = entry_app();
        set_value(&mut app, "place", value);
        focus(&mut app, PLACE, "place");
        app.form_state.as_mut().unwrap().cursor_position = idx;
        let shot = shoot(&app, 80, 24);
        // preset 0, title 1, aside 2-3, body 4, mood 5, place 6.
        assert_eq!(
            shot.cursor,
            Some(Position::new(prefix + cell, 3 + 6)),
            "cursor_position {idx}"
        );
    }

    let mut app = entry_app();
    set_value(&mut app, "cups", "12");
    focus(&mut app, CUPS, "cups");
    let shot = shoot(&app, 80, 24);
    assert_eq!(
        shot.cursor,
        Some(Position::new(("▸ Cups : 12".width()) as u16, 3 + 7))
    );

    let mut app = habit_app();
    set_value(&mut app, "water", "=16");
    focus(&mut app, 2, "water");
    let shot = shoot(&app, 80, 24);
    assert_eq!(
        shot.cursor,
        Some(Position::new(("▸ Water : =16".width()) as u16, 3 + 2))
    );
}

#[test]
fn long_single_line_value_scrolls_within_its_row() {
    let long: String = (0..150)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let title_y = 3 + 1;
    let prefix = "▸ Title : ".width() as u16;

    let mut short_app = entry_app();
    set_value(&mut short_app, "title", "short");
    focus(&mut short_app, TITLE, "title");
    let baseline = shoot(&short_app, 80, 24);

    let mut app = entry_app();
    set_value(&mut app, "title", &long);
    focus(&mut app, TITLE, "title");

    // At the end: the head is hidden, the cursor follows the last char.
    let shot = shoot(&app, 80, 24);
    let c = shot.cursor.expect("cursor visible at the end");
    assert_eq!(c.y, title_y);
    assert!(c.x < 80);
    assert_eq!(sym(&shot.buf, c.x - 1, c.y), &long[149..]);
    assert_eq!(
        sym(&shot.buf, prefix, title_y),
        "◂",
        "{:?}",
        row(&shot.buf, title_y)
    );
    for y in (0..24).filter(|&y| y != title_y) {
        assert_eq!(row(&shot.buf, y), row(&baseline.buf, y), "row {y} moved");
    }

    // Arrow back to the start: the tail is hidden, the cursor stays on screen
    // with the characters on both sides of it.
    for i in (0..150).rev() {
        handle_key(&mut app, key(KeyCode::Left));
        let shot = shoot(&app, 80, 24);
        let c = shot
            .cursor
            .unwrap_or_else(|| panic!("cursor hidden at {i}"));
        assert_eq!(c.y, title_y);
        assert_eq!(
            sym(&shot.buf, c.x, c.y),
            &long[i..=i],
            "char under cursor at {i}"
        );
        if i > 0 {
            assert_eq!(
                sym(&shot.buf, c.x - 1, c.y),
                &long[i - 1..i],
                "char before cursor at {i}"
            );
        }
    }
    let shot = shoot(&app, 80, 24);
    assert_eq!(shot.cursor, Some(Position::new(prefix, title_y)));
    assert_eq!(
        sym(&shot.buf, 79, title_y),
        "▸",
        "{:?}",
        row(&shot.buf, title_y)
    );
    for y in (0..24).filter(|&y| y != title_y) {
        assert_eq!(row(&shot.buf, y), row(&baseline.buf, y), "row {y} moved");
    }
}

#[test]
fn wide_characters_scroll_by_the_cells_they_take() {
    // Popout: 58 cells of room hold 29 `日`; typing 40 must keep scrolling.
    let mut app = entry_app();
    focus(&mut app, BODY, "body");
    handle_key(&mut app, key(KeyCode::Enter));
    for i in 1..=40 {
        handle_key(&mut app, key(KeyCode::Char('日')));
        let shot = shoot(&app, 80, 24);
        let p = popout(&shot);
        let c = cursor_in_popout(&shot, p, &format!("popout, {i} wide chars"));
        assert_eq!(sym(&shot.buf, c.x - 2, c.y), "日", "popout, {i} wide chars");
    }

    // Single-line row: 70 cells of room after `▸ Title : `.
    let mut app = entry_app();
    focus(&mut app, TITLE, "title");
    for i in 1..=60 {
        handle_key(&mut app, key(KeyCode::Char('🫘')));
        let shot = shoot(&app, 80, 24);
        let c = shot
            .cursor
            .unwrap_or_else(|| panic!("title, {i} wide chars: cursor hidden"));
        assert_eq!(c.y, 3 + 1);
        assert!(c.x < 80, "title, {i} wide chars: cursor at {c:?}");
        assert_eq!(sym(&shot.buf, c.x - 2, c.y), "🫘", "title, {i} wide chars");
    }
}
