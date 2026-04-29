// Unit tests for mobile_visible config schema field.

use std::io::Write as _;
use std::sync::Mutex;

use pour::config::{Config, ModuleUpdates};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn write_temp_config(
    content: &str,
) -> (tempfile::NamedTempFile, std::sync::MutexGuard<'static, ()>) {
    let guard = ENV_LOCK.lock().unwrap();
    let mut f = tempfile::NamedTempFile::new().expect("failed to create temp file");
    f.write_all(content.as_bytes())
        .expect("failed to write temp config");
    f.flush().expect("failed to flush temp config");
    // SAFETY: guarded by ENV_LOCK so only one thread holds this at a time.
    unsafe { std::env::set_var("POUR_CONFIG", f.path().to_str().unwrap()) };
    (f, guard)
}

const BASE_TOML: &str = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.coffee]
mode = "create"
path = "Coffee/%Y%m%d.md"

[[modules.coffee.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"#;

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[test]
fn mobile_visible_parses_from_toml() {
    let toml = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.hidden]
mode = "create"
path = "Hidden/%Y%m%d.md"
mobile_visible = false

[[modules.hidden.fields]]
name = "note"
field_type = "text"
prompt = "Note"
"#;
    let config = Config::from_toml(toml).expect("should parse");
    assert_eq!(config.modules["hidden"].mobile_visible, Some(false));
}

#[test]
fn mobile_visible_absent_is_none() {
    let config = Config::from_toml(BASE_TOML).expect("should parse");
    assert_eq!(config.modules["coffee"].mobile_visible, None);
}

#[test]
fn is_mobile_visible_returns_true_when_absent() {
    let config = Config::from_toml(BASE_TOML).expect("should parse");
    assert!(config.modules["coffee"].is_mobile_visible());
}

#[test]
fn is_mobile_visible_returns_true_when_explicit_true() {
    let toml = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.coffee]
mode = "create"
path = "Coffee/%Y%m%d.md"
mobile_visible = true

[[modules.coffee.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"#;
    let config = Config::from_toml(toml).expect("should parse");
    assert!(config.modules["coffee"].is_mobile_visible());
}

#[test]
fn is_mobile_visible_returns_false_when_explicit_false() {
    let toml = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.hidden]
mode = "create"
path = "Hidden/%Y%m%d.md"
mobile_visible = false

[[modules.hidden.fields]]
name = "note"
field_type = "text"
prompt = "Note"
"#;
    let config = Config::from_toml(toml).expect("should parse");
    assert!(!config.modules["hidden"].is_mobile_visible());
}

// ---------------------------------------------------------------------------
// Round-trip through update_module_on_disk
// ---------------------------------------------------------------------------

#[test]
fn mobile_visible_false_round_trips() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let updates = ModuleUpdates {
        path: None,
        display_name: None,
        mode: None,
        append_under_header: None,
        callout_type: None,
        icon: None,
        daily_link: None,
        append_shallow: None,
        mobile_visible: Some(Some(false)),
    };

    Config::update_module_on_disk("coffee", &updates).expect("update should succeed");

    let config = Config::load().expect("should reload");
    assert_eq!(config.modules["coffee"].mobile_visible, Some(false));
    assert!(!config.modules["coffee"].is_mobile_visible());

    // Verify it's actually in the file
    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");
    assert!(
        written.contains("mobile_visible = false"),
        "mobile_visible = false must be persisted"
    );
}

#[test]
fn mobile_visible_true_round_trips() {
    let (_f, _guard) = write_temp_config(BASE_TOML);

    let updates = ModuleUpdates {
        path: None,
        display_name: None,
        mode: None,
        append_under_header: None,
        callout_type: None,
        icon: None,
        daily_link: None,
        append_shallow: None,
        mobile_visible: Some(Some(true)),
    };

    Config::update_module_on_disk("coffee", &updates).expect("update should succeed");

    let config = Config::load().expect("should reload");
    assert_eq!(config.modules["coffee"].mobile_visible, Some(true));
    assert!(config.modules["coffee"].is_mobile_visible());

    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");
    assert!(
        written.contains("mobile_visible = true"),
        "mobile_visible = true must be persisted"
    );
}

#[test]
fn mobile_visible_none_removes_key() {
    // Start with mobile_visible = false in the file
    let toml_with_hidden = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.coffee]
mode = "create"
path = "Coffee/%Y%m%d.md"
mobile_visible = false

[[modules.coffee.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"#;
    let (_f, _guard) = write_temp_config(toml_with_hidden);

    let updates = ModuleUpdates {
        path: None,
        display_name: None,
        mode: None,
        append_under_header: None,
        callout_type: None,
        icon: None,
        daily_link: None,
        append_shallow: None,
        mobile_visible: Some(None), // remove the key
    };

    Config::update_module_on_disk("coffee", &updates).expect("update should succeed");

    let config = Config::load().expect("should reload");
    // Removed → None → defaults to true
    assert_eq!(config.modules["coffee"].mobile_visible, None);
    assert!(config.modules["coffee"].is_mobile_visible());

    let written = std::fs::read_to_string(std::env::var("POUR_CONFIG").unwrap())
        .expect("failed to read config");
    assert!(
        !written.contains("mobile_visible"),
        "mobile_visible key should have been removed"
    );
}
