use pour::config::{
    Config, ConfigError, FieldConfig, FieldTarget, FieldType, FieldUpdates, ModuleConfig,
    ModuleUpdates, VaultUpdates, WriteMode,
};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use tempfile::NamedTempFile;

/// Serialise tests that mutate the `POUR_CONFIG` env var.
///
/// `set_var` is process-global; parallel tests would race on the value.
/// Holding this lock for the duration of each test prevents that.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Minimal config with a comment, an append-mode module, and a create-mode module.
const BASE_TOML: &str = r###"
# Vault configuration
[vault]
base_path = "C:/vault"

# Modules
[modules.journal]
mode = "append"
path = "Journal/daily.md"
append_under_header = "## Log"
display_name = "Journal"

[[modules.journal.fields]]
name = "body"
field_type = "textarea"
prompt = "What happened?"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean used?"
"###;

/// Write `content` to a `NamedTempFile` and point `POUR_CONFIG` at it.
///
/// Returns `(temp_file, lock_guard)`. The caller must hold both for the
/// duration of the test: `temp_file` keeps the file on disk and
/// `lock_guard` serialises access to the process-wide `POUR_CONFIG` env var.
fn write_temp_config(content: &str) -> (NamedTempFile, std::sync::MutexGuard<'static, ()>) {
    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = NamedTempFile::new().expect("failed to create temp file");
    f.write_all(content.as_bytes())
        .expect("failed to write temp config");
    f.flush().expect("failed to flush temp config");
    // SAFETY: guarded by ENV_LOCK so only one thread holds this at a time.
    unsafe { std::env::set_var("POUR_CONFIG", f.path().to_str().unwrap()) };
    (f, guard)
}

#[test]
fn update_preserves_comments() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let updates = ModuleUpdates {
        path: Some("Journal/new-path.md".to_string()),
        display_name: None,
        mode: None,
        append_under_header: None,
        callout_type: None,
    };

    Config::update_module_on_disk("journal", &updates).expect("update should succeed");

    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");

    // Comment must survive.
    assert!(
        written.contains("# Vault configuration"),
        "top-level comment was lost"
    );
    assert!(written.contains("# Modules"), "modules comment was lost");

    // Path must be updated.
    assert!(
        written.contains("Journal/new-path.md"),
        "updated path not found in config"
    );

    // Unrelated fields must still be present.
    assert!(
        written.contains("append_under_header"),
        "append_under_header was removed unexpectedly"
    );
}

#[test]
fn update_mode_toggle() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let updates = ModuleUpdates {
        path: None,
        display_name: None,
        mode: Some(WriteMode::Create),
        append_under_header: Some(None), // remove the header key so validation passes
        callout_type: None,
    };

    Config::update_module_on_disk("journal", &updates).expect("mode toggle should succeed");

    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");

    // Parse result and check mode.
    let config = Config::from_toml(&written).expect("updated config should be valid");
    let journal = config
        .modules
        .get("journal")
        .expect("journal module missing");
    assert_eq!(
        journal.mode,
        WriteMode::Create,
        "mode should have been toggled to create"
    );
    assert!(
        journal.append_under_header.is_none(),
        "append_under_header should have been removed"
    );
}

#[test]
fn update_validation_prevents_bad_writes() {
    let (_f, _guard) = write_temp_config(BASE_TOML);
    let config_path = std::env::var("POUR_CONFIG").unwrap();

    let original_content =
        std::fs::read_to_string(&config_path).expect("failed to read config before update");

    // Remove append_under_header from an append-mode module — invalid.
    let updates = ModuleUpdates {
        path: None,
        display_name: None,
        mode: None,
        append_under_header: Some(None),
        callout_type: None,
    };

    let result = Config::update_module_on_disk("journal", &updates);

    assert!(
        matches!(result, Err(ConfigError::ValidationError(_))),
        "expected ValidationError, got: {result:?}"
    );

    // File must be unchanged.
    let after_content =
        std::fs::read_to_string(&config_path).expect("failed to read config after failed update");

    assert_eq!(
        original_content, after_content,
        "file was modified despite failed validation"
    );
}

