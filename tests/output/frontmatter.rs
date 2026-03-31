use pour::output::frontmatter::generate_frontmatter;

#[test]
fn basic_frontmatter_with_auto_date() {
    let fields = vec![
        ("brew_method".to_string(), "V60".to_string()),
        ("rating".to_string(), "4".to_string()),
    ];
    let result = generate_frontmatter(&fields);

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
    let result = generate_frontmatter(&fields);

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
    let result = generate_frontmatter(&fields);

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
    let result = generate_frontmatter(&fields);

    assert!(
        result.contains(r#"origin: "Ethiopia: Yirgacheffe""#),
        "value with colon should be quoted, got: {result}"
    );
}

#[test]
fn comma_separated_becomes_yaml_list() {
    let fields = vec![("tags".to_string(), "coffee, review, morning".to_string())];
    let result = generate_frontmatter(&fields);

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
    let result = generate_frontmatter(&fields);

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
    let result = generate_frontmatter(&fields);

    assert!(result.starts_with("---\n"));
    assert!(result.ends_with("---\n"));
    assert!(
        result.contains("date:"),
        "should still have auto-injected date"
    );
}
