//! `update` mode: value tokens, counter arithmetic, and the write itself.

use std::collections::HashMap;

use pour::config::Config;
use pour::output::update::{CounterOp, ValueParseError, parse_counter_token, parse_toggle_token};
use pour::output::write_update;
use pour::transport::Transport;
use pour::transport::api::ApiClient;
use pour::transport::fs::FsWriter;

/// A daily note shaped like the template that owns these keys: `cannabis`
/// defaults to `false`, `water` to `null` ("untouched today").
const DAILY_NOTE: &str = "---\ndate: 2026-08-05\ntags:\n  - daily\ncannabis: false\nwater: null\n---\n\n# 20260805\n\nbody\n";

const HABIT_TOML: &str = r####"
[vault]
base_path = "/replaced/at/runtime"
date_format = "%Y%m%d"

[modules.habit]
mode = "update"
path = "daily/%Y%m%d.md"
display_name = "Habits"

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

fn now() -> chrono::DateTime<chrono::Local> {
    use chrono::TimeZone;
    chrono::Local
        .with_ymd_and_hms(2026, 8, 5, 9, 30, 0)
        .unwrap()
}

/// A config + temp vault with today's note already seeded.
fn fixture(note: Option<&str>) -> (tempfile::TempDir, Config) {
    let dir = tempfile::tempdir().unwrap();
    if let Some(body) = note {
        std::fs::create_dir_all(dir.path().join("daily")).unwrap();
        std::fs::write(dir.path().join("daily/20260805.md"), body).unwrap();
    }
    let toml = HABIT_TOML.replace(
        "/replaced/at/runtime",
        &dir.path().to_string_lossy().replace('\\', "\\\\"),
    );
    let config = Config::from_toml(&toml).expect("fixture config must validate");
    (dir, config)
}

fn fs_transport(dir: &tempfile::TempDir) -> Transport {
    Transport::Fs(FsWriter::new(dir.path().to_path_buf()))
}

async fn capture(
    dir: &tempfile::TempDir,
    config: &Config,
    transport: &Transport,
    values: &[(&str, &str)],
) -> anyhow::Result<pour::output::UpdateOutcome> {
    let field_values: HashMap<String, String> = values
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    write_update(
        transport,
        &config.modules["habit"],
        &field_values,
        config.vault.date_format.as_deref(),
        &dir.path().to_string_lossy(),
        now(),
    )
    .await
}

fn note_after(dir: &tempfile::TempDir) -> String {
    std::fs::read_to_string(dir.path().join("daily/20260805.md")).unwrap()
}

// ─── Value tokens ────────────────────────────────────────────────────────────

#[test]
fn counter_tokens_increment_by_default_and_set_with_an_equals() {
    assert_eq!(parse_counter_token("16"), Ok(CounterOp::Increment(16.0)));
    assert_eq!(
        parse_counter_token(" 12.5 "),
        Ok(CounterOp::Increment(12.5))
    );
    assert_eq!(parse_counter_token("-8"), Ok(CounterOp::Increment(-8.0)));
    assert_eq!(parse_counter_token("=160"), Ok(CounterOp::Set(160.0)));
    assert_eq!(parse_counter_token("= 0"), Ok(CounterOp::Set(0.0)));
}

#[test]
fn counter_rejects_tokens_that_are_not_numbers() {
    assert_eq!(
        parse_counter_token("lots"),
        Err(ValueParseError::NotANumber("lots".to_string()))
    );
    assert_eq!(
        parse_counter_token(""),
        Err(ValueParseError::NotANumber("".to_string()))
    );
    assert!(parse_counter_token("inf").is_err(), "no infinite counters");
}

#[test]
fn toggle_tokens_accept_the_correction_words() {
    for token in ["true", "on", "yes", "1", "TRUE"] {
        assert_eq!(parse_toggle_token(token), Ok(true), "{token}");
    }
    for token in ["false", "off", "no", "0", "Off"] {
        assert_eq!(parse_toggle_token(token), Ok(false), "{token}");
    }
    assert_eq!(
        parse_toggle_token("maybe"),
        Err(ValueParseError::NotABoolean("maybe".to_string()))
    );
}

// ─── The write ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn counter_increments_from_null_as_if_it_were_zero() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);

    let outcome = capture(&dir, &config, &transport, &[("water", "16")])
        .await
        .expect("capture must land");

    assert_eq!(outcome.vault_path, "daily/20260805.md");
    assert_eq!(outcome.echoes, vec!["water: 16/96 oz".to_string()]);
    assert_eq!(
        note_after(&dir),
        DAILY_NOTE.replace("water: null", "water: 16")
    );
}