#[test]
fn update_nonexistent_module_errors() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let updates = ModuleUpdates {
        path: Some("anywhere.md".to_string()),
        display_name: None,
        mode: None,
        append_under_header: None,
        callout_type: None,
    };

    let result = Config::update_module_on_disk("nonexistent", &updates);
    assert!(
        matches!(result, Err(ConfigError::ModuleNotFound(_))),
        "expected ModuleNotFound, got: {result:?}"
    );
}

// --- Field update tests ---

/// Extended config with a static_select field for field-level update tests.
const FIELD_TOML: &str = r###"
[vault]
base_path = "C:/vault"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"

# The main field
[[modules.coffee.fields]]
name = "method"
field_type = "static_select"
prompt = "Brew method"
required = true
options = ["V60", "AeroPress", "Espresso"]

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Tasting notes"
target = "body"
"###;

#[test]
fn update_field_name_and_prompt() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    let updates = FieldUpdates {
        name: Some("brew_method".to_string()),
        prompt: Some("Choose brew method".to_string()),
        field_type: None,
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        callout: None,
    };

    Config::update_field_on_disk("coffee", 0, &updates).expect("field update should succeed");

    let config = Config::load().expect("reload should succeed");
    let field = &config.modules["coffee"].fields[0];
    assert_eq!(field.name, "brew_method");
    assert_eq!(field.prompt, "Choose brew method");
    // Unchanged fields should be preserved
    assert_eq!(field.field_type, FieldType::StaticSelect);
    assert_eq!(field.required, Some(true));
}

#[test]
fn update_field_type_with_options() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    // Change the textarea field to a static_select and add options
    let updates = FieldUpdates {
        name: None,
        field_type: Some(FieldType::StaticSelect),
        prompt: None,
        required: None,
        default: None,
        options: Some(Some(vec!["Good".to_string(), "Bad".to_string()])),
        source: None,
        target: Some(Some(FieldTarget::Frontmatter)),
        callout: None,
    };

    Config::update_field_on_disk("coffee", 1, &updates).expect("type change should succeed");

    let config = Config::load().expect("reload should succeed");
    let field = &config.modules["coffee"].fields[1];
    assert_eq!(field.field_type, FieldType::StaticSelect);
    assert_eq!(field.options.as_ref().unwrap(), &["Good", "Bad"]);
    assert_eq!(field.target, Some(FieldTarget::Frontmatter));
}

#[test]
fn update_field_preserves_comments() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    let updates = FieldUpdates {
        name: None,
        field_type: None,
        prompt: Some("Updated prompt".to_string()),
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        callout: None,
    };

    Config::update_field_on_disk("coffee", 0, &updates).expect("update should succeed");

    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");

    assert!(
        written.contains("# The main field"),
        "comment was lost during field update"
    );
}

#[test]
fn update_field_validation_rejects_select_without_options() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);
    let config_path = std::env::var("POUR_CONFIG").unwrap();
    let original = std::fs::read_to_string(&config_path).unwrap();

    // Change textarea to static_select without providing options — invalid
    let updates = FieldUpdates {
        name: None,
        field_type: Some(FieldType::StaticSelect),
        prompt: None,
        required: None,
        default: None,
        options: None, // not providing options — should fail validation
        source: None,
        target: None,
        callout: None,
    };

    let result = Config::update_field_on_disk("coffee", 1, &updates);
    assert!(
        matches!(result, Err(ConfigError::ValidationError(_))),
        "expected ValidationError, got: {result:?}"
    );

    // File must be unchanged
    let after = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(
        original, after,
        "file was modified despite validation failure"
    );
}

#[test]
fn update_field_out_of_range_errors() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    let updates = FieldUpdates {
        name: Some("oops".to_string()),
        field_type: None,
        prompt: None,
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        callout: None,
    };

    let result = Config::update_field_on_disk("coffee", 99, &updates);
    assert!(
        result.is_err(),
        "expected error for out-of-range field index"
    );
}

