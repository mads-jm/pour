//! One-shot argv grammar — `pour <module> <field> [value]` (spec §5).
//!
//! The binary itself is not integration-testable (integration tests link the
//! library, not `src/main.rs`), and extending the harness to spawn a process
//! was out of scope for this cycle. `main.rs` holds only the four-line match
//! that routes into these two functions, so everything with behaviour in it is
//! covered here.

use pour::config::Config;
use pour::oneshot::{OneShot, OneShotError, parse, run};
use pour::transport::Transport;
use pour::transport::fs::FsWriter;

const DAILY_NOTE: &str =
    "---\ndate: 2026-08-05\ncannabis: false\nwater: null\n---\n\n# 20260805\n\nbody\n";

const TOML: &str = r####"
[vault]
base_path = "/replaced/at/runtime"
date_format = "%Y%m%d"

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

[[modules.habit.fields]]
name = "note"
field_type = "text"
prompt = "Note"

[modules.coffee]
mode = "create"
path = "Coffee/%Y%m%d.md"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
"####;

fn config_for(base: &str) -> Config {
    Config::from_toml(&TOML.replace("/replaced/at/runtime", &base.replace('\\', "\\\\")))
        .expect("fixture config must validate")
}

fn args(rest: &[&str]) -> Vec<String> {
    std::iter::once("pour".to_string())
        .chain(rest.iter().map(|s| s.to_string()))
        .collect()
}

/// The compatibility guarantee: `pour <module>` still opens the TUI.
#[test]
fn no_field_argument_falls_through_to_the_tui() {
    let config = config_for("/tmp/vault");
    assert_eq!(parse(&args(&["habit"]), &config), Ok(None));
    assert_eq!(parse(&args(&[]), &config), Ok(None));
    // Even an unknown module defers — `main.rs` owns that error on the TUI path.
    assert_eq!(parse(&args(&["nope"]), &config), Ok(None));
}

#[test]
fn a_bare_toggle_field_means_true() {
    let config = config_for("/tmp/vault");
    assert_eq!(
        parse(&args(&["habit", "cannabis"]), &config),
        Ok(Some(OneShot {
            module_key: "habit".to_string(),
            field_name: "cannabis".to_string(),
            value: "true".to_string(),
        }))
    );
}

#[test]
fn false_and_off_are_the_correction_path() {
    let config = config_for("/tmp/vault");
    for token in ["false", "off"] {
        let shot = parse(&args(&["habit", "cannabis", token]), &config)
            .unwrap()
            .unwrap();
        assert_eq!(shot.value, "false", "{token}");
    }
}

#[test]
fn counter_tokens_pass_through_verbatim() {
    let config = config_for("/tmp/vault");
    let shot = parse(&args(&["habit", "water", "16"]), &config)
        .unwrap()
        .unwrap();
    assert_eq!(shot.value, "16");

    let shot = parse(&args(&["habit", "water", "=160"]), &config)
        .unwrap()
        .unwrap();
    assert_eq!(shot.value, "=160");
}

#[test]
fn unknown_module_and_unknown_field_are_typed_errors() {
    let config = config_for("/tmp/vault");

    assert!(matches!(
        parse(&args(&["nope", "water"]), &config),
        Err(OneShotError::UnknownModule { .. })
    ));
    assert!(matches!(
        parse(&args(&["habit", "steps"]), &config),
        Err(OneShotError::UnknownField { .. })
    ));
}

#[test]
fn one_shot_is_wired_for_update_modules_and_two_field_types_only() {
    let config = config_for("/tmp/vault");

    assert!(matches!(
        parse(&args(&["coffee", "bean", "Onyx"]), &config),
        Err(OneShotError::NotUpdateMode { .. })
    ));
    assert!(matches!(
        parse(&args(&["habit", "note", "hello"]), &config),
        Err(OneShotError::UnsupportedFieldType {
            field_type: "text",
            ..
        })
    ));
}

#[test]
fn a_counter_without_a_value_is_rejected_before_the_vault_is_touched() {
    let config = config_for("/tmp/vault");
    assert!(matches!(
        parse(&args(&["habit", "water"]), &config),
        Err(OneShotError::MissingValue { .. })
    ));
    assert!(matches!(
        parse(&args(&["habit", "water", "lots"]), &config),
        Err(OneShotError::BadValue { .. })
    ));
    assert!(matches!(
        parse(&args(&["habit", "cannabis", "maybe"]), &config),
        Err(OneShotError::BadValue { .. })
    ));
}

#[test]
fn arguments_past_the_value_are_rejected_not_ignored() {
    let config = config_for("/tmp/vault");
    assert!(matches!(
        parse(
            &args(&["habit", "water", "16", "--date", "yesterday"]),
            &config
        ),
        Err(OneShotError::UnexpectedArgs { .. })
    ));
}

#[test]
fn every_error_message_names_something_actionable() {
    let config = config_for("/tmp/vault");
    let err = parse(&args(&["habit", "steps"]), &config).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("steps"), "{msg}");
    assert!(
        msg.contains("cannabis"),
        "must list what is available: {msg}"
    );
}

// ─── End to end over a temp vault ────────────────────────────────────────────

fn seeded() -> (tempfile::TempDir, Config, Transport) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("daily")).unwrap();
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    std::fs::write(
        dir.path().join(format!("daily/{today}.md")),
        DAILY_NOTE.replace("2026-08-05", &today),
    )
    .unwrap();
    let config = config_for(&dir.path().to_string_lossy());
    let transport = Transport::Fs(FsWriter::new(dir.path().to_path_buf()));
    (dir, config, transport)
}

#[tokio::test]
async fn run_writes_and_echoes_the_resulting_state() {
    let (dir, config, transport) = seeded();
    let today = chrono::Local::now().format("%Y%m%d").to_string();

    let shot = parse(&args(&["habit", "water", "16"]), &config)
        .unwrap()
        .unwrap();
    let line = run(&config, &transport, &shot).await.unwrap();

    assert_eq!(line, format!("water: 16/96 oz · ✓ {today}.md"));
    let after = std::fs::read_to_string(dir.path().join(format!("daily/{today}.md"))).unwrap();
    assert!(after.contains("water: 16\n"), "{after}");
    assert!(after.contains("cannabis: false\n"), "{after}");
}

#[tokio::test]
async fn run_on_a_missing_note_reports_rather_than_creating_it() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_for(&dir.path().to_string_lossy());
    let transport = Transport::Fs(FsWriter::new(dir.path().to_path_buf()));

    let shot = parse(&args(&["habit", "cannabis"]), &config)
        .unwrap()
        .unwrap();
    let err = run(&config, &transport, &shot).await.unwrap_err();

    assert!(err.to_string().contains("doesn't exist yet"), "{err}");
    assert!(!dir.path().join("daily").exists());
}
