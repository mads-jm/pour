use pour::config::{Config, WriteMode};
use pour::init::generate_config;

#[test]
fn generated_config_is_valid_toml() {
    let content = generate_config("/tmp/test-vault");
    let config = Config::from_toml(&content).expect("generated config should parse and validate");
    assert_eq!(config.vault.base_path, "/tmp/test-vault");
    assert!(config.modules.contains_key("me"));
    assert!(config.modules.contains_key("todo"));
    assert!(config.modules.contains_key("note"));
    assert!(config.modules.contains_key("coffee"));
}

#[test]
fn windows_path_escaping() {
    let content = generate_config(r"C:\Users\Joseph\vault");
    let config = Config::from_toml(&content).expect("windows path should be escaped correctly");
    assert_eq!(config.vault.base_path, r"C:\Users\Joseph\vault");
}

#[test]
fn vault_path_with_quotes() {
    let content = generate_config(r#"C:\Users\My "Vault"\notes"#);
    let config = Config::from_toml(&content).expect("quoted path should be escaped correctly");
    assert_eq!(config.vault.base_path, r#"C:\Users\My "Vault"\notes"#);
}

#[test]
fn default_modules_structure() {
    let content = generate_config("/vault");
    let config = Config::from_toml(&content).unwrap();

    let me = &config.modules["me"];
    assert_eq!(me.mode, WriteMode::Append);
    assert!(me.append_under_header.is_some());
    assert_eq!(me.fields.len(), 2);

    let todo = &config.modules["todo"];
    assert_eq!(todo.mode, WriteMode::Append);
    assert!(todo.append_under_header.is_some());
    assert_eq!(todo.fields.len(), 1);

    let note = &config.modules["note"];
    assert_eq!(note.mode, WriteMode::Create);
    assert_eq!(note.fields.len(), 2);

    let coffee = &config.modules["coffee"];
    assert_eq!(coffee.mode, WriteMode::Create);
    assert_eq!(coffee.fields.len(), 19);
}
