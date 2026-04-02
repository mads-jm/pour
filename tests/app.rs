use pour::app::{App, Screen};
use pour::config::Config;
use pour::data::history::History;
use pour::transport::Transport;
use pour::transport::fs::FsWriter;

/// A sample TOML config with two modules for testing App state.
const SAMPLE_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/%Y/%Y-%m-%d-%H%M%S.md"
display_name = "Coffee"

[[modules.coffee.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
required = true
options = ["V60", "AeroPress", "Espresso"]

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating (1-5)"
default = "3"

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Tasting notes"

[modules.me]
mode = "append"
path = "Journal/%Y/%Y-%m-%d.md"
append_under_header = "## Log"
append_template = "> {{body}}"
display_name = "Journal"

[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "What's on your mind?"
required = true
"####;

fn make_app() -> App {
    let config = Config::from_toml(SAMPLE_TOML).expect("sample config should parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/pour-test-history.json")),
    )
}

#[test]
fn new_sets_dashboard_screen_and_zero_selection() {
    let app = make_app();
    assert_eq!(app.screen, Screen::Dashboard);
    assert_eq!(app.selected_module, 0);
    assert!(app.form_state.is_none());
    assert!(app.summary_state.is_none());
}

#[test]
fn module_keys_sorted_alphabetically_without_module_order() {
    let app = make_app();
    // No module_order in sample config, so alphabetical fallback
    assert_eq!(app.module_keys, vec!["coffee", "me"]);
}

#[test]
fn module_keys_respect_module_order() {
    let toml_with_order = r####"
module_order = ["me", "coffee"]

[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/%Y/%Y-%m-%d-%H%M%S.md"
display_name = "Coffee"

[[modules.coffee.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
required = true
options = ["V60", "AeroPress", "Espresso"]

[modules.me]
mode = "append"
path = "Journal/%Y/%Y-%m-%d.md"
append_under_header = "## Log"
display_name = "Journal"

[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "What's on your mind?"
required = true
"####;
    let config = Config::from_toml(toml_with_order).expect("config with module_order should parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/pour-test-history.json")),
    );
    assert_eq!(app.module_keys, vec!["me", "coffee"]);
}

#[test]
fn init_form_sets_default_values() {
    let app = make_app();
    let form = app.init_form("coffee").expect("coffee module exists");

    // rating field has default = "3"
    assert_eq!(form.field_values.get("rating").unwrap(), "3");
    // notes has no default, should be empty string
    assert_eq!(form.field_values.get("notes").unwrap(), "");
    // active_field starts at 0
    assert_eq!(form.active_field, 0);
}

#[test]
fn init_form_populates_static_select_options() {
    let app = make_app();
    let form = app.init_form("coffee").expect("coffee module exists");

    let brew_options = form
        .field_options
        .get("brew_method")
        .expect("options present");
    assert_eq!(brew_options, &vec!["V60", "AeroPress", "Espresso"]);
}

#[test]
fn init_form_returns_none_for_unknown_module() {
    let app = make_app();
    assert!(app.init_form("nonexistent").is_none());
}

#[test]
fn validate_catches_empty_required_field() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let mut form = app.init_form("coffee").unwrap();

    // brew_method is required; leave it empty
    form.field_values
        .insert("brew_method".to_string(), "".to_string());

    let errors = App::validate_form(module, &form);
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|e| e.contains("Brew method")));
}

#[test]
fn validate_catches_non_numeric_number_field() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let mut form = app.init_form("coffee").unwrap();

    // Set brew_method so we don't get a required error for it
    form.field_values
        .insert("brew_method".to_string(), "V60".to_string());
    // Set rating to non-numeric
    form.field_values
        .insert("rating".to_string(), "abc".to_string());

    let errors = App::validate_form(module, &form);
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|e| e.contains("number")));
}