#[test]
fn update_field_remove_optional_keys() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    // Remove required and target from the first field
    let updates = FieldUpdates {
        name: None,
        field_type: None,
        prompt: None,
        required: Some(None), // remove the key
        default: None,
        options: None,
        source: None,
        target: None,
        callout: None,
    };

    Config::update_field_on_disk("coffee", 0, &updates).expect("remove should succeed");

    let config = Config::load().expect("reload should succeed");
    let field = &config.modules["coffee"].fields[0];
    assert_eq!(field.required, None, "required should have been removed");
}

// --- Vault update tests ---

#[test]
fn update_vault_base_path() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let updates = VaultUpdates {
        base_path: Some("D:/my-vault".to_string()),
        api_port: None,
        api_key: None,
        date_format: None,
    };

    Config::update_vault_on_disk(&updates).expect("vault base_path update should succeed");

    let config = Config::load().expect("reload should succeed");
    assert_eq!(
        config.vault.base_path, "D:/my-vault",
        "base_path was not updated"
    );
}

#[test]
fn update_vault_api_port() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let updates = VaultUpdates {
        base_path: None,
        api_port: Some(Some(8080)),
        api_key: None,
        date_format: None,
    };

    Config::update_vault_on_disk(&updates).expect("vault api_port update should succeed");

    let config = Config::load().expect("reload should succeed");
    assert_eq!(
        config.vault.api_port,
        Some(8080),
        "api_port was not updated"
    );
}

#[test]
fn update_vault_removes_optional_keys() {
    // Start with a config that has api_key set
    const WITH_KEY: &str = r###"
[vault]
base_path = "C:/vault"
api_key = "secret-token"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean used?"
"###;

    let (_f, _guard) = write_temp_config(WITH_KEY);

    let updates = VaultUpdates {
        base_path: None,
        api_port: None,
        api_key: Some(None), // remove the key
        date_format: None,
    };

    Config::update_vault_on_disk(&updates).expect("removing api_key should succeed");

    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");

    assert!(
        !written.contains("api_key"),
        "api_key should have been removed from the config file"
    );

    let config = Config::load().expect("reload should succeed");
    assert!(
        config.vault.api_key.is_none(),
        "api_key should be None after removal"
    );
}

// --- add_field_on_disk / remove_field_on_disk tests ---

#[test]
fn add_field_appends_to_module() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    let new_field = FieldConfig {
        name: "grind_size".to_string(),
        field_type: FieldType::Text,
        prompt: "Grind size?".to_string(),
        required: None,
        default: Some("medium".to_string()),
        options: None,
        source: None,
        target: None,
        sub_fields: None,
        callout: None,
    };

    Config::add_field_on_disk("coffee", &new_field).expect("add_field should succeed");

    let config = Config::load().expect("reload should succeed");
    let fields = &config.modules["coffee"].fields;

    // Original two fields must still be present.
    assert_eq!(fields[0].name, "method");
    assert_eq!(fields[1].name, "notes");

    // New field appended at the end.
    assert_eq!(fields.len(), 3, "expected 3 fields after add");
    let added = &fields[2];
    assert_eq!(added.name, "grind_size");
    assert_eq!(added.field_type, FieldType::Text);
    assert_eq!(added.prompt, "Grind size?");
    assert_eq!(added.default.as_deref(), Some("medium"));
}

#[test]
fn add_field_to_nonexistent_module_errors() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    let new_field = FieldConfig {
        name: "anything".to_string(),
        field_type: FieldType::Text,
        prompt: "Anything".to_string(),
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        sub_fields: None,
        callout: None,
    };

    let result = Config::add_field_on_disk("nonexistent", &new_field);
    assert!(
        matches!(result, Err(ConfigError::ModuleNotFound(_))),
        "expected ModuleNotFound, got: {result:?}"
    );
}