#[tokio::test]
async fn counter_accumulates_across_captures() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);

    capture(&dir, &config, &transport, &[("water", "16")])
        .await
        .unwrap();
    let outcome = capture(&dir, &config, &transport, &[("water", "48")])
        .await
        .unwrap();

    assert_eq!(outcome.echoes, vec!["water: 64/96 oz".to_string()]);
    assert_eq!(
        note_after(&dir),
        DAILY_NOTE.replace("water: null", "water: 64")
    );
}

#[tokio::test]
async fn equals_sets_rather_than_adds() {
    let (dir, config) = fixture(Some(&DAILY_NOTE.replace("water: null", "water: 640")));
    let transport = fs_transport(&dir);

    let outcome = capture(&dir, &config, &transport, &[("water", "=64")])
        .await
        .unwrap();

    assert_eq!(outcome.echoes, vec!["water: 64/96 oz".to_string()]);
    assert!(note_after(&dir).contains("water: 64\n"));
}

#[tokio::test]
async fn floats_are_allowed_and_integral_results_stay_bare() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);

    capture(&dir, &config, &transport, &[("water", "12.5")])
        .await
        .unwrap();
    assert!(note_after(&dir).contains("water: 12.5\n"));

    let outcome = capture(&dir, &config, &transport, &[("water", "12.5")])
        .await
        .unwrap();
    assert_eq!(outcome.echoes, vec!["water: 25/96 oz".to_string()]);
    assert!(
        note_after(&dir).contains("water: 25\n"),
        "an integral result must not gain a decimal point"
    );
}

#[tokio::test]
async fn toggle_writes_a_bare_boolean() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);

    let outcome = capture(&dir, &config, &transport, &[("cannabis", "true")])
        .await
        .unwrap();

    assert_eq!(outcome.echoes, vec!["cannabis: true".to_string()]);
    assert_eq!(
        note_after(&dir),
        DAILY_NOTE.replace("cannabis: false", "cannabis: true")
    );
}

#[tokio::test]
async fn only_the_named_keys_change() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);

    capture(&dir, &config, &transport, &[("water", "8")])
        .await
        .unwrap();

    let after = note_after(&dir);
    assert!(after.contains("date: 2026-08-05\n"), "{after}");
    assert!(after.contains("tags:\n  - daily\n"), "{after}");
    assert!(after.contains("cannabis: false\n"), "{after}");
    assert!(after.ends_with("---\n\n# 20260805\n\nbody\n"), "{after}");
}

#[tokio::test]
async fn a_blank_value_is_no_change_not_zero() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);

    let outcome = capture(
        &dir,
        &config,
        &transport,
        &[("water", "16"), ("cannabis", "")],
    )
    .await;
    let outcome = outcome.unwrap();

    assert_eq!(outcome.echoes, vec!["water: 16/96 oz".to_string()]);
    assert!(
        note_after(&dir).contains("cannabis: false\n"),
        "a blank field must not be written"
    );
}

#[tokio::test]
async fn two_live_fields_both_land_in_module_field_order() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);

    let outcome = capture(
        &dir,
        &config,
        &transport,
        &[("cannabis", "true"), ("water", "16")],
    )
    .await
    .expect("a two-field submission must write both keys");

    assert_eq!(
        outcome.echoes,
        vec!["cannabis: true".to_string(), "water: 16/96 oz".to_string()],
        "echoes follow module field order, not HashMap order"
    );
    assert_eq!(
        note_after(&dir),
        DAILY_NOTE
            .replace("cannabis: false", "cannabis: true")
            .replace("water: null", "water: 16"),
        "both keys change and nothing else does"
    );
}

#[tokio::test]
async fn a_failure_on_the_second_key_reports_that_the_first_already_landed() {
    // `water` is a block sequence in this note, so the patcher refuses it — a
    // deterministic stand-in for any mid-loop transport failure (a concurrent
    // Obsidian save landing between the two patches behaves the same way).
    // Multi-key updates are not atomic; the contract is that the error says so.
    let note = DAILY_NOTE.replace("water: null", "water:\n  - 16");
    let (dir, config) = fixture(Some(&note));
    let transport = fs_transport(&dir);

    let err = capture(
        &dir,
        &config,
        &transport,
        &[("cannabis", "true"), ("water", "16")],
    )
    .await
    .expect_err("the second key cannot be patched");

    let msg = err.to_string();
    assert!(
        msg.contains("failed to set 'water'"),
        "the failing key must be named, got: {msg}"
    );
    assert!(
        msg.contains("'cannabis'") && msg.contains("not rolled back"),
        "the key that already landed must be reported, got: {msg}"
    );

    let after = note_after(&dir);
    assert!(
        after.contains("cannabis: true\n"),
        "the first patch really did land: {after}"
    );
    assert!(
        after.contains("water:\n  - 16\n"),
        "the refused key is untouched: {after}"
    );
}

