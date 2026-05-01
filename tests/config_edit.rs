use pour::config::{Config, ConfigError};
use std::io::Write;
use std::sync::Mutex;
use tempfile::NamedTempFile;

/// Serialise tests that mutate the `POUR_CONFIG` env var.
static ENV_LOCK: Mutex<()> = Mutex::new(());

const BASE_TOML: &str = r###"
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

/// Round-trip: edit() mutates a key; re-parsing the file confirms the mutation landed.
#[test]
fn edit_round_trip_mutation_persists() {
    let (f, _guard) = write_temp_config(BASE_TOML);

    Config::edit(f.path(), |draft| {
        draft.doc["vault"]["base_path"] = toml_edit::value("C:/new_vault");
        Ok(())
    })
    .expect("edit should succeed");

    let written = std::fs::read_to_string(f.path()).expect("file should be readable");
    let reparsed = Config::from_toml(&written).expect("re-parse should succeed");
    assert_eq!(
        reparsed.vault.base_path, "C:/new_vault",
        "mutation should be visible in re-parsed config"
    );
}

/// Closure error rolls back: if the closure returns Err, the file on disk is unchanged.
#[test]
fn edit_closure_error_leaves_file_unchanged() {
    let (f, _guard) = write_temp_config(BASE_TOML);
    let original = std::fs::read_to_string(f.path()).expect("read original");

    let result = Config::edit(f.path(), |draft| {
        // Mutate the doc inside the closure...
        draft.doc["vault"]["base_path"] = toml_edit::value("C:/mutated");
        // ...then return an error.
        Err(ConfigError::ModuleNotFound("nonexistent".to_string()))
    });

    assert!(result.is_err(), "edit should propagate the closure error");

    let after = std::fs::read_to_string(f.path()).expect("read after");
    assert_eq!(
        original, after,
        "file contents must be unchanged after closure error"
    );
    // Confirm no orphan .toml.tmp was left behind.
    let tmp_path = f.path().with_extension("toml.tmp");
    assert!(
        !tmp_path.exists(),
        "orphan .toml.tmp must not exist after rollback"
    );
}

/// Closure can read draft.parsed: snapshot reflects the on-disk state before edits.
#[test]
fn edit_draft_parsed_reflects_disk_state() {
    let (f, _guard) = write_temp_config(BASE_TOML);

    let mut saw_journal = false;
    let mut saw_coffee = false;

    Config::edit(f.path(), |draft| {
        saw_journal = draft.parsed.modules.contains_key("journal");
        saw_coffee = draft.parsed.modules.contains_key("coffee");
        Ok(())
    })
    .expect("edit should succeed");

    assert!(
        saw_journal,
        "draft.parsed should contain the 'journal' module"
    );
    assert!(
        saw_coffee,
        "draft.parsed should contain the 'coffee' module"
    );
}

/// Closure error on an invalid TOML mutation (via from_toml validation) rolls back.
#[test]
fn edit_validation_error_does_not_write() {
    let (f, _guard) = write_temp_config(BASE_TOML);
    let original = std::fs::read_to_string(f.path()).expect("read original");

    let result = Config::edit(f.path(), |draft| {
        // Remove vault.base_path — this will fail Config::from_toml validation.
        draft
            .doc
            .get_mut("vault")
            .and_then(|v| v.as_table_mut())
            .map(|t| t.remove("base_path"));
        // Manually validate to surface the error inside the closure.
        Config::from_toml(&draft.doc.to_string())?;
        Ok(())
    });

    assert!(result.is_err(), "removing base_path should error");

    let after = std::fs::read_to_string(f.path()).expect("read after");
    assert_eq!(
        original, after,
        "file must be unchanged after validation failure"
    );
}

/// No-op closure: edit with no mutations returns Ok and file is unchanged.
#[test]
fn edit_noop_closure_preserves_file() {
    let (f, _guard) = write_temp_config(BASE_TOML);
    let original = std::fs::read_to_string(f.path()).expect("read original");

    Config::edit(f.path(), |_draft| Ok(())).expect("no-op edit should succeed");

    // The file is rewritten atomically even on no-op, but content should parse
    // to the same config (whitespace/formatting may be re-serialized by toml_edit).
    let after = std::fs::read_to_string(f.path()).expect("read after");
    let reparsed = Config::from_toml(&after).expect("re-parse should succeed");
    let original_parsed = Config::from_toml(&original).expect("original parse should succeed");
    assert_eq!(
        reparsed.vault.base_path, original_parsed.vault.base_path,
        "vault base_path must be unchanged"
    );
    assert_eq!(
        reparsed.modules.len(),
        original_parsed.modules.len(),
        "module count must be unchanged"
    );
}
