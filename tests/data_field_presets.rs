use pour::data::field_presets::{FieldPresetEntry, FieldPresets, preset_key, reconcile_rows};
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn rows(values: &[&[&str]]) -> Vec<Vec<String>> {
    values
        .iter()
        .map(|row| row.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

fn make_entry(name: &str, row_data: &[&[&str]]) -> FieldPresetEntry {
    FieldPresetEntry {
        name: name.to_owned(),
        description: None,
        rows: rows(row_data),
    }
}

fn tmp_presets() -> (FieldPresets, NamedTempFile) {
    let file = NamedTempFile::new().expect("tempfile");
    let path = file.path().to_path_buf();
    let presets = FieldPresets::load_from(path);
    (presets, file)
}

// ---------------------------------------------------------------------------
// Missing / corrupt file handling
// ---------------------------------------------------------------------------

#[test]
fn missing_file_returns_empty_defaults() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.json");
    let presets = FieldPresets::load_from(path);
    assert!(presets.get("coffee.recipe").is_empty());
}

#[test]
fn corrupt_file_returns_empty_defaults() {
    let file = NamedTempFile::new().expect("tempfile");
    std::fs::write(file.path(), b"{ not valid json !!!").expect("write");
    let presets = FieldPresets::load_from(file.path().to_path_buf());
    assert!(presets.get("coffee.recipe").is_empty());
}

// ---------------------------------------------------------------------------
// preset_key
// ---------------------------------------------------------------------------

#[test]
fn preset_key_joins_module_and_field() {
    assert_eq!(preset_key("coffee", "recipe"), "coffee.recipe");
    assert_eq!(
        preset_key("coffee", "pressure_profile"),
        "coffee.pressure_profile"
    );
}

// ---------------------------------------------------------------------------
// set / get
// ---------------------------------------------------------------------------

#[test]
fn set_then_get_returns_entry() {
    let (mut presets, _file) = tmp_presets();
    let entry = make_entry(
        "Hoffmann 4:6",
        &[&["Bloom", "60", "30"], &["First Pour", "120", "30"]],
    );
    presets.set("coffee.recipe", entry);

    let list = presets.get("coffee.recipe");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Hoffmann 4:6");
    assert_eq!(list[0].rows.len(), 2);
    assert_eq!(list[0].rows[0], vec!["Bloom", "60", "30"]);
}

#[test]
fn set_same_name_overwrites_not_duplicates() {
    let (mut presets, _file) = tmp_presets();
    presets.set(
        "coffee.recipe",
        make_entry("Standard", &[&["Bloom", "60", "30"]]),
    );
    presets.set(
        "coffee.recipe",
        make_entry("Standard", &[&["Bloom", "80", "45"]]),
    );

    let list = presets.get("coffee.recipe");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].rows[0], vec!["Bloom", "80", "45"]);
}

#[test]
fn set_different_names_appends() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee.recipe", make_entry("A", &[&["Bloom", "60", "30"]]));
    presets.set("coffee.recipe", make_entry("B", &[&["Bloom", "80", "30"]]));

    let names: Vec<_> = presets
        .get("coffee.recipe")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["A", "B"]);
}

#[test]
fn distinct_keys_dont_collide() {
    let (mut presets, _file) = tmp_presets();
    presets.set(
        "coffee.recipe",
        make_entry("Same", &[&["Bloom", "60", "30"]]),
    );
    presets.set(
        "coffee.pressure_profile",
        make_entry("Same", &[&["9.0", "20"]]),
    );

    assert_eq!(presets.get("coffee.recipe").len(), 1);
    assert_eq!(presets.get("coffee.pressure_profile").len(), 1);
    // Different sub-field shapes preserved.
    assert_eq!(presets.get("coffee.recipe")[0].rows[0].len(), 3);
    assert_eq!(presets.get("coffee.pressure_profile")[0].rows[0].len(), 2);
}

// ---------------------------------------------------------------------------
// Round-trip
// ---------------------------------------------------------------------------

#[test]
fn round_trip_save_and_load() {
    let file = NamedTempFile::new().expect("tempfile");
    let path = file.path().to_path_buf();

    let mut presets = FieldPresets::load_from(path.clone());
    presets.set(
        "coffee.recipe",
        make_entry(
            "Hoffmann 4:6",
            &[
                &["Bloom", "60", "30"],
                &["First Pour", "120", "30"],
                &["Draw Down", "240", "30"],
            ],
        ),
    );
    presets.set(
        "coffee.pressure_profile",
        make_entry("9-bar standard", &[&["9.0", "25"]]),
    );
    presets.save().expect("save");

    let loaded = FieldPresets::load_from(path);
    let recipe = loaded.get("coffee.recipe");
    assert_eq!(recipe.len(), 1);
    assert_eq!(recipe[0].rows.len(), 3);
    assert_eq!(recipe[0].rows[2], vec!["Draw Down", "240", "30"]);

    let pressure = loaded.get("coffee.pressure_profile");
    assert_eq!(pressure.len(), 1);
    assert_eq!(pressure[0].rows[0], vec!["9.0", "25"]);
}