#[test]
fn remove_field_by_index() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    // Remove field 0 ("method"); field 1 ("notes") should shift to index 0.
    Config::remove_field_on_disk("coffee", 0).expect("remove should succeed");

    let config = Config::load().expect("reload should succeed");
    let fields = &config.modules["coffee"].fields;

    assert_eq!(fields.len(), 1, "expected 1 field after remove");
    assert_eq!(fields[0].name, "notes");
}

#[test]
fn remove_last_field_fails_validation() {
    // A config with only one field — removing it must be rejected.
    const ONE_FIELD_TOML: &str = r###"
[vault]
base_path = "C:/vault"

[modules.solo]
mode = "create"
path = "Solo/log.md"

[[modules.solo.fields]]
name = "only"
field_type = "text"
prompt = "The only field"
"###;

    let (_f, _guard) = write_temp_config(ONE_FIELD_TOML);
    let config_path = std::env::var("POUR_CONFIG").unwrap();
    let original = std::fs::read_to_string(&config_path).unwrap();

    let result = Config::remove_field_on_disk("solo", 0);
    assert!(
        matches!(result, Err(ConfigError::ValidationError(_))),
        "expected ValidationError when removing the last field, got: {result:?}"
    );

    // File must be unchanged.
    let after = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(
        original, after,
        "file was modified despite validation failure"
    );
}

#[test]
fn add_field_preserves_comments() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    let new_field = FieldConfig {
        name: "temperature".to_string(),
        field_type: FieldType::Number,
        prompt: "Water temp (°C)?".to_string(),
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        sub_fields: None,
        callout: None,
    };

    Config::add_field_on_disk("coffee", &new_field).expect("add_field should succeed");

    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");

    assert!(
        written.contains("# The main field"),
        "comment was lost after add_field"
    );
}

// --- add_module_on_disk / update_module_order_on_disk tests ---

fn make_simple_module(mode: WriteMode, path: &str) -> ModuleConfig {
    ModuleConfig {
        mode,
        path: path.to_string(),
        display_name: None,
        append_under_header: None,
        append_template: None,
        callout_type: None,
        fields: vec![FieldConfig {
            name: "note".to_string(),
            field_type: FieldType::Text,
            prompt: "Note?".to_string(),
            required: None,
            default: None,
            options: None,
            source: None,
            target: None,
            sub_fields: None,
            callout: None,
        }],
    }
}

#[test]
fn test_add_module_on_disk() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let new_module = make_simple_module(WriteMode::Create, "Tea/log.md");
    Config::add_module_on_disk("tea", &new_module).expect("add_module should succeed");

    let config = Config::load().expect("reload should succeed");
    assert!(
        config.modules.contains_key("tea"),
        "tea module not found after add"
    );
    // Original modules must still be present.
    assert!(
        config.modules.contains_key("journal"),
        "journal module was lost"
    );
    assert!(
        config.modules.contains_key("coffee"),
        "coffee module was lost"
    );

    let tea = &config.modules["tea"];
    assert_eq!(tea.path, "Tea/log.md");
    assert_eq!(tea.mode, WriteMode::Create);
    assert_eq!(tea.fields.len(), 1);
    assert_eq!(tea.fields[0].name, "note");
}

#[test]
fn test_add_module_duplicate_key_rejected() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    // "coffee" already exists in BASE_TOML.
    let duplicate = make_simple_module(WriteMode::Create, "Coffee/other.md");
    let result = Config::add_module_on_disk("coffee", &duplicate);

    assert!(
        matches!(result, Err(ConfigError::DuplicateModule(_))),
        "expected DuplicateModule error, got: {result:?}"
    );
}

