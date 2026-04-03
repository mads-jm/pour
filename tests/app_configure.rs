use pour::app::{App, ConfigureLevel, SettingKind};
use pour::config::Config;
use pour::data::history::History;
use pour::transport::Transport;
use pour::transport::fs::FsWriter;
use std::collections::HashMap;

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
name = "source_dir"
field_type = "dynamic_select"
prompt = "Source"
source = "Beans/"

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
        History::load_from(std::path::PathBuf::from("/tmp/test-cfg-history.json")),
    )
}

// ── init_configure ──

#[test]
fn init_configure_create_mode_settings() {
    let app = make_app();
    let state = app.init_configure("coffee").unwrap();
    let keys: Vec<&str> = state.settings.iter().map(|s| s.key.as_str()).collect();
    assert!(keys.contains(&"path"));
    assert!(keys.contains(&"display_name"));
    assert!(keys.contains(&"mode"));
    assert!(keys.contains(&"callout_type"));
    assert!(keys.contains(&"fields"));
    // create mode should NOT have append_under_header
    assert!(!keys.contains(&"append_under_header"));
}

#[test]
fn init_configure_append_mode_has_append_header() {
    let app = make_app();
    let state = app.init_configure("journal").unwrap();
    let keys: Vec<&str> = state.settings.iter().map(|s| s.key.as_str()).collect();
    assert!(keys.contains(&"append_under_header"));
}

#[test]
fn init_configure_unknown_module_returns_none() {
    let app = make_app();
    assert!(app.init_configure("nonexistent").is_none());
}

#[test]
fn init_configure_field_count_in_navlink() {
    let app = make_app();
    let state = app.init_configure("coffee").unwrap();
    let fields_setting = state.settings.iter().find(|s| s.key == "fields").unwrap();
    assert_eq!(fields_setting.value, "6 fields");
}

#[test]
fn init_configure_sets_level_and_module_key() {
    let app = make_app();
    let state = app.init_configure("coffee").unwrap();
    assert_eq!(state.level, ConfigureLevel::ModuleSettings);
    assert_eq!(state.module_key, "coffee");
}

#[test]
fn init_configure_populates_path_and_display_name() {
    let app = make_app();
    let state = app.init_configure("coffee").unwrap();
    let path = state.settings.iter().find(|s| s.key == "path").unwrap();
    assert_eq!(path.value, "Coffee/test.md");
    let dn = state
        .settings
        .iter()
        .find(|s| s.key == "display_name")
        .unwrap();
    assert_eq!(dn.value, "Coffee");
}

// ── init_vault_configure ──

#[test]
fn vault_configure_has_four_settings() {
    let app = make_app();
    let state = app.init_vault_configure();
    assert_eq!(state.settings.len(), 4);
    let keys: Vec<&str> = state.settings.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["base_path", "api_port", "api_key", "date_format"]
    );
}

#[test]
fn vault_configure_module_key_is_sentinel() {
    let app = make_app();
    let state = app.init_vault_configure();
    assert_eq!(state.module_key, "__vault__");
}

#[test]
fn vault_configure_level_is_vault_settings() {
    let app = make_app();
    let state = app.init_vault_configure();
    assert_eq!(state.level, ConfigureLevel::VaultSettings);
}

#[test]
fn vault_configure_populates_values() {
    let app = make_app();
    let state = app.init_vault_configure();
    let base = state
        .settings
        .iter()
        .find(|s| s.key == "base_path")
        .unwrap();
    assert_eq!(base.value, "/tmp/vault");
    let port = state.settings.iter().find(|s| s.key == "api_port").unwrap();
    assert_eq!(port.value, "27124");
    let fmt = state
        .settings
        .iter()
        .find(|s| s.key == "date_format")
        .unwrap();
    assert_eq!(fmt.value, "%Y-%m-%d");
}

// ── init_new_module_configure ──

#[test]
fn new_module_has_four_settings() {
    let app = make_app();
    let state = app.init_new_module_configure();
    assert_eq!(state.settings.len(), 4);
    let keys: Vec<&str> = state.settings.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, vec!["module_key", "display_name", "mode", "path"]);
}

