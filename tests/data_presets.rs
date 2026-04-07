use pour::data::presets::{PresetEntry, Presets};
use std::collections::HashMap;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_entry(name: &str, values: &[(&str, &str)]) -> PresetEntry {
    PresetEntry {
        name: name.to_owned(),
        values: values
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn tmp_presets() -> (Presets, NamedTempFile) {
    let file = NamedTempFile::new().expect("tempfile");
    // NamedTempFile creates a zero-byte file. load_from will fail to parse
    // the empty string and fall back to defaults — same as corrupt-file path.
    let path = file.path().to_path_buf();
    let presets = Presets::load_from(path);
    (presets, file)
}

// ---------------------------------------------------------------------------
// Missing / corrupt file handling
// ---------------------------------------------------------------------------

#[test]
fn missing_file_returns_empty_defaults() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.json");
    let presets = Presets::load_from(path);
    assert!(presets.get("any_module").is_empty());
}

#[test]
fn corrupt_file_returns_empty_defaults() {
    let file = NamedTempFile::new().expect("tempfile");
    std::fs::write(file.path(), b"{ not valid json !!!").expect("write");
    let presets = Presets::load_from(file.path().to_path_buf());
    assert!(presets.get("coffee").is_empty());
}

// ---------------------------------------------------------------------------
// get() for absent module
// ---------------------------------------------------------------------------

#[test]
fn get_nonexistent_module_returns_empty_vec() {
    let (presets, _file) = tmp_presets();
    let result = presets.get("no_such_module");
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// set() / get()
// ---------------------------------------------------------------------------

#[test]
fn set_then_get_returns_entry() {
    let (mut presets, _file) = tmp_presets();
    let entry = make_entry("Morning", &[("bean", "Ethiopia"), ("dose", "18")]);
    presets.set("coffee", entry);

    let list = presets.get("coffee");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Morning");
    assert_eq!(
        list[0].values.get("bean").map(String::as_str),
        Some("Ethiopia")
    );
    assert_eq!(list[0].values.get("dose").map(String::as_str), Some("18"));
}

#[test]
fn set_same_name_overwrites_not_duplicates() {
    let (mut presets, _file) = tmp_presets();

    presets.set("coffee", make_entry("Morning", &[("bean", "Ethiopia")]));
    presets.set("coffee", make_entry("Morning", &[("bean", "Kenya")])); // overwrite

    let list = presets.get("coffee");
    assert_eq!(list.len(), 1, "should not duplicate on same name");
    assert_eq!(
        list[0].values.get("bean").map(String::as_str),
        Some("Kenya")
    );
}

#[test]
fn set_different_names_appends() {
    let (mut presets, _file) = tmp_presets();

    presets.set("coffee", make_entry("Morning", &[("bean", "Ethiopia")]));
    presets.set("coffee", make_entry("Afternoon", &[("bean", "Kenya")]));

    let list = presets.get("coffee");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name, "Morning");
    assert_eq!(list[1].name, "Afternoon");
}

// ---------------------------------------------------------------------------
// Round-trip (save + load_from)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_save_and_load() {
    let file = NamedTempFile::new().expect("tempfile");
    let path = file.path().to_path_buf();

    let mut presets = Presets::load_from(path.clone());
    presets.set(
        "coffee",
        make_entry("Morning", &[("bean", "Ethiopia"), ("dose", "18")]),
    );
    presets.set(
        "coffee",
        make_entry("Afternoon", &[("bean", "Kenya"), ("dose", "16")]),
    );
    presets.set("me", make_entry("Baseline", &[("mood", "good")]));
    presets.save().expect("save");

    let loaded = Presets::load_from(path);
    let coffee = loaded.get("coffee");
    assert_eq!(coffee.len(), 2);
    assert_eq!(coffee[0].name, "Morning");
    assert_eq!(coffee[1].name, "Afternoon");
    assert_eq!(coffee[0].values["bean"], "Ethiopia");

    let me = loaded.get("me");
    assert_eq!(me.len(), 1);
    assert_eq!(me[0].name, "Baseline");
}