#[test]
fn test_add_module_updates_module_order() {
    const WITH_ORDER: &str = r###"
module_order = ["existing"]

[vault]
base_path = "C:/vault"

[modules.existing]
mode = "create"
path = "Existing/log.md"

[[modules.existing.fields]]
name = "note"
field_type = "text"
prompt = "Note?"
"###;

    let (_f, _guard) = write_temp_config(WITH_ORDER);

    let new_module = make_simple_module(WriteMode::Create, "New/log.md");
    Config::add_module_on_disk("new_mod", &new_module).expect("add_module should succeed");

    let config = Config::load().expect("reload should succeed");
    let order = config
        .module_order
        .expect("module_order should still exist");
    assert!(
        order.contains(&"existing".to_string()),
        "existing key missing from module_order"
    );
    assert!(
        order.contains(&"new_mod".to_string()),
        "new_mod not appended to module_order"
    );
    assert_eq!(order[0], "existing", "existing should remain first");
    assert_eq!(order[1], "new_mod", "new_mod should be appended at the end");
}

#[test]
fn test_update_module_order_on_disk() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let new_order = vec!["coffee".to_string(), "journal".to_string()];
    Config::update_module_order_on_disk(&new_order).expect("update_module_order should succeed");

    let config = Config::load().expect("reload should succeed");
    let order = config
        .module_order
        .expect("module_order should have been set");
    assert_eq!(
        order, new_order,
        "persisted order does not match what was written"
    );
}

#[test]
fn test_add_module_preserves_comments() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let new_module = make_simple_module(WriteMode::Create, "Water/log.md");
    Config::add_module_on_disk("water", &new_module).expect("add_module should succeed");

    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");

    assert!(
        written.contains("# Vault configuration"),
        "top-level comment was lost after add_module"
    );
    assert!(
        written.contains("# Modules"),
        "modules comment was lost after add_module"
    );
}

// --- check_paths tests ---

/// Config with a create-mode module whose parent dir doesn't exist on disk.
fn make_check_paths_config(vault_base: &Path, path: &str, mode: &str) -> String {
    let header = if mode == "append" {
        "append_under_header = \"## Log\""
    } else {
        ""
    };
    format!(
        "\n[vault]\nbase_path = \"{vault}\"\n\n[modules.test_mod]\nmode = \"{mode}\"\npath = \"{path}\"\n{header}\n\n[[modules.test_mod.fields]]\nname = \"note\"\nfield_type = \"text\"\nprompt = \"Note?\"\n",
        vault = vault_base.to_str().unwrap().replace('\\', "/"),
        mode = mode,
        path = path,
        header = header,
    )
}

#[test]
fn test_check_paths_missing_parent_dir() {
    let vault_dir = tempfile::tempdir().expect("tempdir");
    // Point to a subdir that doesn't exist.
    let config_str = make_check_paths_config(vault_dir.path(), "NonExistent/note.md", "create");

    let config = Config::from_toml(&config_str).expect("parse should succeed");
    let warnings = config.check_paths(vault_dir.path());

    assert_eq!(
        warnings.len(),
        1,
        "expected exactly one warning, got: {warnings:?}"
    );
    assert!(
        warnings[0].contains("test_mod"),
        "warning should name the module: {}",
        warnings[0]
    );
    assert!(
        warnings[0].contains("parent directory not found"),
        "warning should mention missing parent: {}",
        warnings[0]
    );
}

#[test]
fn test_check_paths_template_path_skipped() {
    let vault_dir = tempfile::tempdir().expect("tempdir");
    // Path contains a template variable — should be skipped.
    let config_str = make_check_paths_config(vault_dir.path(), "Daily/{{date}}.md", "create");

    let config = Config::from_toml(&config_str).expect("parse should succeed");
    let warnings = config.check_paths(vault_dir.path());

    assert!(
        warnings.is_empty(),
        "template paths should not generate warnings, got: {warnings:?}"
    );
}

