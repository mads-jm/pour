use std::collections::{HashMap, HashSet};

use pour::config::{FieldConfig, FieldType, ShowWhen};
use pour::visibility::{is_field_visible, visible_field_indices};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unconditional(name: &str) -> FieldConfig {
    FieldConfig {
        name: name.to_string(),
        field_type: FieldType::Text,
        prompt: name.to_string(),
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        sub_fields: None,
        callout: None,
        allow_create: None,
        wikilink: None,
        create_template: None,
        post_create_command: None,
        show_when: None,
    }
}

fn with_equals(name: &str, controlling_field: &str, equals: &str) -> FieldConfig {
    FieldConfig {
        name: name.to_string(),
        field_type: FieldType::Text,
        prompt: name.to_string(),
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        sub_fields: None,
        callout: None,
        allow_create: None,
        wikilink: None,
        create_template: None,
        post_create_command: None,
        show_when: Some(ShowWhen {
            field: controlling_field.to_string(),
            equals: Some(equals.to_string()),
            one_of: None,
        }),
    }
}

fn with_one_of(name: &str, controlling_field: &str, values: &[&str]) -> FieldConfig {
    FieldConfig {
        name: name.to_string(),
        field_type: FieldType::Text,
        prompt: name.to_string(),
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        sub_fields: None,
        callout: None,
        allow_create: None,
        wikilink: None,
        create_template: None,
        post_create_command: None,
        show_when: Some(ShowWhen {
            field: controlling_field.to_string(),
            equals: None,
            one_of: Some(values.iter().map(|s| s.to_string()).collect()),
        }),
    }
}

fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// is_field_visible — unconditional
// ---------------------------------------------------------------------------

#[test]
fn unconditional_field_is_always_visible() {
    let field = unconditional("notes");
    assert!(is_field_visible(&field, &HashMap::new()));
    assert!(is_field_visible(&field, &values(&[("anything", "value")])));
}

// ---------------------------------------------------------------------------
// is_field_visible — equals variant
// ---------------------------------------------------------------------------

#[test]
fn equals_match_is_visible() {
    let field = with_equals("detail", "brew_method", "Espresso");
    let fv = values(&[("brew_method", "Espresso")]);
    assert!(is_field_visible(&field, &fv));
}

#[test]
fn equals_mismatch_is_hidden() {
    let field = with_equals("detail", "brew_method", "Espresso");
    let fv = values(&[("brew_method", "V60")]);
    assert!(!is_field_visible(&field, &fv));
}

#[test]
fn equals_is_case_sensitive() {
    let field = with_equals("detail", "brew_method", "Espresso");
    let fv = values(&[("brew_method", "espresso")]);
    assert!(!is_field_visible(&field, &fv));
}

// ---------------------------------------------------------------------------
// is_field_visible — one_of variant
// ---------------------------------------------------------------------------

#[test]
fn one_of_matching_value_is_visible() {
    let field = with_one_of("grind", "brew_method", &["V60", "AeroPress"]);
    let fv = values(&[("brew_method", "AeroPress")]);
    assert!(is_field_visible(&field, &fv));
}

#[test]
fn one_of_non_matching_value_is_hidden() {
    let field = with_one_of("grind", "brew_method", &["V60", "AeroPress"]);
    let fv = values(&[("brew_method", "Espresso")]);
    assert!(!is_field_visible(&field, &fv));
}

#[test]
fn one_of_is_case_sensitive() {
    let field = with_one_of("grind", "brew_method", &["V60", "AeroPress"]);
    let fv = values(&[("brew_method", "aeropress")]);
    assert!(!is_field_visible(&field, &fv));
}

// ---------------------------------------------------------------------------
// is_field_visible — missing / empty controlling field
// ---------------------------------------------------------------------------

#[test]
fn referenced_field_absent_is_hidden() {
    let field = with_equals("detail", "brew_method", "Espresso");
    assert!(!is_field_visible(&field, &HashMap::new()));
}

#[test]
fn referenced_field_empty_string_is_hidden() {
    let field = with_equals("detail", "brew_method", "Espresso");
    let fv = values(&[("brew_method", "")]);
    assert!(!is_field_visible(&field, &fv));
}

#[test]
fn one_of_referenced_field_absent_is_hidden() {
    let field = with_one_of("grind", "brew_method", &["V60", "AeroPress"]);
    assert!(!is_field_visible(&field, &HashMap::new()));
}

#[test]
fn one_of_referenced_field_empty_string_is_hidden() {
    let field = with_one_of("grind", "brew_method", &["V60", "AeroPress"]);
    let fv = values(&[("brew_method", "")]);
    assert!(!is_field_visible(&field, &fv));
}

// ---------------------------------------------------------------------------
// visible_field_indices
// ---------------------------------------------------------------------------

#[test]
fn visible_field_indices_returns_correct_subset() {
    // fields: [0] unconditional, [1] equals/match, [2] unconditional, [3] equals/no-match
    // Expected visible: [0, 1, 2]
    let fields = vec![
        unconditional("f0"),
        with_equals("f1", "brew_method", "V60"),
        unconditional("f2"),
        with_equals("f3", "brew_method", "Espresso"),
    ];
    let fv = values(&[("brew_method", "V60")]);
    assert_eq!(visible_field_indices(&fields, &fv), vec![0, 1, 2]);
}