#[test]
fn validate_returns_empty_when_all_valid() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let mut form = app.init_form("coffee").unwrap();

    form.field_values
        .insert("brew_method".to_string(), "V60".to_string());
    form.field_values
        .insert("rating".to_string(), "4.5".to_string());
    form.field_values
        .insert("notes".to_string(), "fruity".to_string());

    let errors = App::validate_form(module, &form);
    assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
}

#[test]
fn validate_skips_empty_optional_number_field() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let mut form = app.init_form("coffee").unwrap();

    form.field_values
        .insert("brew_method".to_string(), "V60".to_string());
    // rating is optional, leave it empty — should NOT trigger number parse error
    form.field_values
        .insert("rating".to_string(), "".to_string());

    let errors = App::validate_form(module, &form);
    assert!(
        errors.is_empty(),
        "expected no errors for empty optional number, got: {errors:?}"
    );
}

// --- composite_array state & validation tests ---

const COMPOSITE_APP_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"

[[modules.coffee.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Brew stages"
required = true

[[modules.coffee.fields.sub_fields]]
name = "pour"
field_type = "number"
prompt = "Pour (g)"

[[modules.coffee.fields.sub_fields]]
name = "time"
field_type = "number"
prompt = "Time (s)"

[[modules.coffee.fields.sub_fields]]
name = "technique"
field_type = "static_select"
prompt = "Technique"
options = ["Bloom", "Spiral", "Center", "Pulse"]
"####;

fn make_composite_app() -> App {
    let config = Config::from_toml(COMPOSITE_APP_TOML).expect("composite config should parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from("/tmp/pour-test-history.json")),
    )
}

#[test]
fn init_form_sets_up_composite_values() {
    let app = make_composite_app();
    let form = app.init_form("coffee").expect("coffee module exists");

    // bean should be in field_values
    assert!(form.field_values.contains_key("bean"));
    // recipe should NOT be in field_values
    assert!(!form.field_values.contains_key("recipe"));
    // recipe should be in composite_values as empty vec
    let rows = form
        .composite_values
        .get("recipe")
        .expect("recipe in composite_values");
    assert!(rows.is_empty());
    // overlay state should be closed
    assert!(!form.composite_open);
    assert_eq!(form.composite_row, 0);
    assert_eq!(form.composite_col, 0);
}

#[test]
fn validate_composite_required_with_no_rows_fails() {
    let app = make_composite_app();
    let module = &app.config.modules["coffee"];
    let form = app.init_form("coffee").unwrap();

    let errors = App::validate_form(module, &form);
    assert!(errors.iter().any(|e| e.contains("at least one row")));
}

#[test]
fn validate_composite_strips_empty_rows() {
    let app = make_composite_app();
    let module = &app.config.modules["coffee"];
    let mut form = app.init_form("coffee").unwrap();

    // Add one empty row and one populated row
    form.composite_values.insert(
        "recipe".to_string(),
        vec![
            vec!["".to_string(), "".to_string(), "".to_string()], // empty — stripped
            vec!["50".to_string(), "30".to_string(), "Bloom".to_string()], // valid
        ],
    );

    let errors = App::validate_form(module, &form);
    assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
}

#[test]
fn validate_composite_catches_bad_number() {
    let app = make_composite_app();
    let module = &app.config.modules["coffee"];
    let mut form = app.init_form("coffee").unwrap();

    form.composite_values.insert(
        "recipe".to_string(),
        vec![vec![
            "abc".to_string(),
            "30".to_string(),
            "Bloom".to_string(),
        ]],
    );

    let errors = App::validate_form(module, &form);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("Pour (g)") && e.contains("number"))
    );
}

#[test]
fn validate_composite_passes_with_valid_rows() {
    let app = make_composite_app();
    let module = &app.config.modules["coffee"];
    let mut form = app.init_form("coffee").unwrap();

    form.composite_values.insert(
        "recipe".to_string(),
        vec![
            vec!["50".to_string(), "30".to_string(), "Bloom".to_string()],
            vec!["100".to_string(), "45".to_string(), "Spiral".to_string()],
        ],
    );

    let errors = App::validate_form(module, &form);
    assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
}