// ---------------------------------------------------------------------------
// delete
// ---------------------------------------------------------------------------

#[test]
fn delete_removes_entry_and_returns_true() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee.recipe", make_entry("A", &[&["Bloom", "60", "30"]]));
    presets.set("coffee.recipe", make_entry("B", &[&["Bloom", "80", "30"]]));

    assert!(presets.delete("coffee.recipe", "A"));
    let list = presets.get("coffee.recipe");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "B");
}

#[test]
fn delete_nonexistent_returns_false() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee.recipe", make_entry("A", &[&["Bloom", "60", "30"]]));
    assert!(!presets.delete("coffee.recipe", "Ghost"));
    assert!(!presets.delete("coffee.unknown_field", "anything"));
}

// ---------------------------------------------------------------------------
// reorder
// ---------------------------------------------------------------------------

#[test]
fn reorder_forward_and_backward() {
    let (mut presets, _file) = tmp_presets();
    for n in ["A", "B", "C"] {
        presets.set("coffee.recipe", make_entry(n, &[&["Bloom", "60", "30"]]));
    }
    presets.reorder("coffee.recipe", "A", 1);
    let names: Vec<_> = presets
        .get("coffee.recipe")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["B", "A", "C"]);

    presets.reorder("coffee.recipe", "C", -1);
    let names: Vec<_> = presets
        .get("coffee.recipe")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["B", "C", "A"]);
}

#[test]
fn reorder_at_boundary_is_noop() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee.recipe", make_entry("A", &[&["Bloom", "60", "30"]]));
    presets.set("coffee.recipe", make_entry("B", &[&["Bloom", "80", "30"]]));

    presets.reorder("coffee.recipe", "A", -1);
    presets.reorder("coffee.recipe", "B", 1);
    let names: Vec<_> = presets
        .get("coffee.recipe")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["A", "B"]);
}

#[test]
fn reorder_unknown_targets_are_noop() {
    let (mut presets, _file) = tmp_presets();
    presets.set("coffee.recipe", make_entry("A", &[&["Bloom", "60", "30"]]));
    presets.reorder("coffee.recipe", "Ghost", 1);
    presets.reorder("nonexistent.key", "A", 1);
    presets.reorder("coffee.recipe", "A", 0);

    let names: Vec<_> = presets
        .get("coffee.recipe")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["A"]);
}

// ---------------------------------------------------------------------------
// description round-trip + omission
// ---------------------------------------------------------------------------

#[test]
fn description_round_trips() {
    let file = NamedTempFile::new().expect("tempfile");
    let path = file.path().to_path_buf();
    let mut presets = FieldPresets::load_from(path.clone());

    presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "Hoffmann 4:6".to_owned(),
            description: Some("light roast, 1:15 ratio".to_owned()),
            rows: rows(&[&["Bloom", "60", "30"]]),
        },
    );
    presets.save().expect("save");

    let loaded = FieldPresets::load_from(path);
    let entry = &loaded.get("coffee.recipe")[0];
    assert_eq!(
        entry.description.as_deref(),
        Some("light roast, 1:15 ratio")
    );
}

#[test]
fn save_omits_description_when_none() {
    let file = NamedTempFile::new().expect("tempfile");
    let path = file.path().to_path_buf();
    let mut presets = FieldPresets::load_from(path.clone());
    presets.set(
        "coffee.recipe",
        make_entry("Plain", &[&["Bloom", "60", "30"]]),
    );
    presets.save().expect("save");

    let contents = std::fs::read_to_string(&path).expect("read");
    assert!(
        !contents.contains("description"),
        "description must be omitted when None; got: {contents}"
    );
}

// ---------------------------------------------------------------------------
// reconcile_rows — schema-drift handling
// ---------------------------------------------------------------------------

#[test]
fn reconcile_matching_shape_is_unchanged() {
    let input = rows(&[&["Bloom", "60", "30"], &["First Pour", "120", "30"]]);
    let (out, adjusted) = reconcile_rows(input.clone(), 3);
    assert!(!adjusted);
    assert_eq!(out, input);
}

#[test]
fn reconcile_truncates_extra_cells() {
    // Saved with 4 cells, current schema has 3.
    let input = rows(&[&["Bloom", "60", "30", "extra"]]);
    let (out, adjusted) = reconcile_rows(input, 3);
    assert!(adjusted);
    assert_eq!(out, rows(&[&["Bloom", "60", "30"]]));
}

#[test]
fn reconcile_pads_missing_cells() {
    // Saved with 2 cells, current schema has 3.
    let input = rows(&[&["Bloom", "60"]]);
    let (out, adjusted) = reconcile_rows(input, 3);
    assert!(adjusted);
    assert_eq!(out, rows(&[&["Bloom", "60", ""]]));
}

#[test]
fn reconcile_empty_rows_to_zero_subfields() {
    let input = rows(&[&["Bloom", "60", "30"]]);
    let (out, adjusted) = reconcile_rows(input, 0);
    assert!(adjusted);
    assert_eq!(out, vec![Vec::<String>::new()]);
}
