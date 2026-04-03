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
    assert!(
        result.contains("rating: \"4\""),
        "should contain rating (quoted numeric)"
    );

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

// --- format_scalar / needs_quoting edge-case tests ---

#[test]
fn yaml_reserved_bare_words_are_quoted() {
    for word in &["true", "false", "null", "yes", "no", "on", "off"] {
        let fields = vec![("flag".to_string(), word.to_string())];
        let result = generate_frontmatter(&fields, &[]);
        assert!(
            result.contains(&format!("flag: \"{word}\"")),
            "bare word '{word}' should be quoted, got: {result}"
        );
    }
}

#[test]
fn yaml_reserved_bare_words_case_insensitive() {
    for word in &["True", "FALSE", "Null", "YES", "NO", "On", "OFF"] {
        let fields = vec![("flag".to_string(), word.to_string())];
        let result = generate_frontmatter(&fields, &[]);
        assert!(
            result.contains(&format!("flag: \"{word}\"")),
            "bare word '{word}' (mixed-case) should be quoted, got: {result}"
        );
    }
}

#[test]
fn numeric_looking_strings_are_quoted() {
    for num in &["42", "3.14", "-7", "1e10", "0.0"] {
        let fields = vec![("val".to_string(), num.to_string())];
        let result = generate_frontmatter(&fields, &[]);
        assert!(
            result.contains(&format!("val: \"{num}\"")),
            "numeric string '{num}' should be quoted, got: {result}"
        );
    }
}

#[test]
fn newline_in_value_is_escaped() {
    let fields = vec![("note".to_string(), "line one\nline two".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    // The literal newline must be replaced with \n inside the quoted string.
    assert!(
        result.contains(r#"note: "line one\nline two""#),
        "newline should be escaped, got: {result}"
    );
    // The raw newline must not appear inside the value (only between YAML lines).
    let value_line = result
        .lines()
        .find(|l| l.starts_with("note:"))
        .expect("note field should be present");
    assert!(
        !value_line.contains('\n'),
        "value line should not contain a raw newline"
    );
}

#[test]
fn carriage_return_in_value_is_escaped() {
    let fields = vec![("note".to_string(), "line one\rline two".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains(r#"note: "line one\rline two""#),
        "carriage return should be escaped, got: {result}"
    );
}

#[test]
fn backslash_in_value_is_escaped() {
    let fields = vec![("path".to_string(), r"C:\Users\Joe".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    // The single backslash must be doubled inside the quoted YAML string.
    assert!(
        result.contains(r#"path: "C:\\Users\\Joe""#),
        "backslashes should be escaped, got: {result}"
    );
}

#[test]
fn backslash_before_double_quote_ordering() {
    // A value containing both a backslash and a double-quote.
    // Correct output: "C:\\\"file\""  — backslash doubled, quote escaped.
    // Wrong (if order reversed): "C:\"\\file\"" etc.
    let fields = vec![("v".to_string(), "C:\\\"file\"".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains(r#"v: "C:\\\"file\"""#),
        "backslash-then-quote ordering must be correct, got: {result}"
    );
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

// --- Additional edge-case tests ---

#[test]
fn value_starting_with_dash_is_quoted() {
    let fields = vec![("mood".to_string(), "-negative".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains("mood: \"-negative\""),
        "value starting with dash should be quoted, got: {result}"
    );
}

#[test]
fn value_with_embedded_quotes_escaped() {
    let fields = vec![("title".to_string(), "He said \"hello\"".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains(r#"title: "He said \"hello\"""#),
        "embedded quotes should be escaped, got: {result}"
    );
}

#[test]
fn value_with_hash_is_quoted() {
    let fields = vec![("label".to_string(), "Coffee #3".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains("label: \"Coffee #3\""),
        "hash should trigger quoting, got: {result}"
    );
}

#[test]
fn value_with_exclamation_is_quoted() {
    let fields = vec![("label".to_string(), "Wow!".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains("label: \"Wow!\""),
        "exclamation should trigger quoting, got: {result}"
    );
}

#[test]
fn value_with_at_sign_is_quoted() {
    let fields = vec![("contact".to_string(), "user@email".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains("contact: \"user@email\""),
        "@ should trigger quoting, got: {result}"
    );
}

#[test]
fn multiple_special_chars_quoted() {
    let fields = vec![("desc".to_string(), "a: b & c".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains("desc: \"a: b & c\""),
        "multiple special chars should be quoted, got: {result}"
    );
}

#[test]
fn single_item_not_treated_as_list() {
    let fields = vec![("tag".to_string(), "coffee".to_string())];
    let result = generate_frontmatter(&fields, &[]);
    assert!(
        result.contains("tag: coffee"),
        "single item without comma-space should be scalar, got: {result}"
    );
    assert!(!result.contains("  - coffee"), "should not be a list item");
}

#[test]
fn composite_with_empty_cells_skips_them() {
    let subs = recipe_sub_fields();
    // Row where only "pour" has a value, time and technique are empty
    let rows = vec![vec!["50".to_string(), String::new(), String::new()]];
    let composites: Vec<FrontmatterComposite<'_>> = vec![("recipe".to_string(), &subs, rows)];

    let result = generate_frontmatter(&[], &composites);
    assert!(
        result.contains("  - pour: 50"),
        "non-empty cell should appear"
    );
    assert!(!result.contains("time:"), "empty cell should be skipped");
    assert!(
        !result.contains("technique:"),
        "empty cell should be skipped"
    );
}

#[test]
fn composite_text_cell_with_special_chars_quoted() {
    // Use a composite with a text sub-field containing special chars
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.c]
mode = "create"
path = "c.md"

[[modules.c.fields]]
name = "items"
field_type = "composite_array"
prompt = "Items"

[[modules.c.fields.sub_fields]]
name = "desc"
field_type = "text"
prompt = "Description"
"####;
    let config = Config::from_toml(toml).unwrap();
    let module = &config.modules["c"];
    let subs = module.fields[0].sub_fields.clone().unwrap();

    let rows = vec![vec!["A: B".to_string()]];
    let composites: Vec<FrontmatterComposite<'_>> = vec![("items".to_string(), &subs, rows)];

    let result = generate_frontmatter(&[], &composites);
    assert!(
        result.contains("desc: \"A: B\""),
        "composite text with colon should be quoted, got: {result}"
    );
}