#[test]
fn visible_field_indices_all_visible() {
    let fields = vec![unconditional("a"), unconditional("b"), unconditional("c")];
    assert_eq!(
        visible_field_indices(&fields, &HashMap::new()),
        vec![0, 1, 2]
    );
}

#[test]
fn visible_field_indices_none_visible() {
    let fields = vec![
        with_equals("a", "x", "yes"),
        with_equals("b", "x", "yes"),
    ];
    let fv = values(&[("x", "no")]);
    assert_eq!(visible_field_indices(&fields, &fv), Vec::<usize>::new());
}

#[test]
fn visible_field_indices_empty_fields_slice() {
    assert_eq!(
        visible_field_indices(&[], &HashMap::new()),
        Vec::<usize>::new()
    );
}

// ---------------------------------------------------------------------------
// submit-time stripping — mirrors the handle_submit logic in main.rs:
//   visible_indices = visible_field_indices(fields, &field_values)
//   visible_names   = visible_indices.map(|i| fields[i].name)
//   field_values.retain(|k, _| visible_names.contains(k))
// ---------------------------------------------------------------------------

/// Helper: apply the submit-time strip to a mutable field_values map.
fn strip_hidden(fields: &[FieldConfig], mut field_values: HashMap<String, String>) -> HashMap<String, String> {
    let visible_names: HashSet<String> = visible_field_indices(fields, &field_values)
        .into_iter()
        .map(|i| fields[i].name.clone())
        .collect();
    field_values.retain(|k, _| visible_names.contains(k));
    field_values
}

/// Hidden field value is removed; unconditional and visible conditional fields survive.
#[test]
fn submit_strip_removes_hidden_equals_field() {
    // fields: brew_method (unconditional), grind (show_when brew_method=V60)
    let fields = vec![
        unconditional("brew_method"),
        with_equals("grind", "brew_method", "V60"),
    ];
    // brew_method = "Espresso" → grind is hidden
    let fv = values(&[
        ("brew_method", "Espresso"),
        ("grind", "coarse"),          // stale value from a previous selection
    ]);
    let result = strip_hidden(&fields, fv);
    assert!(result.contains_key("brew_method"), "unconditional field should survive");
    assert!(!result.contains_key("grind"), "hidden field should be stripped");
}

/// Visible conditional field is retained when condition is met.
#[test]
fn submit_strip_keeps_visible_conditional_field() {
    let fields = vec![
        unconditional("brew_method"),
        with_equals("grind", "brew_method", "V60"),
    ];
    // brew_method = "V60" → grind is visible
    let fv = values(&[
        ("brew_method", "V60"),
        ("grind", "medium-fine"),
    ]);
    let result = strip_hidden(&fields, fv);
    assert_eq!(result["brew_method"], "V60");
    assert_eq!(result["grind"], "medium-fine");
}

/// Optional unconditional field left empty survives (not hidden, just blank).
#[test]
fn submit_strip_preserves_empty_optional_field() {
    let fields = vec![
        unconditional("brew_method"),
        unconditional("notes"),       // optional — user left it blank
    ];
    let fv = values(&[
        ("brew_method", "V60"),
        ("notes", ""),
    ]);
    let result = strip_hidden(&fields, fv);
    assert!(result.contains_key("notes"), "empty optional field must not be stripped");
    assert_eq!(result["notes"], "");
}

/// one_of variant: hidden field stripped when controlling value is outside the list.
#[test]
fn submit_strip_removes_hidden_one_of_field() {
    let fields = vec![
        unconditional("brew_method"),
        with_one_of("pressure", "brew_method", &["Espresso", "Moka"]),
    ];
    // brew_method = "V60" → pressure is hidden
    let fv = values(&[
        ("brew_method", "V60"),
        ("pressure", "9 bar"),
    ]);
    let result = strip_hidden(&fields, fv);
    assert!(!result.contains_key("pressure"), "hidden one_of field should be stripped");
}

/// All fields hidden → result is empty.
#[test]
fn submit_strip_all_hidden_yields_empty_map() {
    let fields = vec![
        with_equals("a", "x", "yes"),
        with_equals("b", "x", "yes"),
    ];
    let fv = values(&[
        ("x", "no"),
        ("a", "stale-a"),
        ("b", "stale-b"),
    ]);
    let result = strip_hidden(&fields, fv);
    // "x" is not in fields so it was never visible_field_indices; also stripped
    assert!(result.is_empty(), "all fields hidden (or not in module) should yield empty map, got: {result:?}");
}

/// Controlling field itself (unconditional) is preserved even when it makes others hidden.
#[test]
fn submit_strip_controlling_field_always_kept() {
    let fields = vec![
        unconditional("kind"),
        with_equals("detail", "kind", "special"),
    ];
    let fv = values(&[
        ("kind", "normal"),
        ("detail", "secret"),
    ]);
    let result = strip_hidden(&fields, fv);
    assert_eq!(result.get("kind"), Some(&"normal".to_string()));
    assert!(!result.contains_key("detail"));
}
