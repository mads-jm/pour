//! A config stowed out of a dotfiles repo must survive being edited.
//!
//! `~/.pour/config.toml` is commonly a symlink into a tracked directory (stow,
//! chezmoi, a bare repo). Pour's config writes end in `rename(2)`, which swaps
//! a directory entry — so writing onto the link replaced it with a regular
//! file, stranded the edit outside version control, and left the tracked file
//! stale. These tests drive the real public editing API over that layout.
//!
//! Unix-only: the symlink layout under test needs `std::os::unix`, and gating
//! the whole file keeps the shared helpers from reading as dead on Windows.
#![cfg(unix)]

use pour::config::Config;
use std::sync::Mutex;
use tempfile::tempdir;

/// Serialise tests that mutate the `POUR_CONFIG` env var.
static ENV_LOCK: Mutex<()> = Mutex::new(());

const BASE_TOML: &str = r###"
[vault]
base_path = "C:/vault"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"

[[modules.coffee.fields]]
name = "origin"
field_type = "static_select"
prompt = "Origin?"
options = ["Ethiopia", "Colombia"]
"###;

/// Build the stow layout in a tempdir: a tracked `repo/config.toml` and a
/// `home/config.toml` symlink pointing at it. Points `POUR_CONFIG` at the link.
fn stowed_config() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    std::sync::MutexGuard<'static, ()>,
) {
    use std::os::unix::fs::symlink;

    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().expect("failed to create tempdir");

    let repo = dir.path().join("repo");
    let home = dir.path().join("home");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(&home).unwrap();

    let tracked = repo.join("config.toml");
    let link = home.join("config.toml");
    std::fs::write(&tracked, BASE_TOML).unwrap();
    symlink(&tracked, &link).unwrap();

    // SAFETY: guarded by ENV_LOCK so only one thread holds this at a time.
    unsafe { std::env::set_var("POUR_CONFIG", link.to_str().unwrap()) };

    (dir, tracked, link, guard)
}

/// The reported bug: adding a bean origin through the TUI broke the symlink.
#[test]
fn appending_an_option_keeps_the_symlink_and_updates_the_tracked_file() {
    let (_dir, tracked, link, _guard) = stowed_config();

    Config::append_option_to_field_on_disk("coffee", 0, "Honduras")
        .expect("appending an option should succeed");

    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the stow link must survive a config write"
    );

    let tracked_text = std::fs::read_to_string(&tracked).unwrap();
    assert!(
        tracked_text.contains("Honduras"),
        "the edit must land in the tracked file, not beside the link:\n{tracked_text}"
    );

    let reparsed = Config::from_toml(&tracked_text).expect("tracked file should still parse");
    let options = reparsed.modules["coffee"].fields[0]
        .options
        .as_ref()
        .expect("origin field should have options");
    assert_eq!(
        options,
        &vec![
            "Ethiopia".to_string(),
            "Colombia".to_string(),
            "Honduras".to_string()
        ],
        "the option should be appended, preserving the existing entries"
    );
}

/// Repeated edits must not degrade the link — the second write goes through the
/// same resolution as the first.
#[test]
fn successive_edits_keep_the_symlink() {
    let (_dir, tracked, link, _guard) = stowed_config();

    Config::append_option_to_field_on_disk("coffee", 0, "Honduras").unwrap();
    Config::append_option_to_field_on_disk("coffee", 0, "Rwanda").unwrap();

    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the link must survive more than one write"
    );

    let tracked_text = std::fs::read_to_string(&tracked).unwrap();
    assert!(
        tracked_text.contains("Honduras") && tracked_text.contains("Rwanda"),
        "both edits must reach the tracked file:\n{tracked_text}"
    );
}

/// No orphan `config.toml.tmp` may be left next to the link. The temp file
/// belongs beside the resolved target, and the rename consumes it.
#[test]
fn no_temp_file_is_orphaned_beside_the_link_or_the_target() {
    let (_dir, tracked, link, _guard) = stowed_config();

    Config::append_option_to_field_on_disk("coffee", 0, "Honduras").unwrap();

    let beside_link = link.with_extension("toml.tmp");
    let beside_target = tracked.with_extension("toml.tmp");
    assert!(
        !beside_link.exists(),
        "a temp file was orphaned next to the link at {}",
        beside_link.display()
    );
    assert!(
        !beside_target.exists(),
        "a temp file was orphaned next to the tracked file at {}",
        beside_target.display()
    );
}
