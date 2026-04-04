use pour::config::Config;
use pour::config::{FieldTarget, FieldType, SubFieldType, TemplateFieldType, WriteMode};

/// A representative config string that exercises every struct and enum variant.
const SAMPLE_TOML: &str = r#####"
[vault]
base_path = "C:/Users/Joseph/obsidian-vault"
api_port = 27124
api_key = "secret-token"

[modules.me]
mode = "append"
path = "Journal/%Y/%Y-%m-%d.md"
append_under_header = "## Log"
append_template = "#### {{time}}\n> [!note] {{title}}\n> {{body}}"
display_name = "Journal"

[[modules.me.fields]]
name = "title"
field_type = "text"
prompt = "Title (optional)"
target = "body"

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
"#####;

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
    assert_eq!(me.fields.len(), 2);
    assert_eq!(me.fields[0].field_type, FieldType::Text);
    assert_eq!(me.fields[0].target, Some(FieldTarget::Body));
    assert_eq!(me.fields[1].field_type, FieldType::Textarea);
    assert_eq!(me.fields[1].target, Some(FieldTarget::Body));

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
fn dynamic_select_source_with_path_traversal_fails_validation() {
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
source = "../../etc/secrets"
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("must not contain '..'"),
        "expected path traversal error, got: {msg}"
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

// --- Path validation tests ---

/// Helper: build a minimal config TOML with the given module path.
fn config_with_module_path(path: &str) -> String {
    format!(
        r#"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "{path}"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"#
    )
}

/// Helper: build a minimal config TOML with a dynamic_select source path.
fn config_with_source_path(source: &str) -> String {
    format!(
        r#"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "valid/path.md"

[[modules.test.fields]]
name = "item"
field_type = "dynamic_select"
prompt = "Select"
source = "{source}"
"#
    )
}

#[test]
fn module_path_rejects_unix_absolute() {
    let result = Config::from_toml(&config_with_module_path("/etc/passwd"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("vault-relative"), "got: {msg}");
}

#[test]
fn module_path_rejects_windows_drive() {
    let result = Config::from_toml(&config_with_module_path("C:\\\\Users\\\\vault\\\\note.md"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("drive-qualified"), "got: {msg}");
}

#[test]
fn module_path_rejects_windows_drive_forward_slash() {
    let result = Config::from_toml(&config_with_module_path("D:/vault/note.md"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("drive-qualified"), "got: {msg}");
}

#[test]
fn module_path_rejects_unc_backslash() {
    let result = Config::from_toml(&config_with_module_path(
        "\\\\\\\\server\\\\share\\\\note.md",
    ));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("UNC"), "got: {msg}");
}

#[test]
fn module_path_rejects_unc_forward_slash() {
    // //server/share starts with '/' so it's caught as absolute — correct behavior
    let result = Config::from_toml(&config_with_module_path("//server/share/note.md"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("vault-relative"), "got: {msg}");
}

#[test]
fn module_path_rejects_traversal() {
    let result = Config::from_toml(&config_with_module_path("Journal/../../etc/passwd"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("traversal"), "got: {msg}");
}

#[test]
fn module_path_rejects_leading_traversal() {
    let result = Config::from_toml(&config_with_module_path("../outside.md"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("traversal"), "got: {msg}");
}

#[test]
fn module_path_accepts_vault_relative() {
    let result = Config::from_toml(&config_with_module_path("Journal/%Y/%Y-%m-%d.md"));
    assert!(result.is_ok(), "vault-relative path should be accepted");
}

#[test]
fn source_path_rejects_absolute() {
    let result = Config::from_toml(&config_with_source_path("/etc/secrets"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("vault-relative"), "got: {msg}");
}

#[test]
fn source_path_rejects_windows_drive() {
    let result = Config::from_toml(&config_with_source_path("C:\\\\Data\\\\beans"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("drive-qualified"), "got: {msg}");
}

#[test]
fn source_path_rejects_traversal() {
    let result = Config::from_toml(&config_with_source_path("../../etc/secrets"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("traversal"), "got: {msg}");
}

// --- composite_array tests ---

const COMPOSITE_TOML: &str = r####"
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

#[test]
fn composite_array_parses() {
    let config = Config::from_toml(COMPOSITE_TOML).expect("should parse composite_array config");
    let coffee = &config.modules["coffee"];
    assert_eq!(coffee.fields.len(), 2);

    let recipe = &coffee.fields[1];
    assert_eq!(recipe.field_type, FieldType::CompositeArray);
    assert_eq!(recipe.name, "recipe");

    let subs = recipe
        .sub_fields
        .as_ref()
        .expect("sub_fields should be Some");
    assert_eq!(subs.len(), 3);

    assert_eq!(subs[0].name, "pour");
    assert_eq!(subs[0].field_type, SubFieldType::Number);

    assert_eq!(subs[1].name, "time");
    assert_eq!(subs[1].field_type, SubFieldType::Number);

    assert_eq!(subs[2].name, "technique");
    assert_eq!(subs[2].field_type, SubFieldType::StaticSelect);
    assert_eq!(subs[2].options.as_ref().unwrap().len(), 4);
}

#[test]
fn composite_array_without_sub_fields_fails() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Stages"
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("composite_array requires 'sub_fields'"),
        "got: {msg}"
    );
}

#[test]
fn composite_array_with_empty_sub_fields_fails() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Stages"
sub_fields = []
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("must not be empty"), "got: {msg}");
}

#[test]
fn composite_array_select_sub_field_without_options_fails() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Stages"

[[modules.test.fields.sub_fields]]
name = "technique"
field_type = "static_select"
prompt = "Technique"
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("sub_field 'technique': static_select requires 'options'"),
        "got: {msg}"
    );
}

#[test]
fn composite_array_duplicate_sub_field_names_fails() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Stages"

[[modules.test.fields.sub_fields]]
name = "pour"
field_type = "number"
prompt = "Pour"

[[modules.test.fields.sub_fields]]
name = "pour"
field_type = "number"
prompt = "Pour again"
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("duplicate sub_field name 'pour'"),
        "got: {msg}"
    );
}

#[test]
fn existing_sample_toml_still_parses_with_composite_array() {
    // Regression guard: the original SAMPLE_TOML must still parse.
    let config = Config::from_toml(SAMPLE_TOML).expect("SAMPLE_TOML should still parse");
    assert!(config.modules.contains_key("me"));
    assert!(config.modules.contains_key("coffee"));
}

#[test]
fn callout_type_parses_on_module() {
    let toml = r###"
[vault]
base_path = "/tmp"

[modules.journal]
mode = "append"
path = "Journal/daily.md"
append_under_header = "## Log"
callout_type = "tip"

[[modules.journal.fields]]
name = "body"
field_type = "textarea"
prompt = "Body"
"###;
    let config = Config::from_toml(toml).unwrap();
    let module = &config.modules["journal"];
    assert_eq!(module.callout_type.as_deref(), Some("tip"));
}

#[test]
fn callout_parses_on_field() {
    let toml = r#"
[vault]
base_path = "/tmp"

[modules.test]
mode = "create"
path = "Test/note.md"

[[modules.test.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
callout = "warning"
"#;
    let config = Config::from_toml(toml).unwrap();
    let field = &config.modules["test"].fields[0];
    assert_eq!(field.callout.as_deref(), Some("warning"));
}

// --- allow_create tests ---

#[test]
fn allow_create_on_dynamic_select_is_valid() {
    let toml_str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Select bean"
source = "Coffee/Beans"
allow_create = true
"####;
    let config =
        Config::from_toml(toml_str).expect("allow_create on dynamic_select should be valid");
    let field = &config.modules["test"].fields[0];
    assert_eq!(field.allow_create, Some(true));
}

#[test]
fn allow_create_on_text_field_fails_validation() {
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
allow_create = true
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("allow_create is only valid on dynamic_select"),
        "expected allow_create type restriction error, got: {msg}"
    );
}

#[test]
fn allow_create_false_on_non_dynamic_select_fails_validation() {
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
allow_create = false
"####;
    let result = Config::from_toml(toml_str);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("allow_create is only valid on dynamic_select"),
        "expected allow_create type restriction error, got: {msg}"
    );
}

#[test]
fn allow_create_absent_defaults_to_none() {
    // Backward compat: existing configs without allow_create must parse identically.
    let config = Config::from_toml(SAMPLE_TOML).expect("SAMPLE_TOML should parse");
    let bean = &config.modules["coffee"].fields[1];
    assert_eq!(bean.field_type, FieldType::DynamicSelect);
    assert_eq!(
        bean.allow_create, None,
        "allow_create should default to None"
    );
}

#[test]
fn callout_fields_default_to_none() {
    let config = Config::from_toml(SAMPLE_TOML).unwrap();
    let me = &config.modules["me"];
    assert!(
        me.callout_type.is_none(),
        "callout_type should default to None"
    );
    assert!(
        me.fields[0].callout.is_none(),
        "field callout should default to None"
    );
}

// --- Template tests ---

/// Helper: build a minimal valid config with a template section appended.
fn config_with_template(template_toml: &str) -> String {
    format!(
        r#"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "Test/{{{{title}}}}.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"

{template_toml}
"#
    )
}

#[test]
fn config_with_templates_parses() {
    let toml = config_with_template(
        r#"
[templates.bean_template]
path = "Coffee/Beans/{{name}}.md"

[[templates.bean_template.fields]]
name = "origin"
field_type = "text"
prompt = "Origin country"

[[templates.bean_template.fields]]
name = "roast"
field_type = "static_select"
prompt = "Roast level"
options = ["Light", "Medium", "Dark"]
"#,
    );
    let config = Config::from_toml(&toml).unwrap();
    let templates = config.templates.as_ref().expect("templates should be Some");
    let bean = &templates["bean_template"];
    assert_eq!(bean.fields.len(), 2);
    assert_eq!(bean.path, "Coffee/Beans/{{name}}.md");
    assert_eq!(bean.fields[0].field_type, TemplateFieldType::Text);
    assert_eq!(bean.fields[1].field_type, TemplateFieldType::StaticSelect);
}

#[test]
fn config_without_templates_parses() {
    let config = Config::from_toml(SAMPLE_TOML).unwrap();
    assert!(config.templates.is_none());
}

#[test]
fn template_path_without_name_placeholder_fails() {
    let toml = config_with_template(
        r#"
[templates.bad]
path = "Coffee/Beans/test.md"

[[templates.bad.fields]]
name = "origin"
field_type = "text"
prompt = "Origin"
"#,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("path must contain"),
        "expected path placeholder error, got: {err}"
    );
}

#[test]
fn template_path_with_traversal_fails() {
    let toml = config_with_template(
        r#"
[templates.bad]
path = "Coffee/../Beans/{{name}}.md"

[[templates.bad.fields]]
name = "origin"
field_type = "text"
prompt = "Origin"
"#,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains(".."),
        "expected path traversal error, got: {err}"
    );
}

#[test]
fn template_with_no_fields_fails() {
    let toml = config_with_template(
        r#"
[templates.bad]
path = "Coffee/Beans/{{name}}.md"
fields = []
"#,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("must have at least one field"),
        "expected empty fields error, got: {err}"
    );
}

#[test]
fn template_with_duplicate_field_names_fails() {
    let toml = config_with_template(
        r#"
[templates.bad]
path = "Coffee/Beans/{{name}}.md"

[[templates.bad.fields]]
name = "origin"
field_type = "text"
prompt = "Origin"

[[templates.bad.fields]]
name = "origin"
field_type = "number"
prompt = "Origin again"
"#,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("duplicate field name"),
        "expected duplicate field error, got: {err}"
    );
}

#[test]
fn template_static_select_without_options_fails() {
    let toml = config_with_template(
        r#"
[templates.bad]
path = "Coffee/Beans/{{name}}.md"

[[templates.bad.fields]]
name = "roast"
field_type = "static_select"
prompt = "Roast level"
"#,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("static_select requires"),
        "expected missing options error, got: {err}"
    );
}

#[test]
fn template_static_select_with_empty_options_fails() {
    let toml = config_with_template(
        r#"
[templates.bad]
path = "Coffee/Beans/{{name}}.md"

[[templates.bad.fields]]
name = "roast"
field_type = "static_select"
prompt = "Roast level"
options = []
"#,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("must not be empty"),
        "expected empty options error, got: {err}"
    );
}

// --- create_template cross-reference validation tests ---

/// Helper: config with a dynamic_select field and optionally a template section.
fn config_with_create_template(field_extra: &str, template_section: &str) -> String {
    format!(
        r#"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "Test/test.md"

[[modules.test.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"
{field_extra}

{template_section}
"#
    )
}

const VALID_TEMPLATE: &str = r#"
[templates.bean_template]
path = "Coffee/Beans/{{name}}.md"

[[templates.bean_template.fields]]
name = "origin"
field_type = "text"
prompt = "Origin"
"#;

#[test]
fn create_template_on_non_dynamic_select_fails() {
    let toml = format!(
        r#"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "Test/test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
allow_create = true
create_template = "bean_template"

{VALID_TEMPLATE}
"#
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("create_template is only valid on dynamic_select"),
        "expected field type error, got: {err}"
    );
}

#[test]
fn create_template_without_allow_create_fails() {
    let toml =
        config_with_create_template(r#"create_template = "bean_template""#, VALID_TEMPLATE);
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("create_template requires allow_create = true"),
        "expected allow_create requirement error, got: {err}"
    );
}

#[test]
fn create_template_referencing_nonexistent_template_fails() {
    let toml = config_with_create_template(
        r#"allow_create = true
create_template = "nonexistent""#,
        VALID_TEMPLATE,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("create_template references unknown template 'nonexistent'"),
        "expected unknown template error, got: {err}"
    );
}

#[test]
fn post_create_command_without_create_template_fails() {
    let toml = config_with_create_template(
        r#"allow_create = true
post_create_command = "templater:run""#,
        VALID_TEMPLATE,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("post_create_command requires create_template to be set"),
        "expected post_create_command error, got: {err}"
    );
}

#[test]
fn valid_create_template_with_allow_create_and_existing_template_passes() {
    let toml = config_with_create_template(
        r#"allow_create = true
create_template = "bean_template""#,
        VALID_TEMPLATE,
    );
    Config::from_toml(&toml).expect("valid create_template config should pass");
}

#[test]
fn template_field_named_date_is_rejected() {
    let toml = config_with_template(
        r#"
[templates.bad]
path = "Beans/{{name}}.md"

[[templates.bad.fields]]
name = "date"
field_type = "text"
prompt = "Date"
"#,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("'date' is reserved"),
        "expected reserved field name error, got: {err}"
    );
}

#[test]
fn template_field_named_name_is_rejected() {
    let toml = config_with_template(
        r#"
[templates.bad]
path = "Beans/{{name}}.md"

[[templates.bad.fields]]
name = "name"
field_type = "text"
prompt = "Name"
"#,
    );
    let err = Config::from_toml(&toml).unwrap_err().to_string();
    assert!(
        err.contains("'name' is reserved"),
        "expected reserved field name error, got: {err}"
    );
}

#[test]
fn valid_create_template_with_post_create_command_passes() {
    let toml = config_with_create_template(
        r#"allow_create = true
create_template = "bean_template"
post_create_command = "templater:run""#,
        VALID_TEMPLATE,
    );
    Config::from_toml(&toml).expect("valid create_template + post_create_command should pass");
}

// --- show_when tests ---

/// Helper: minimal config with a show_when clause on the second field.
fn config_with_show_when(show_when_toml: &str) -> String {
    format!(
        r#"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
options = ["Espresso", "AeroPress", "V60"]

[[modules.test.fields]]
name = "pressure"
field_type = "number"
prompt = "Pressure (bar)"
{show_when_toml}
"#
    )
}

#[test]
fn show_when_equals_deserializes_correctly() {
    let toml = config_with_show_when(r#"show_when = { field = "brew_method", equals = "Espresso" }"#);
    let config = Config::from_toml(&toml).expect("show_when equals should parse");
    let field = &config.modules["test"].fields[1];
    let sw = field.show_when.as_ref().expect("show_when should be Some");
    assert_eq!(sw.field, "brew_method");
    assert_eq!(sw.equals.as_deref(), Some("Espresso"));
    assert!(sw.one_of.is_none());
}

#[test]
fn show_when_one_of_deserializes_correctly() {
    let toml = config_with_show_when(
        r#"show_when = { field = "brew_method", one_of = ["Espresso", "AeroPress"] }"#,
    );
    let config = Config::from_toml(&toml).expect("show_when one_of should parse");
    let field = &config.modules["test"].fields[1];
    let sw = field.show_when.as_ref().expect("show_when should be Some");
    assert_eq!(sw.field, "brew_method");
    assert!(sw.equals.is_none());
    let one_of = sw.one_of.as_ref().expect("one_of should be Some");
    assert_eq!(one_of, &["Espresso", "AeroPress"]);
}

#[test]
fn show_when_neither_equals_nor_one_of_fails_validation() {
    let toml = config_with_show_when(r#"show_when = { field = "brew_method" }"#);
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("show_when on field 'pressure': must specify 'equals' or 'one_of'"),
        "expected missing condition error, got: {msg}"
    );
}

#[test]
fn show_when_both_equals_and_one_of_fails_validation() {
    let toml = config_with_show_when(
        r#"show_when = { field = "brew_method", equals = "Espresso", one_of = ["Espresso", "AeroPress"] }"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("show_when on field 'pressure': specify either 'equals' or 'one_of', not both"),
        "expected conflicting condition error, got: {msg}"
    );
}

#[test]
fn show_when_absent_defaults_to_none() {
    let config = Config::from_toml(SAMPLE_TOML).expect("SAMPLE_TOML should parse");
    for field in &config.modules["coffee"].fields {
        assert!(
            field.show_when.is_none(),
            "field '{}' show_when should default to None",
            field.name
        );
    }
}

// --- show_when cross-field reference validation tests ---

/// Helper: build a multi-field module config for show_when reference tests.
fn config_with_show_when_ref(fields_toml: &str) -> String {
    format!(
        r#"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

{fields_toml}
"#
    )
}

#[test]
fn show_when_valid_backward_reference_passes() {
    // Field B references field A which appears earlier — valid.
    let toml = config_with_show_when_ref(
        r#"
[[modules.test.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
options = ["Espresso", "V60"]

[[modules.test.fields]]
name = "pressure"
field_type = "number"
prompt = "Pressure"
show_when = { field = "brew_method", equals = "Espresso" }
"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_ok(), "backward reference should be valid: {result:?}");
}

#[test]
fn show_when_valid_forward_reference_passes() {
    // Field A references field B which appears later — forward reference is allowed.
    let toml = config_with_show_when_ref(
        r#"
[[modules.test.fields]]
name = "pressure"
field_type = "number"
prompt = "Pressure"
show_when = { field = "brew_method", equals = "Espresso" }

[[modules.test.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
options = ["Espresso", "V60"]
"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_ok(), "forward reference should be allowed: {result:?}");
}

#[test]
fn show_when_unknown_field_reference_fails() {
    let toml = config_with_show_when_ref(
        r#"
[[modules.test.fields]]
name = "pressure"
field_type = "number"
prompt = "Pressure"
show_when = { field = "nonexistent", equals = "Espresso" }
"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Field 'pressure' in module 'test' has show_when referencing unknown field 'nonexistent'"),
        "expected unknown field error, got: {msg}"
    );
}

#[test]
fn show_when_self_reference_fails() {
    let toml = config_with_show_when_ref(
        r#"
[[modules.test.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
options = ["Espresso", "V60"]
show_when = { field = "brew_method", equals = "Espresso" }
"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Field 'brew_method' in module 'test' has show_when referencing itself"),
        "expected self-reference error, got: {msg}"
    );
}

#[test]
fn show_when_composite_array_reference_fails() {
    let toml = config_with_show_when_ref(
        r#"
[[modules.test.fields]]
name = "shots"
field_type = "composite_array"
prompt = "Shots"

[[modules.test.fields.sub_fields]]
name = "vol"
field_type = "number"
prompt = "Volume"

[[modules.test.fields]]
name = "notes"
field_type = "text"
prompt = "Notes"
show_when = { field = "shots", equals = "1" }
"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("show_when on field 'notes' in module 'test' cannot reference composite_array field 'shots'"),
        "expected composite_array reference error, got: {msg}"
    );
}

#[test]
fn show_when_direct_circular_dependency_fails() {
    // A → B and B → A
    let toml = config_with_show_when_ref(
        r#"
[[modules.test.fields]]
name = "field_a"
field_type = "text"
prompt = "A"
show_when = { field = "field_b", equals = "x" }

[[modules.test.fields]]
name = "field_b"
field_type = "text"
prompt = "B"
show_when = { field = "field_a", equals = "y" }
"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Circular show_when dependency detected in module 'test'"),
        "expected circular dependency error, got: {msg}"
    );
}

#[test]
fn show_when_transitive_circular_dependency_fails() {
    // A → B → C → A
    let toml = config_with_show_when_ref(
        r#"
[[modules.test.fields]]
name = "field_a"
field_type = "text"
prompt = "A"
show_when = { field = "field_b", equals = "x" }

[[modules.test.fields]]
name = "field_b"
field_type = "text"
prompt = "B"
show_when = { field = "field_c", equals = "y" }

[[modules.test.fields]]
name = "field_c"
field_type = "text"
prompt = "C"
show_when = { field = "field_a", equals = "z" }
"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Circular show_when dependency detected in module 'test'"),
        "expected transitive circular dependency error, got: {msg}"
    );
}

// --- config_version tests ---

/// Minimal config helper without any config_version key.
const MINIMAL_TOML_NO_VERSION: &str = r#"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"#;

#[test]
fn config_without_version_defaults_to_0_1_0() {
    let config = Config::from_toml(MINIMAL_TOML_NO_VERSION)
        .expect("config without config_version should parse successfully");
    assert_eq!(
        config.config_version.as_deref(),
        Some("0.1.0"),
        "missing config_version should resolve to 0.1.0"
    );
}

#[test]
fn config_version_0_1_0_parses_correctly() {
    let toml = format!("config_version = \"0.1.0\"\n{MINIMAL_TOML_NO_VERSION}");
    let config = Config::from_toml(&toml).expect("config_version = 0.1.0 should parse");
    assert_eq!(config.config_version.as_deref(), Some("0.1.0"));
}

#[test]
fn config_version_0_2_0_parses_correctly() {
    let toml = format!("config_version = \"0.2.0\"\n{MINIMAL_TOML_NO_VERSION}");
    let config = Config::from_toml(&toml).expect("config_version = 0.2.0 should parse");
    assert_eq!(config.config_version.as_deref(), Some("0.2.0"));
}

#[test]
fn config_version_unsupported_major_is_rejected() {
    let toml = format!("config_version = \"99.0.0\"\n{MINIMAL_TOML_NO_VERSION}");
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("99.0.0") && msg.contains("not supported"),
        "expected unsupported version error, got: {msg}"
    );
}

#[test]
fn config_version_empty_string_is_rejected() {
    let toml = format!("config_version = \"\"\n{MINIMAL_TOML_NO_VERSION}");
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("config_version") && msg.contains("empty"),
        "expected empty config_version error, got: {msg}"
    );
}

#[test]
fn config_version_leading_zeros_rejected() {
    let toml = format!("config_version = \"00.01.00\"\n{MINIMAL_TOML_NO_VERSION}");
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("leading zeros"),
        "expected leading zeros error, got: {msg}"
    );
}

#[test]
fn config_version_two_part_rejected() {
    let toml = format!("config_version = \"1.0\"\n{MINIMAL_TOML_NO_VERSION}");
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("1.0") && msg.contains("major.minor.patch"),
        "expected wrong-parts-count error, got: {msg}"
    );
}

#[test]
fn config_version_prefixed_rejected() {
    let toml = format!("config_version = \"v0.1.0\"\n{MINIMAL_TOML_NO_VERSION}");
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("v0.1.0") && msg.contains("major.minor.patch"),
        "expected non-numeric segment error, got: {msg}"
    );
}

#[test]
fn show_when_empty_one_of_rejected() {
    let toml = config_with_show_when(r#"show_when = { field = "brew_method", one_of = [] }"#);
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("must not be empty"),
        "expected empty one_of error, got: {msg}"
    );
}

#[test]
fn show_when_empty_equals_rejected() {
    let toml = config_with_show_when(r#"show_when = { field = "brew_method", equals = "" }"#);
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("must not be empty"),
        "expected empty equals error, got: {msg}"
    );
}

#[test]
fn show_when_two_disjoint_cycles_both_reported() {
    // A↔B and C↔D in the same module — both cycles must appear in the error.
    let toml = config_with_show_when_ref(
        r#"
[[modules.test.fields]]
name = "field_a"
field_type = "text"
prompt = "A"
show_when = { field = "field_b", equals = "x" }

[[modules.test.fields]]
name = "field_b"
field_type = "text"
prompt = "B"
show_when = { field = "field_a", equals = "y" }

[[modules.test.fields]]
name = "field_c"
field_type = "text"
prompt = "C"
show_when = { field = "field_d", equals = "x" }

[[modules.test.fields]]
name = "field_d"
field_type = "text"
prompt = "D"
show_when = { field = "field_c", equals = "y" }
"#,
    );
    let result = Config::from_toml(&toml);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    // Both cycles must be reported — count occurrences of the cycle error message
    let cycle_count = msg
        .matches("Circular show_when dependency detected in module 'test'")
        .count();
    assert_eq!(
        cycle_count, 2,
        "expected both A↔B and C↔D cycles to be reported, got: {msg}"
    );
}