#[test]
fn test_check_paths_dynamic_source_missing() {
    let vault_dir = tempfile::tempdir().expect("tempdir");

    let config_str = format!(
        r###"
[vault]
base_path = "{vault}"

[modules.beans]
mode = "create"
path = "Beans/log.md"

[[modules.beans.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean?"
source = "Beans/Origins"
"###,
        vault = vault_dir.path().to_str().unwrap().replace('\\', "/"),
    );

    // Create the parent for the module path so only the source warning fires.
    std::fs::create_dir_all(vault_dir.path().join("Beans")).unwrap();

    let config = Config::from_toml(&config_str).expect("parse should succeed");
    let warnings = config.check_paths(vault_dir.path());

    assert_eq!(
        warnings.len(),
        1,
        "expected exactly one warning, got: {warnings:?}"
    );
    assert!(
        warnings[0].contains("source"),
        "warning should mention source: {}",
        warnings[0]
    );
    assert!(
        warnings[0].contains("directory not found"),
        "warning should mention missing directory: {}",
        warnings[0]
    );
}

// --- delete_module_on_disk tests ---

#[test]
fn test_delete_module_on_disk() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    Config::delete_module_on_disk("coffee").expect("delete should succeed");

    let config = Config::load().expect("reload should succeed");
    assert!(
        !config.modules.contains_key("coffee"),
        "coffee should have been deleted"
    );
    assert!(
        config.modules.contains_key("journal"),
        "journal should still be present"
    );
}

#[test]
fn test_delete_last_module_rejected() {
    const ONE_MOD: &str = r###"
[vault]
base_path = "C:/vault"

[modules.solo]
mode = "create"
path = "Solo/log.md"

[[modules.solo.fields]]
name = "note"
field_type = "text"
prompt = "Note?"
"###;

    let (_f, _guard) = write_temp_config(ONE_MOD);

    let result = Config::delete_module_on_disk("solo");
    assert!(
        matches!(result, Err(ConfigError::ValidationError(_))),
        "expected ValidationError when deleting the last module, got: {result:?}"
    );
}

#[test]
fn test_delete_module_removes_from_module_order() {
    const WITH_ORDER: &str = r###"
module_order = ["journal", "coffee"]

[vault]
base_path = "C:/vault"

[modules.journal]
mode = "append"
path = "Journal/daily.md"
append_under_header = "## Log"

[[modules.journal.fields]]
name = "body"
field_type = "textarea"
prompt = "What happened?"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean used?"
"###;

    let (_f, _guard) = write_temp_config(WITH_ORDER);

    Config::delete_module_on_disk("coffee").expect("delete should succeed");

    let config = Config::load().expect("reload should succeed");
    let order = config
        .module_order
        .expect("module_order should still be present");
    assert!(
        !order.contains(&"coffee".to_string()),
        "coffee should have been removed from module_order"
    );
    assert!(
        order.contains(&"journal".to_string()),
        "journal should remain in module_order"
    );
}

// --- reorder_fields_on_disk tests ---

#[test]
fn test_reorder_fields_on_disk() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);

    // FIELD_TOML has fields: [0] method, [1] notes
    // Swap: new_order = [1, 0]  =>  notes first, method second.
    Config::reorder_fields_on_disk("coffee", &[1, 0]).expect("reorder should succeed");

    let config = Config::load().expect("reload should succeed");
    let fields = &config.modules["coffee"].fields;

    assert_eq!(fields.len(), 2, "field count should be unchanged");
    assert_eq!(fields[0].name, "notes", "notes should now be first");
    assert_eq!(fields[1].name, "method", "method should now be second");
}

#[test]
fn test_reorder_fields_invalid_permutation() {
    let (_f, _guard) = write_temp_config(FIELD_TOML);
    let config_path = std::env::var("POUR_CONFIG").unwrap();
    let original = std::fs::read_to_string(&config_path).unwrap();

    // Wrong length.
    let result = Config::reorder_fields_on_disk("coffee", &[0]);
    assert!(
        matches!(result, Err(ConfigError::ValidationError(_))),
        "wrong length should return ValidationError, got: {result:?}"
    );

    // Duplicate index.
    let result = Config::reorder_fields_on_disk("coffee", &[0, 0]);
    assert!(
        matches!(result, Err(ConfigError::ValidationError(_))),
        "duplicate index should return ValidationError, got: {result:?}"
    );

    // File must be unchanged.
    let after = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(
        original, after,
        "file should not be modified on invalid permutation"
    );
}
