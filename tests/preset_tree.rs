use pour::config::Config;
use pour::data::preset_tree::{AxisError, TreeNode, build, suggest_preset_name, validate_axes};
use pour::data::presets::PresetEntry;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const AXES_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"
preset_axes = ["method", "bean"]

[[modules.coffee.fields]]
name = "method"
field_type = "static_select"
prompt = "Method"
options = ["V60", "AeroPress"]

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"

[[modules.coffee.fields]]
name = "notes"
field_type = "text"
prompt = "Notes"

[[modules.coffee.fields]]
name = "timestamp"
field_type = "text"
prompt = "Timestamp"
preset_exclude = true

[[modules.coffee.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Recipe"

[[modules.coffee.fields.sub_fields]]
name = "amount"
field_type = "number"
prompt = "Amount (g)"
"####;

fn make_preset(name: &str, pairs: &[(&str, &str)]) -> PresetEntry {
    PresetEntry {
        name: name.to_owned(),
        description: None,
        values: pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Case 1: three-level build (method → bean → leaves)
// ---------------------------------------------------------------------------

#[test]
fn build_three_level_tree() {
    let presets = vec![
        make_preset("V60 Onyx 1", &[("method", "V60"), ("bean", "Onyx")]),
        make_preset("V60 Onyx 2", &[("method", "V60"), ("bean", "Onyx")]),
        make_preset("V60 Kenya", &[("method", "V60"), ("bean", "Kenya")]),
        make_preset("Aero Onyx", &[("method", "AeroPress"), ("bean", "Onyx")]),
    ];
    let axes = ["method".to_owned(), "bean".to_owned()];
    let tree = build(&presets, &axes);

    assert_eq!(tree.ungrouped.len(), 0);
    // Branches sorted alphabetically: AeroPress < V60
    assert_eq!(tree.roots.len(), 2);
    let first_name = match &tree.roots[0] {
        TreeNode::Branch { axis_value, .. } => axis_value.clone(),
        _ => panic!("expected branch"),
    };
    assert_eq!(first_name, "AeroPress");
}

// ---------------------------------------------------------------------------
// Case 2: missing axis value → ungrouped
// ---------------------------------------------------------------------------

#[test]
fn missing_axis_value_goes_to_ungrouped() {
    let presets = vec![
        make_preset("Good", &[("method", "V60"), ("bean", "Onyx")]),
        // bean is missing
        make_preset("No Bean", &[("method", "V60")]),
        // bean is empty string
        make_preset("Empty Bean", &[("method", "V60"), ("bean", "")]),
    ];
    let axes = ["method".to_owned(), "bean".to_owned()];
    let tree = build(&presets, &axes);

    assert_eq!(tree.ungrouped.len(), 2);
    assert_eq!(tree.roots.len(), 1);
}

// ---------------------------------------------------------------------------
// Case 3: empty axes → all ungrouped, roots empty
// ---------------------------------------------------------------------------

#[test]
fn empty_axes_all_ungrouped() {
    let presets = vec![
        make_preset("A", &[("method", "V60"), ("bean", "Onyx")]),
        make_preset("B", &[("method", "AeroPress"), ("bean", "Kenya")]),
    ];
    let tree = build(&presets, &[]);

    assert_eq!(tree.roots.len(), 0);
    assert_eq!(tree.ungrouped.len(), 2);
}

// ---------------------------------------------------------------------------
// Case 4: validate_axes rejects unknown field name
// ---------------------------------------------------------------------------

#[test]
fn validate_axes_rejects_unknown_field() {
    let config = Config::from_toml(AXES_TOML).expect("parse");
    let fields = &config.modules["coffee"].fields;
    let axes = vec!["method".to_owned(), "ghost_field".to_owned()];
    let result = validate_axes(&axes, fields);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.contains(&AxisError::UnknownField("ghost_field".to_owned())));
}

// ---------------------------------------------------------------------------
// Case 5: validate_axes rejects composite_array field
// ---------------------------------------------------------------------------

#[test]
fn validate_axes_rejects_composite_array() {
    let config = Config::from_toml(AXES_TOML).expect("parse");
    let fields = &config.modules["coffee"].fields;
    let axes = vec!["recipe".to_owned()];
    let result = validate_axes(&axes, fields);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.contains(&AxisError::CompositeArrayField("recipe".to_owned())));
}

// ---------------------------------------------------------------------------
// Case 6: validate_axes rejects preset_exclude field
// ---------------------------------------------------------------------------

#[test]
fn validate_axes_rejects_preset_excluded_field() {
    let config = Config::from_toml(AXES_TOML).expect("parse");
    let fields = &config.modules["coffee"].fields;
    let axes = vec!["timestamp".to_owned()];
    let result = validate_axes(&axes, fields);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.contains(&AxisError::PresetExcludedField("timestamp".to_owned())));
}

// ---------------------------------------------------------------------------
// Case 7: branch count aggregates correctly
// ---------------------------------------------------------------------------

#[test]
fn branch_count_aggregates() {
    let presets = vec![
        make_preset("P1", &[("method", "V60"), ("bean", "Onyx")]),
        make_preset("P2", &[("method", "V60"), ("bean", "Kenya")]),
        make_preset("P3", &[("method", "V60"), ("bean", "Onyx")]),
    ];
    let axes = ["method".to_owned(), "bean".to_owned()];
    let tree = build(&presets, &axes);

    match &tree.roots[0] {
        TreeNode::Branch { count, .. } => assert_eq!(*count, 3),
        _ => panic!("expected branch"),
    }
}

// ---------------------------------------------------------------------------
// Case 8: existing presets auto-categorize without migration
// ---------------------------------------------------------------------------

#[test]
fn existing_presets_auto_categorize() {
    // A preset that already has method + bean values in its values map.
    let presets = vec![make_preset(
        "V60 Onyx Classic",
        &[("method", "V60"), ("bean", "Onyx")],
    )];
    let axes = ["method".to_owned(), "bean".to_owned()];
    let tree = build(&presets, &axes);

    assert_eq!(tree.ungrouped.len(), 0);
    assert_eq!(tree.roots.len(), 1);
    let inner = match &tree.roots[0] {
        TreeNode::Branch { children, .. } => children,
        _ => panic!("expected branch"),
    };
    assert_eq!(inner.len(), 1);
    let leaf = match &inner[0] {
        TreeNode::Branch { children, .. } => &children[0],
        leaf => leaf,
    };
    match leaf {
        TreeNode::Leaf { preset_name, .. } => assert_eq!(preset_name, "V60 Onyx Classic"),
        _ => panic!("expected leaf"),
    }
}

// ---------------------------------------------------------------------------
// Case 9: single child at a level still shows branch (no auto-drill)
// ---------------------------------------------------------------------------

#[test]
fn single_child_at_level_still_shows_branch() {
    // One preset → should produce one Branch root with one child Branch with one Leaf.
    let presets = vec![make_preset("Solo", &[("method", "V60"), ("bean", "Onyx")])];
    let axes = ["method".to_owned(), "bean".to_owned()];
    let tree = build(&presets, &axes);

    // The root must still show the "method" branch even though only one child.
    assert_eq!(tree.roots.len(), 1, "single preset must still produce a root branch");
    let method_branch = &tree.roots[0];
    match method_branch {
        TreeNode::Branch { axis_value, children, .. } => {
            assert_eq!(axis_value, "V60");
            assert_eq!(children.len(), 1, "single bean branch still present");
        }
        TreeNode::Leaf { .. } => panic!("expected Branch at root level, not Leaf"),
    }
}

// ---------------------------------------------------------------------------
// Case 10: leaves preserve saved order (not alphabetically sorted)
// ---------------------------------------------------------------------------

#[test]
fn leaves_preserve_saved_order() {
    // Saved order is [B_preset, A_preset] — alphabetically reversed.
    // Leaves under the same branch must come out in that order.
    let presets = vec![
        make_preset("B_preset", &[("method", "V60"), ("bean", "Onyx")]),
        make_preset("A_preset", &[("method", "V60"), ("bean", "Onyx")]),
    ];
    let axes = ["method".to_owned(), "bean".to_owned()];
    let tree = build(&presets, &axes);

    // Drill to the Onyx sub-branch children.
    let v60 = match &tree.roots[0] {
        TreeNode::Branch { children, .. } => children,
        _ => panic!("expected V60 branch"),
    };
    let onyx = match &v60[0] {
        TreeNode::Branch { children, .. } => children,
        _ => panic!("expected Onyx branch"),
    };

    assert_eq!(onyx.len(), 2, "both presets present");
    let first_name = match &onyx[0] {
        TreeNode::Leaf { preset_name, .. } => preset_name.as_str(),
        _ => panic!("expected leaf"),
    };
    let second_name = match &onyx[1] {
        TreeNode::Leaf { preset_name, .. } => preset_name.as_str(),
        _ => panic!("expected leaf"),
    };
    assert_eq!(first_name, "B_preset", "leaves must preserve saved order");
    assert_eq!(second_name, "A_preset", "leaves must preserve saved order");
}

// ---------------------------------------------------------------------------
// suggest_preset_name
// ---------------------------------------------------------------------------

#[test]
fn suggest_skips_empty_axis_values() {
    let mut values = HashMap::new();
    values.insert("method".to_owned(), "V60".to_owned());
    values.insert("bean".to_owned(), String::new());
    let axes = ["method".to_owned(), "bean".to_owned()];
    let name = suggest_preset_name(&values, &axes);
    assert_eq!(name, "V60");
}

#[test]
fn suggest_joins_with_middle_dot() {
    let mut values = HashMap::new();
    values.insert("method".to_owned(), "V60".to_owned());
    values.insert("bean".to_owned(), "Onyx".to_owned());
    let axes = ["method".to_owned(), "bean".to_owned()];
    let name = suggest_preset_name(&values, &axes);
    assert_eq!(name, "V60 \u{00B7} Onyx");
}