#[test]
fn new_module_level_is_new_module() {
    let app = make_app();
    let state = app.init_new_module_configure();
    assert_eq!(state.level, ConfigureLevel::NewModule);
}

#[test]
fn new_module_default_mode_is_create() {
    let app = make_app();
    let state = app.init_new_module_configure();
    let mode = state.settings.iter().find(|s| s.key == "mode").unwrap();
    assert_eq!(mode.value, "create");
}

#[test]
fn new_module_key_is_identifier_kind() {
    let app = make_app();
    let state = app.init_new_module_configure();
    let key_setting = state
        .settings
        .iter()
        .find(|s| s.key == "module_key")
        .unwrap();
    assert!(matches!(key_setting.kind, SettingKind::Identifier));
}

// ── build_field_settings ──

#[test]
fn text_field_settings_no_extra_settings() {
    let app = make_app();
    let field = &app.config.modules["coffee"].fields[0]; // bean (text)
    let settings = App::build_field_settings(field);
    let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
    assert!(keys.contains(&"name"));
    assert!(keys.contains(&"prompt"));
    assert!(keys.contains(&"field_type"));
    assert!(keys.contains(&"required"));
    assert!(keys.contains(&"default"));
    assert!(keys.contains(&"target"));
    // No options, source, or sub_fields for text
    assert!(!keys.contains(&"options"));
    assert!(!keys.contains(&"source"));
    assert!(!keys.contains(&"sub_fields"));
}

#[test]
fn static_select_field_has_options() {
    let app = make_app();
    let field = &app.config.modules["coffee"].fields[2]; // origin (static_select)
    let settings = App::build_field_settings(field);
    let opts = settings.iter().find(|s| s.key == "options").unwrap();
    assert!(matches!(opts.kind, SettingKind::ListEditor));
    assert_eq!(opts.value, "Ethiopia\nColombia");
}

#[test]
fn dynamic_select_field_has_source() {
    let app = make_app();
    let field = &app.config.modules["coffee"].fields[3]; // source_dir (dynamic_select)
    let settings = App::build_field_settings(field);
    let src = settings.iter().find(|s| s.key == "source").unwrap();
    assert!(matches!(src.kind, SettingKind::Path));
    assert_eq!(src.value, "Beans/");
}

#[test]
fn composite_field_has_sub_fields_navlink() {
    let app = make_app();
    let field = &app.config.modules["coffee"].fields[5]; // recipe (composite_array)
    let settings = App::build_field_settings(field);
    let subs = settings.iter().find(|s| s.key == "sub_fields").unwrap();
    assert!(matches!(subs.kind, SettingKind::NavLink));
    assert_eq!(subs.value, "2 columns");
}

#[test]
fn textarea_field_has_callout() {
    let app = make_app();
    let field = &app.config.modules["coffee"].fields[4]; // notes (textarea)
    let settings = App::build_field_settings(field);
    let callout = settings.iter().find(|s| s.key == "callout").unwrap();
    assert!(matches!(callout.kind, SettingKind::QuickSelect(_)));
    assert_eq!(callout.value, "tip");
}

// ── validate_form edge cases ──

fn make_form_state(
    field_values: HashMap<String, String>,
    composite_values: HashMap<String, Vec<Vec<String>>>,
) -> pour::app::FormState {
    pour::app::FormState {
        field_values,
        field_options: HashMap::new(),
        active_field: 0,
        validation_errors: Vec::new(),
        cursor_position: 0,
        dropdown_open: false,
        textarea_open: false,
        textarea_scroll_offset: 0,
        composite_values,
        composite_open: false,
        composite_row: 0,
        composite_col: 0,
        search_buffers: HashMap::new(),
        sub_form: None,
    }
}

#[test]
fn optional_empty_number_passes() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    // rating is number, not required, empty value
    let mut values = HashMap::new();
    values.insert("bean".to_string(), "Ethiopian".to_string());
    values.insert("rating".to_string(), String::new());
    values.insert("origin".to_string(), String::new());
    values.insert("source_dir".to_string(), String::new());
    values.insert("notes".to_string(), String::new());
    let form = make_form_state(values, HashMap::new());
    let errors = App::validate_form(module, &form);
    assert!(errors.is_empty(), "got errors: {:?}", errors);
}

