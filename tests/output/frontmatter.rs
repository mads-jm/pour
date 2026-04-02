use pour::config::Config;
use pour::output::FrontmatterComposite;
use pour::output::frontmatter::generate_frontmatter;

#[test]
fn basic_frontmatter_with_auto_date() {
    let fields = vec![
        ("brew_method".to_string(), "V60".to_string()),
        ("rating".to_string(), "4".to_string()),
    ];
    let result = generate_frontmatter(&fields, &[]);

    assert!(result.starts_with("---\n"), "should start with ---");
    assert!(result.ends_with("---\n"), "should end with ---");
    assert!(result.contains("date:"), "should auto-inject date");
    assert!(
        result.contains("brew_method: V60"),
        "should contain brew_method"
    );
    assert!(result.contains("rating: 4"), "should contain rating");

    // Date should be the first field after the opening ---.
    let lines: Vec<&str> = result.lines().collect();
    assert!(lines[1].starts_with("date:"), "date should be first field");
}

#[test]
fn explicit_date_is_preserved_and_first() {
    let fields = vec![
        ("rating".to_string(), "5".to_string()),
        ("date".to_string(), "2025-01-15".to_string()),
    ];
    let result = generate_frontmatter(&fields, &[]);

    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(
        lines[1], "date: 2025-01-15",
        "explicit date should be first and preserved"
    );
    // Should NOT have a second date line.
    let date_count = result.matches("date:").count();
    assert_eq!(date_count, 1, "should have exactly one date field");
}

#[test]
fn empty_values_are_skipped() {
    let fields = vec![
        ("title".to_string(), "Hello".to_string()),
        ("empty_field".to_string(), String::new()),
    ];
    let result = generate_frontmatter(&fields, &[]);

    assert!(
        !result.contains("empty_field"),
        "empty values should be skipped"
    );
    assert!(
        result.contains("title: Hello"),
        "non-empty values should appear"
    );
}

#[test]
fn special_chars_are_quoted() {
    let fields = vec![("origin".to_string(), "Ethiopia: Yirgacheffe".to_string())];
    let result = generate_frontmatter(&fields, &[]);

    assert!(
        result.contains(r#"origin: "Ethiopia: Yirgacheffe""#),
        "value with colon should be quoted, got: {result}"
    );
}

#[test]
fn comma_separated_becomes_yaml_list() {
    let fields = vec![("tags".to_string(), "coffee, review, morning".to_string())];
    let result = generate_frontmatter(&fields, &[]);

    assert!(result.contains("tags:\n"), "should start a YAML list");
    assert!(
        result.contains("  - coffee\n"),
        "should have list item coffee"
    );
    assert!(
        result.contains("  - review\n"),
        "should have list item review"
    );
    assert!(
        result.contains("  - morning\n"),
        "should have list item morning"
    );
}

#[test]
fn comma_separated_items_with_special_chars_are_quoted() {
    let fields = vec![(
        "notes".to_string(),
        "good: flavor, bad: aftertaste".to_string(),
    )];
    let result = generate_frontmatter(&fields, &[]);

    assert!(
        result.contains("  - \"good: flavor\""),
        "list items with colons should be quoted"
    );
    assert!(
        result.contains("  - \"bad: aftertaste\""),
        "list items with colons should be quoted"
    );
}

#[test]
fn all_empty_fields_still_produces_date() {
    let fields = vec![
        ("a".to_string(), String::new()),
        ("b".to_string(), String::new()),
    ];
    let result = generate_frontmatter(&fields, &[]);

    assert!(result.starts_with("---\n"));
    assert!(result.ends_with("---\n"));
    assert!(
        result.contains("date:"),
        "should still have auto-injected date"
    );
}

// --- composite frontmatter tests ---

fn recipe_sub_fields() -> Vec<pour::config::SubFieldConfig> {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.c]
mode = "create"
path = "c.md"

[[modules.c.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Brew"

[[modules.c.fields.sub_fields]]
name = "pour"
field_type = "number"
prompt = "Pour (g)"

[[modules.c.fields.sub_fields]]
name = "time"
field_type = "number"
prompt = "Time (s)"

[[modules.c.fields.sub_fields]]
name = "technique"
field_type = "static_select"
prompt = "Technique"
options = ["Bloom", "Spiral"]
"####;
    let config = Config::from_toml(toml).unwrap();
    let module = &config.modules["c"];
    module.fields[0].sub_fields.clone().unwrap()
}

#[test]
fn composite_frontmatter_sequence_of_mappings() {
    let subs = recipe_sub_fields();
    let rows = vec![
        vec!["50".to_string(), "30".to_string(), "Bloom".to_string()],
        vec!["100".to_string(), "45".to_string(), "Spiral".to_string()],
    ];
    let composites: Vec<FrontmatterComposite<'_>> = vec![("recipe".to_string(), &subs, rows)];

    let result = generate_frontmatter(&[], &composites);

    assert!(result.contains("recipe:"), "should have recipe key");
    assert!(result.contains("  - pour: 50"), "first row pour");
    assert!(result.contains("    time: 30"), "first row time");
    assert!(
        result.contains("    technique: Bloom"),
        "first row technique"
    );
    assert!(result.contains("  - pour: 100"), "second row pour");
    assert!(result.contains("    time: 45"), "second row time");
    assert!(
        result.contains("    technique: Spiral"),
        "second row technique"
    );
}

#[test]
fn composite_numbers_serialize_unquoted() {
    let subs = recipe_sub_fields();
    let rows = vec![vec![
        "42".to_string(),
        "10".to_string(),
        "Bloom".to_string(),
    ]];
    let composites: Vec<FrontmatterComposite<'_>> = vec![("recipe".to_string(), &subs, rows)];

    let result = generate_frontmatter(&[], &composites);

    // Numbers should NOT be quoted
    assert!(result.contains("pour: 42"), "number should be unquoted");
    assert!(
        !result.contains("pour: \"42\""),
        "number should not be quoted"
    );
}

#[test]
fn composite_mixed_with_scalar_fields() {
    let subs = recipe_sub_fields();
    let rows = vec![vec![
        "50".to_string(),
        "30".to_string(),
        "Bloom".to_string(),
    ]];

    let scalars = vec![("bean".to_string(), "Ethiopian Yirgacheffe".to_string())];
    let composites: Vec<FrontmatterComposite<'_>> = vec![("recipe".to_string(), &subs, rows)];

    let result = generate_frontmatter(&scalars, &composites);

    assert!(
        result.contains("bean: Ethiopian Yirgacheffe"),
        "scalar field"
    );
    assert!(result.contains("recipe:"), "composite field");
    assert!(result.contains("  - pour: 50"), "composite row");
}

#[test]
fn composite_empty_rows_skipped() {
    let subs = recipe_sub_fields();
    let rows: Vec<Vec<String>> = vec![];
    let composites: Vec<FrontmatterComposite<'_>> = vec![("recipe".to_string(), &subs, rows)];

    let result = generate_frontmatter(&[], &composites);

    assert!(
        !result.contains("recipe:"),
        "empty composite should be skipped"
    );
}
