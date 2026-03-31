use pour::config::Config;
use pour::config::{FieldTarget, FieldType, WriteMode};

/// A representative config string that exercises every struct and enum variant.
const SAMPLE_TOML: &str = r####"
[vault]
base_path = "C:/Users/Joseph/obsidian-vault"
api_port = 27124
api_key = "secret-token"

[modules.me]
mode = "append"
path = "Journal/%Y/%Y-%m-%d.md"
append_under_header = "## Log"
append_template = "> [!note] {{time}}\n> {{body}}"
display_name = "Journal"

[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "What's on your mind?"
required = true
target = "body"

[modules.coffee]
mode = "create"
path = "Coffee/%Y/%Y-%m-%d-%H%M%S.md"
display_name = "Coffee"

[[modules.coffee.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
required = true
options = ["V60", "AeroPress", "Espresso", "French Press"]
target = "frontmatter"

[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating (1-5)"
default = "3"

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Tasting notes"
target = "body"
"####;

#[test]
fn round_trip_sample_config() {
    let config = Config::from_toml(SAMPLE_TOML).expect("should parse sample TOML");

    // Vault
    assert_eq!(config.vault.base_path, "C:/Users/Joseph/obsidian-vault");
    assert_eq!(config.vault.api_port, Some(27124));
    assert_eq!(config.vault.api_key.as_deref(), Some("secret-token"));

    // Modules exist
    assert!(config.modules.contains_key("me"));
    assert!(config.modules.contains_key("coffee"));

    // Module: me
    let me = &config.modules["me"];
    assert_eq!(me.mode, WriteMode::Append);
    assert_eq!(me.append_under_header.as_deref(), Some("## Log"));
    assert!(me.append_template.is_some());
    assert_eq!(me.fields.len(), 1);
    assert_eq!(me.fields[0].field_type, FieldType::Textarea);
    assert_eq!(me.fields[0].target, Some(FieldTarget::Body));

    // Module: coffee
    let coffee = &config.modules["coffee"];
    assert_eq!(coffee.mode, WriteMode::Create);
    assert_eq!(coffee.fields.len(), 4);

    // static_select field
    let brew = &coffee.fields[0];
    assert_eq!(brew.field_type, FieldType::StaticSelect);
    assert_eq!(brew.options.as_ref().unwrap().len(), 4);

    // dynamic_select field
    let bean = &coffee.fields[1];
    assert_eq!(bean.field_type, FieldType::DynamicSelect);
    assert_eq!(bean.source.as_deref(), Some("Coffee/Beans"));

    // number field with default
    let rating = &coffee.fields[2];
    assert_eq!(rating.field_type, FieldType::Number);
    assert_eq!(rating.default.as_deref(), Some("3"));

    // textarea field
    let notes = &coffee.fields[3];
    assert_eq!(notes.field_type, FieldType::Textarea);
    assert_eq!(notes.target, Some(FieldTarget::Body));
}

#[test]
fn api_port_defaults_when_omitted() {
    let toml_str = r####"
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
    let config = Config::from_toml(toml_str).expect("should parse");
    assert_eq!(config.vault.api_port, Some(27124));
}

#[test]
fn minimal_config_parses() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.quick]
mode = "create"
path = "quick.md"

[[modules.quick.fields]]
name = "note"
field_type = "text"
prompt = "Note"
"####;
    let config = Config::from_toml(toml_str).expect("should parse minimal config");
    assert_eq!(config.modules.len(), 1);
    assert!(config.modules.contains_key("quick"));
}

#[test]
fn valid_config_parses_via_from_str() {
    let result = Config::from_toml(SAMPLE_TOML);
    assert!(result.is_ok(), "valid config should parse: {result:?}");
}

#[test]
fn module_with_no_fields_fails_validation() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.empty]
mode = "create"
path = "empty.md"
fields = []
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("at least one field"),
        "expected 'at least one field' error, got: {msg}"
    );
}

#[test]
fn append_mode_without_header_fails_validation() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.journal]
mode = "append"
path = "journal.md"

[[modules.journal.fields]]
name = "body"
field_type = "textarea"
prompt = "Write"
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("append_under_header"),
        "expected append_under_header error, got: {msg}"
    );
}

#[test]
fn static_select_without_options_fails_validation() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "choice"
field_type = "static_select"
prompt = "Pick one"
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("static_select requires 'options'"),
        "expected options error, got: {msg}"
    );
}

#[test]
fn static_select_with_empty_options_fails_validation() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "choice"
field_type = "static_select"
prompt = "Pick one"
options = []
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("must not be empty"),
        "expected empty options error, got: {msg}"
    );
}

#[test]
fn dynamic_select_without_source_fails_validation() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "item"
field_type = "dynamic_select"
prompt = "Select item"
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("dynamic_select requires 'source'"),
        "expected source error, got: {msg}"
    );
}

#[test]
fn invalid_toml_produces_parse_error() {
    let result = Config::from_toml("this is not valid toml {{{{");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("failed to parse config"),
        "expected parse error, got: {msg}"
    );
}

#[test]
fn api_key_env_var_overrides_config() {
    // SAFETY: test is single-threaded via cargo test -- --test-threads=1 or
    // env var is scoped tightly. Acceptable in test code.
    unsafe {
        std::env::set_var("POUR_API_KEY", "env-secret");
    }
    let result = Config::from_toml(SAMPLE_TOML);
    unsafe {
        std::env::remove_var("POUR_API_KEY");
    }

    let config = result.expect("should parse");
    assert_eq!(config.vault.api_key.as_deref(), Some("env-secret"));
}

#[test]
fn load_with_pour_config_env_var_nonexistent_file() {
    unsafe {
        std::env::set_var("POUR_CONFIG", "/nonexistent/path/config.toml");
    }
    let result = Config::load();
    unsafe {
        std::env::remove_var("POUR_CONFIG");
    }

    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found"),
        "expected not found error, got: {msg}"
    );
}

#[test]
fn load_from_pour_config_env_var() {
    // Write a temp config file and point POUR_CONFIG at it
    let dir = std::env::temp_dir().join("pour_test_load");
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("config.toml");
    std::fs::write(
        &config_path,
        r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####,
    )
    .unwrap();

    unsafe {
        std::env::set_var("POUR_CONFIG", config_path.to_str().unwrap());
    }
    let result = Config::load();
    unsafe {
        std::env::remove_var("POUR_CONFIG");
    }
    let _ = std::fs::remove_dir_all(&dir);

    let config = result.expect("should load from POUR_CONFIG");
    assert_eq!(config.vault.base_path, "/tmp/vault");
    assert!(config.modules.contains_key("test"));
}