// ---------------------------------------------------------------------------
// delete()
// ---------------------------------------------------------------------------

#[test]
fn delete_removes_entry_and_returns_true() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("Morning", &[]));
    presets.set("coffee", make_entry("Afternoon", &[]));

    let removed = presets.delete("coffee", "Morning");
    assert!(removed, "should return true when preset existed");

    let list = presets.get("coffee");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Afternoon");
}

#[test]
fn delete_nonexistent_preset_returns_false() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("Morning", &[]));

    let removed = presets.delete("coffee", "Ghost");
    assert!(!removed, "should return false when preset not found");
    assert_eq!(presets.get("coffee").len(), 1, "list unchanged");
}

#[test]
fn delete_on_absent_module_returns_false() {
    let (mut presets, _file) = tmp_presets();
    let removed = presets.delete("no_such_module", "anything");
    assert!(!removed);
}

#[test]
fn second_delete_returns_false() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("Morning", &[]));

    assert!(presets.delete("coffee", "Morning"));
    assert!(
        !presets.delete("coffee", "Morning"),
        "second delete should return false"
    );
}

// ---------------------------------------------------------------------------
// reorder()
// ---------------------------------------------------------------------------

#[test]
fn reorder_forward_moves_preset_toward_end() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("A", &[]));
    presets.set("coffee", make_entry("B", &[]));
    presets.set("coffee", make_entry("C", &[]));

    presets.reorder("coffee", "A", 1); // A moves from index 0 -> 1

    let names: Vec<_> = presets.get("coffee").into_iter().map(|p| p.name).collect();
    assert_eq!(names, vec!["B", "A", "C"]);
}

#[test]
fn reorder_backward_moves_preset_toward_start() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("A", &[]));
    presets.set("coffee", make_entry("B", &[]));
    presets.set("coffee", make_entry("C", &[]));

    presets.reorder("coffee", "C", -1); // C moves from index 2 -> 1

    let names: Vec<_> = presets.get("coffee").into_iter().map(|p| p.name).collect();
    assert_eq!(names, vec!["A", "C", "B"]);
}

#[test]
fn reorder_forward_at_end_is_noop() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("A", &[]));
    presets.set("coffee", make_entry("B", &[]));

    presets.reorder("coffee", "B", 1); // already at end

    let names: Vec<_> = presets.get("coffee").into_iter().map(|p| p.name).collect();
    assert_eq!(names, vec!["A", "B"]);
}

#[test]
fn reorder_backward_at_start_is_noop() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("A", &[]));
    presets.set("coffee", make_entry("B", &[]));

    presets.reorder("coffee", "A", -1); // already at start

    let names: Vec<_> = presets.get("coffee").into_iter().map(|p| p.name).collect();
    assert_eq!(names, vec!["A", "B"]);
}

#[test]
fn reorder_nonexistent_preset_is_noop() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("A", &[]));
    presets.set("coffee", make_entry("B", &[]));

    presets.reorder("coffee", "Ghost", 1); // no-op

    let names: Vec<_> = presets.get("coffee").into_iter().map(|p| p.name).collect();
    assert_eq!(names, vec!["A", "B"]);
}

#[test]
fn reorder_absent_module_is_noop() {
    let (mut presets, _file) = tmp_presets();
    // Should not panic
    presets.reorder("no_such_module", "A", 1);
}

#[test]
fn reorder_direction_zero_is_noop() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee", make_entry("A", &[]));
    presets.set("coffee", make_entry("B", &[]));

    presets.reorder("coffee", "A", 0);

    let names: Vec<_> = presets.get("coffee").into_iter().map(|p| p.name).collect();
    assert_eq!(names, vec!["A", "B"]);
}