#[test]
fn required_empty_text_fails() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let values = HashMap::new(); // all empty
    let form = make_form_state(values, HashMap::new());
    let errors = App::validate_form(module, &form);
    assert!(errors.iter().any(|e| e.contains("Bean")));
}

#[test]
fn valid_number_passes() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let mut values = HashMap::new();
    values.insert("bean".to_string(), "Test".to_string());
    values.insert("rating".to_string(), "3.14".to_string());
    let form = make_form_state(values, HashMap::new());
    let errors = App::validate_form(module, &form);
    assert!(!errors.iter().any(|e| e.contains("Rating")));
}

#[test]
fn invalid_number_fails() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let mut values = HashMap::new();
    values.insert("bean".to_string(), "Test".to_string());
    values.insert("rating".to_string(), "abc".to_string());
    let form = make_form_state(values, HashMap::new());
    let errors = App::validate_form(module, &form);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("Rating") && e.contains("number"))
    );
}

#[test]
fn composite_required_empty_fails() {
    // Use a config where composite is required
    let toml = r####"
[vault]
base_path = "/tmp/vault"

[modules.t]
mode = "create"
path = "t.md"

[[modules.t.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Recipe"
required = true

[[modules.t.fields.sub_fields]]
name = "a"
field_type = "text"
prompt = "A"
"####;
    let config = Config::from_toml(toml).unwrap();
    let module = &config.modules["t"];
    let form = make_form_state(HashMap::new(), HashMap::new());
    let errors = App::validate_form(module, &form);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("Recipe") && e.contains("row"))
    );
}

#[test]
fn composite_optional_empty_passes() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    // recipe is not required
    let mut values = HashMap::new();
    values.insert("bean".to_string(), "Test".to_string());
    let form = make_form_state(values, HashMap::new());
    let errors = App::validate_form(module, &form);
    assert!(!errors.iter().any(|e| e.contains("Recipe")));
}

#[test]
fn composite_invalid_number_cell_fails() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let mut values = HashMap::new();
    values.insert("bean".to_string(), "Test".to_string());
    let mut composites = HashMap::new();
    composites.insert(
        "recipe".to_string(),
        vec![vec!["not-a-number".to_string(), "Bloom".to_string()]],
    );
    let form = make_form_state(values, composites);
    let errors = App::validate_form(module, &form);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("Amount") && e.contains("number"))
    );
}

#[test]
fn multiple_validation_errors_collected() {
    let app = make_app();
    let module = &app.config.modules["coffee"];
    let mut values = HashMap::new();
    // bean is required but missing
    values.insert("rating".to_string(), "abc".to_string()); // invalid number
    let form = make_form_state(values, HashMap::new());
    let errors = App::validate_form(module, &form);
    assert!(
        errors.len() >= 2,
        "expected at least 2 errors, got: {:?}",
        errors
    );
}

// ── build_sub_field_settings ──

#[test]
fn sub_field_text_settings() {
    let app = make_app();
    let field = &app.config.modules["coffee"].fields[5]; // recipe
    let sub = &field.sub_fields.as_ref().unwrap()[0]; // amount (number)
    let settings = App::build_sub_field_settings(sub);
    let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
    assert!(keys.contains(&"name"));
    assert!(keys.contains(&"prompt"));
    assert!(keys.contains(&"field_type"));
    // Number sub-field should not have options
    assert!(!keys.contains(&"options"));
}

#[test]
fn sub_field_static_select_has_options() {
    let app = make_app();
    let field = &app.config.modules["coffee"].fields[5]; // recipe
    let sub = &field.sub_fields.as_ref().unwrap()[1]; // type (static_select)
    let settings = App::build_sub_field_settings(sub);
    let opts = settings.iter().find(|s| s.key == "options").unwrap();
    assert!(matches!(opts.kind, SettingKind::ListEditor));
    assert_eq!(opts.value, "Bloom\nSpiral");
}
