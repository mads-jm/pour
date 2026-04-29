// Unit tests for `Presets` extension methods added in Step D.
//
// Tests the `module_presets`, `api_set`, `api_remove`, and `api_reorder`
// methods without going through the HTTP layer.

use std::collections::HashMap;

use pour::data::presets::{Presets, ReorderError, SetResult};

fn presets_in_tmp(dir: &tempfile::TempDir) -> Presets {
    let path = dir.path().join("presets.json");
    Presets::load_from(path)
}

fn vals(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// module_presets
// ---------------------------------------------------------------------------

#[test]
fn module_presets_none_when_module_not_present() {
    let tmp = tempfile::tempdir().unwrap();
    let p = presets_in_tmp(&tmp);
    assert!(p.module_presets("coffee").is_none());
}

#[test]
fn module_presets_empty_slice_after_add_and_remove_all() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Alpha", None, vals(&[("bean", "Onyx")]))
        .unwrap();
    p.api_remove("coffee", "Alpha").unwrap();

    // Module key now exists (entry was created then removed); slice should be empty.
    let presets = p.module_presets("coffee").unwrap_or(&[]);
    assert!(presets.is_empty());
}

// ---------------------------------------------------------------------------
// api_set — Created vs Updated
// ---------------------------------------------------------------------------

#[test]
fn api_set_new_preset_returns_created() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    let result = p
        .api_set(
            "coffee",
            "Morning",
            Some("desc".to_string()),
            vals(&[("bean", "Onyx")]),
        )
        .unwrap();
    assert_eq!(result, SetResult::Created);
}

#[test]
fn api_set_existing_preset_returns_updated() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Morning", None, vals(&[("bean", "Onyx")]))
        .unwrap();
    let result = p
        .api_set(
            "coffee",
            "Morning",
            Some("new desc".to_string()),
            vals(&[("bean", "Onyx2")]),
        )
        .unwrap();
    assert_eq!(result, SetResult::Updated);
}

#[test]
fn api_set_update_preserves_position() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Alpha", None, vals(&[])).unwrap();
    p.api_set("coffee", "Beta", None, vals(&[])).unwrap();
    p.api_set(
        "coffee",
        "Alpha",
        Some("updated".to_string()),
        vals(&[("x", "y")]),
    )
    .unwrap();

    let list = p.module_presets("coffee").unwrap();
    assert_eq!(list.len(), 2);
    // Alpha must still be first (position preserved).
    assert_eq!(list[0].name, "Alpha");
    assert_eq!(list[1].name, "Beta");
    // Description updated.
    assert_eq!(list[0].description.as_deref(), Some("updated"));
}

#[test]
fn api_set_persists_to_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("presets.json");

    {
        let mut p = Presets::load_from(path.clone());
        p.api_set("coffee", "Morning", None, vals(&[("bean", "Onyx")]))
            .unwrap();
    }

    // Reload from disk.
    let p2 = Presets::load_from(path);
    let list = p2.module_presets("coffee").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Morning");
    assert_eq!(list[0].values["bean"], "Onyx");
}

// ---------------------------------------------------------------------------
// api_remove
// ---------------------------------------------------------------------------

#[test]
fn api_remove_returns_true_when_found() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Morning", None, vals(&[])).unwrap();
    let removed = p.api_remove("coffee", "Morning").unwrap();
    assert!(removed);
}

#[test]
fn api_remove_returns_false_when_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    let removed = p.api_remove("coffee", "Nonexistent").unwrap();
    assert!(!removed);
}

#[test]
fn api_remove_removes_correct_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Alpha", None, vals(&[])).unwrap();
    p.api_set("coffee", "Beta", None, vals(&[])).unwrap();
    p.api_remove("coffee", "Alpha").unwrap();

    let list = p.module_presets("coffee").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Beta");
}

// ---------------------------------------------------------------------------
// api_reorder
// ---------------------------------------------------------------------------

#[test]
fn api_reorder_happy_path() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Alpha", None, vals(&[])).unwrap();
    p.api_set("coffee", "Beta", None, vals(&[])).unwrap();
    p.api_set("coffee", "Gamma", None, vals(&[])).unwrap();

    p.api_reorder(
        "coffee",
        vec!["Gamma".to_string(), "Alpha".to_string(), "Beta".to_string()],
    )
    .unwrap();

    let list = p.module_presets("coffee").unwrap();
    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["Gamma", "Alpha", "Beta"]);
}

#[test]
fn api_reorder_missing_names_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Alpha", None, vals(&[])).unwrap();
    p.api_set("coffee", "Beta", None, vals(&[])).unwrap();

    let err = p
        .api_reorder("coffee", vec!["Alpha".to_string()])
        .unwrap_err();

    match err {
        ReorderError::MissingNames(missing) => {
            assert!(
                missing.contains(&"Beta".to_string()),
                "missing should contain Beta; got {missing:?}"
            );
        }
        other => panic!("expected MissingNames, got {other:?}"),
    }
}

#[test]
fn api_reorder_extra_names_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Alpha", None, vals(&[])).unwrap();

    let err = p
        .api_reorder("coffee", vec!["Alpha".to_string(), "Extra".to_string()])
        .unwrap_err();

    match err {
        ReorderError::ExtraNames(extra) => {
            assert!(
                extra.contains(&"Extra".to_string()),
                "extra should contain Extra; got {extra:?}"
            );
        }
        other => panic!("expected ExtraNames, got {other:?}"),
    }
}

#[test]
fn api_reorder_empty_list_on_empty_module_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    // No presets for "coffee" yet — reordering with empty list is valid (no-op).
    p.api_reorder("coffee", vec![]).unwrap();
    let list = p.module_presets("coffee").unwrap_or(&[]);
    assert!(list.is_empty());
}

#[test]
fn api_reorder_duplicate_names_returns_error() {
    // Regression test for MAJOR #3: ["Alpha","Beta","Beta"] against ["Alpha","Beta"]
    // previously passed set-diff validation (sets are equal) and silently dropped
    // the second Beta. DuplicateNames is now checked before the set-diff.
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Alpha", None, vals(&[])).unwrap();
    p.api_set("coffee", "Beta", None, vals(&[])).unwrap();

    let err = p
        .api_reorder(
            "coffee",
            vec!["Alpha".to_string(), "Beta".to_string(), "Beta".to_string()],
        )
        .unwrap_err();

    match err {
        ReorderError::DuplicateNames(dups) => {
            assert!(
                dups.contains(&"Beta".to_string()),
                "duplicates should contain Beta; got {dups:?}"
            );
        }
        other => panic!("expected DuplicateNames, got {other:?}"),
    }
}

#[test]
fn api_reorder_duplicate_does_not_mutate_list() {
    // Even if the caller crafted a request that could pass set-diff, the list
    // must remain unchanged when DuplicateNames is returned.
    let tmp = tempfile::tempdir().unwrap();
    let mut p = presets_in_tmp(&tmp);

    p.api_set("coffee", "Alpha", None, vals(&[])).unwrap();
    p.api_set("coffee", "Beta", None, vals(&[])).unwrap();

    let _ = p.api_reorder(
        "coffee",
        vec!["Alpha".to_string(), "Beta".to_string(), "Beta".to_string()],
    );

    // List should still be [Alpha, Beta] in original order.
    let list = p.module_presets("coffee").unwrap();
    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "Beta"]);
}