#[tokio::test]
async fn a_missing_key_is_inserted_and_reported_as_a_stale_template() {
    let stale = "---\ndate: 2026-08-05\n---\n\nbody\n";
    let (dir, config) = fixture(Some(stale));
    let transport = fs_transport(&dir);

    let outcome = capture(&dir, &config, &transport, &[("water", "16")])
        .await
        .expect("a stale template must not block the capture");

    assert_eq!(outcome.inserted_keys, vec!["water".to_string()]);
    assert!(
        outcome
            .notices()
            .iter()
            .any(|n| n.contains("template is stale")),
        "got {:?}",
        outcome.notices()
    );
    assert_eq!(
        note_after(&dir),
        "---\ndate: 2026-08-05\nwater: 16\n---\n\nbody\n"
    );
}

#[tokio::test]
async fn a_missing_note_fails_loudly_and_is_never_fabricated() {
    let (dir, config) = fixture(None);
    let transport = fs_transport(&dir);

    let err = capture(&dir, &config, &transport, &[("water", "16")])
        .await
        .expect_err("the fs path must not create the note");

    assert!(
        err.to_string().contains("doesn't exist yet"),
        "message must be actionable, got: {err}"
    );
    assert!(!dir.path().join("daily/20260805.md").exists());
}

#[tokio::test]
async fn a_note_without_frontmatter_fails_rather_than_restructuring_it() {
    let (dir, config) = fixture(Some("# 20260805\n\nbody\n"));
    let transport = fs_transport(&dir);

    let err = capture(&dir, &config, &transport, &[("water", "16")])
        .await
        .expect_err("pour never adds a frontmatter block");

    assert!(
        err.to_string().contains("no frontmatter block"),
        "got: {err}"
    );
    assert_eq!(note_after(&dir), "# 20260805\n\nbody\n");
}

#[tokio::test]
async fn a_non_numeric_current_value_is_an_error_not_a_silent_reset() {
    let (dir, config) = fixture(Some(&DAILY_NOTE.replace("water: null", "water: lots")));
    let transport = fs_transport(&dir);

    let err = capture(&dir, &config, &transport, &[("water", "16")])
        .await
        .expect_err("incrementing prose would destroy it");

    assert!(err.to_string().contains("cannot increment"), "got: {err}");
    assert!(note_after(&dir).contains("water: lots"));
}

#[tokio::test]
async fn write_update_refuses_a_non_update_module() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);
    let mut module = config.modules["habit"].clone();
    module.mode = pour::config::WriteMode::Create;

    let err = write_update(
        &transport,
        &module,
        &HashMap::from([("water".to_string(), "1".to_string())]),
        None,
        &dir.path().to_string_lossy(),
        now(),
    )
    .await
    .expect_err("mode guard must hold");
    assert!(err.to_string().contains("non-update module"), "got: {err}");
}

#[tokio::test]
async fn an_api_that_cannot_serve_the_patch_degrades_to_the_filesystem() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    // Port 19877 is closed: every request fails to connect, which is how an
    // API that has gone away — or a plugin that cannot serve the operation —
    // reaches `write_update`. Either way the capture must land on disk.
    let transport = Transport::Api(ApiClient::new(19877, "unused".to_string()).unwrap());

    let outcome = capture(&dir, &config, &transport, &[("water", "16")])
        .await
        .expect("degradation must keep the capture alive");

    assert!(outcome.degraded, "the degrade must be observable");
    assert!(
        outcome
            .notices()
            .iter()
            .any(|n| n.contains("wrote to the file directly")),
        "got {:?}",
        outcome.notices()
    );
    assert_eq!(
        note_after(&dir),
        DAILY_NOTE.replace("water: null", "water: 16")
    );
}

#[tokio::test]
async fn echo_line_is_the_one_shot_confirmation_shape() {
    let (dir, config) = fixture(Some(DAILY_NOTE));
    let transport = fs_transport(&dir);

    let outcome = capture(&dir, &config, &transport, &[("water", "64")])
        .await
        .unwrap();

    assert_eq!(outcome.echo_line(), "water: 64/96 oz · ✓ 20260805.md");
}
