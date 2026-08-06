use pour::config::Config;
use pour::output::FrontmatterComposite;
use pour::output::frontmatter::generate_frontmatter;

/// Fixed test date — avoids any real clock call in frontmatter unit tests.
const TEST_DATE: &str = "2026-01-15";

#[test]
fn basic_frontmatter_with_auto_date() {
    let fields = vec![
        ("brew_method".to_string(), "V60".to_string(), false),
        ("rating".to_string(), "4".to_string(), false),
    ];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

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
        ("rating".to_string(), "5".to_string(), false),
        ("date".to_string(), "2025-01-15".to_string(), false),
    ];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

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
        ("title".to_string(), "Hello".to_string(), false),
        ("empty_field".to_string(), String::new(), false),
    ];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

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
    let fields = vec![(
        "origin".to_string(),
        "Ethiopia: Yirgacheffe".to_string(),
        false,
    )];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

    assert!(
        result.contains(r#"origin: "Ethiopia: Yirgacheffe""#),
        "value with colon should be quoted, got: {result}"
    );
}

#[test]
fn comma_separated_becomes_yaml_list() {
    // list = true opts in to comma-split behavior
    let fields = vec![(
        "tags".to_string(),
        "coffee, review, morning".to_string(),
        true,
    )];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

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
    // list = true opts in to comma-split behavior
    let fields = vec![(
        "notes".to_string(),
        "good: flavor, bad: aftertaste".to_string(),
        true,
    )];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

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
        ("a".to_string(), String::new(), false),
        ("b".to_string(), String::new(), false),
    ];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

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

    let result = generate_frontmatter(&[], &composites, TEST_DATE, &[]);

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

    let result = generate_frontmatter(&[], &composites, TEST_DATE, &[]);

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

    let scalars = vec![(
        "bean".to_string(),
        "Ethiopian Yirgacheffe".to_string(),
        false,
    )];
    let composites: Vec<FrontmatterComposite<'_>> = vec![("recipe".to_string(), &subs, rows)];

    let result = generate_frontmatter(&scalars, &composites, TEST_DATE, &[]);

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
        let fields = vec![("flag".to_string(), word.to_string(), false)];
        let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
        assert!(
            result.contains(&format!("flag: \"{word}\"")),
            "bare word '{word}' should be quoted, got: {result}"
        );
    }
}

#[test]
fn yaml_reserved_bare_words_case_insensitive() {
    for word in &["True", "FALSE", "Null", "YES", "NO", "On", "OFF"] {
        let fields = vec![("flag".to_string(), word.to_string(), false)];
        let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
        assert!(
            result.contains(&format!("flag: \"{word}\"")),
            "bare word '{word}' (mixed-case) should be quoted, got: {result}"
        );
    }
}

#[test]
fn numeric_looking_strings_are_quoted() {
    for num in &["42", "3.14", "-7", "1e10", "0.0"] {
        let fields = vec![("val".to_string(), num.to_string(), false)];
        let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
        assert!(
            result.contains(&format!("val: \"{num}\"")),
            "numeric string '{num}' should be quoted, got: {result}"
        );
    }
}

#[test]
fn newline_in_value_is_escaped() {
    let fields = vec![("note".to_string(), "line one\nline two".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
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
    let fields = vec![("note".to_string(), "line one\rline two".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
    assert!(
        result.contains(r#"note: "line one\rline two""#),
        "carriage return should be escaped, got: {result}"
    );
}

#[test]
fn backslash_in_value_is_escaped() {
    let fields = vec![("path".to_string(), r"C:\Users\Joe".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
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
    let fields = vec![("v".to_string(), "C:\\\"file\"".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
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

    let result = generate_frontmatter(&[], &composites, TEST_DATE, &[]);

    assert!(
        !result.contains("recipe:"),
        "empty composite should be skipped"
    );
}

// --- Additional edge-case tests ---

#[test]
fn value_starting_with_dash_is_quoted() {
    let fields = vec![("mood".to_string(), "-negative".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
    assert!(
        result.contains("mood: \"-negative\""),
        "value starting with dash should be quoted, got: {result}"
    );
}

#[test]
fn value_with_embedded_quotes_escaped() {
    let fields = vec![("title".to_string(), "He said \"hello\"".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
    assert!(
        result.contains(r#"title: "He said \"hello\"""#),
        "embedded quotes should be escaped, got: {result}"
    );
}

#[test]
fn value_with_hash_is_quoted() {
    let fields = vec![("label".to_string(), "Coffee #3".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
    assert!(
        result.contains("label: \"Coffee #3\""),
        "hash should trigger quoting, got: {result}"
    );
}

#[test]
fn value_with_exclamation_is_quoted() {
    let fields = vec![("label".to_string(), "Wow!".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
    assert!(
        result.contains("label: \"Wow!\""),
        "exclamation should trigger quoting, got: {result}"
    );
}

#[test]
fn value_with_at_sign_is_quoted() {
    let fields = vec![("contact".to_string(), "user@email".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
    assert!(
        result.contains("contact: \"user@email\""),
        "@ should trigger quoting, got: {result}"
    );
}

#[test]
fn multiple_special_chars_quoted() {
    let fields = vec![("desc".to_string(), "a: b & c".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
    assert!(
        result.contains("desc: \"a: b & c\""),
        "multiple special chars should be quoted, got: {result}"
    );
}

#[test]
fn single_item_not_treated_as_list() {
    let fields = vec![("tag".to_string(), "coffee".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);
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

    let result = generate_frontmatter(&[], &composites, TEST_DATE, &[]);
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

    let result = generate_frontmatter(&[], &composites, TEST_DATE, &[]);
    assert!(
        result.contains("desc: \"A: B\""),
        "composite text with colon should be quoted, got: {result}"
    );
}

// --- list flag tests ---

#[test]
fn frontmatter_comma_value_is_literal_by_default() {
    // list = false (default): comma-separated value must be a single quoted scalar, not a list.
    let fields = vec![("tags".to_string(), "tag1, tag2".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

    // Should be a scalar line, not a YAML sequence.
    assert!(
        !result.contains("  - tag1"),
        "list=false should not produce sequence items, got: {result}"
    );
    assert!(
        !result.contains("  - tag2"),
        "list=false should not produce sequence items, got: {result}"
    );
    // The value contains a comma which is YAML-special, so it must be quoted.
    assert!(
        result.contains("tags: \"tag1, tag2\""),
        "list=false comma value should be a quoted scalar, got: {result}"
    );
}

#[test]
fn frontmatter_comma_value_splits_when_list_true() {
    // list = true: comma-separated value must be emitted as a YAML sequence.
    let fields = vec![("tags".to_string(), "tag1, tag2".to_string(), true)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &[]);

    assert!(
        result.contains("tags:\n"),
        "list=true should open a YAML sequence, got: {result}"
    );
    assert!(
        result.contains("  - tag1\n"),
        "list=true should emit first item, got: {result}"
    );
    assert!(
        result.contains("  - tag2\n"),
        "list=true should emit second item, got: {result}"
    );
}

// ── Static module frontmatter (`[modules.<n>.frontmatter]`) ──────────────────

/// Parse a `[modules.t.frontmatter]` table out of a config fragment, so these
/// tests exercise the same TOML→Value path the real config takes rather than
/// hand-building `toml::Value`s that TOML could never produce.
fn statics_from_toml(fragment: &str) -> std::collections::BTreeMap<String, toml::Value> {
    let toml = format!(
        r####"
[vault]
base_path = "/tmp"

[modules.t]
mode = "create"
path = "t.md"

[modules.t.frontmatter]
{fragment}

[[modules.t.fields]]
name = "body"
field_type = "text"
prompt = "Body"
"####
    );
    let config = Config::from_toml(&toml).expect("fragment should parse");
    config.modules["t"]
        .frontmatter
        .clone()
        .expect("frontmatter table should be present")
}

fn as_statics(map: &std::collections::BTreeMap<String, toml::Value>) -> Vec<(&str, &toml::Value)> {
    map.iter().map(|(k, v)| (k.as_str(), v)).collect()
}

#[test]
fn static_array_renders_as_a_block_sequence() {
    let map = statics_from_toml(r#"tags = ["lyra", "toss"]"#);
    let result = generate_frontmatter(&[], &[], TEST_DATE, &as_statics(&map));

    assert_eq!(
        result,
        "---\ndate: 2026-01-15\ntags:\n  - lyra\n  - toss\n---\n"
    );
}

#[test]
fn single_element_static_array_still_renders_as_a_sequence() {
    // The regression that forced static frontmatter to have its own renderer:
    // the `list = true` path only emits a sequence when the value contains
    // ", ", so a one-element array would have come out as `cssclasses: mads-toss`.
    let map = statics_from_toml(r#"cssclasses = ["mads-toss"]"#);
    let result = generate_frontmatter(&[], &[], TEST_DATE, &as_statics(&map));

    assert!(
        result.contains("cssclasses:\n  - mads-toss\n"),
        "single-element array must be a block sequence, got: {result}"
    );
    assert!(
        !result.contains("cssclasses: mads-toss"),
        "must not collapse to a scalar, got: {result}"
    );
}

#[test]
fn static_scalars_render_as_scalars() {
    let map = statics_from_toml(
        r#"
author = "mads"
weight = 3
ratio = 1.5
pinned = true
"#,
    );
    let result = generate_frontmatter(&[], &[], TEST_DATE, &as_statics(&map));

    // Numbers and booleans stay typed and unquoted; strings are bare when YAML
    // will read them back as strings.
    assert!(result.contains("author: mads\n"), "got: {result}");
    assert!(result.contains("weight: 3\n"), "got: {result}");
    assert!(result.contains("ratio: 1.5\n"), "got: {result}");
    assert!(result.contains("pinned: true\n"), "got: {result}");
}

#[test]
fn static_keys_are_emitted_in_alphabetical_order() {
    // BTreeMap, not HashMap: the order must be stable across runs so that two
    // captures a second apart do not produce diffs in key order.
    let map = statics_from_toml(
        r#"
tags = ["lyra"]
author = "mads"
cssclasses = ["mads-toss"]
"#,
    );
    let result = generate_frontmatter(&[], &[], TEST_DATE, &as_statics(&map));

    let author = result.find("author:").expect("author present");
    let cssclasses = result.find("cssclasses:").expect("cssclasses present");
    let tags = result.find("tags:").expect("tags present");
    assert!(author < cssclasses && cssclasses < tags, "got: {result}");
}

#[test]
fn statics_are_emitted_after_captured_fields() {
    let map = statics_from_toml(r#"author = "mads""#);
    let fields = vec![("kind".to_string(), "musing".to_string(), false)];
    let result = generate_frontmatter(&fields, &[], TEST_DATE, &as_statics(&map));

    let kind = result.find("kind:").expect("kind present");
    let author = result.find("author:").expect("author present");
    assert!(kind < author, "captured fields come first, got: {result}");
}

#[test]
fn static_string_needing_quotes_is_quoted() {
    let map = statics_from_toml(r#"note = "key: value""#);
    let result = generate_frontmatter(&[], &[], TEST_DATE, &as_statics(&map));

    assert!(result.contains(r#"note: "key: value""#), "got: {result}");
}

#[test]
fn empty_static_array_is_skipped() {
    let map = statics_from_toml(r#"tags = []"#);
    let result = generate_frontmatter(&[], &[], TEST_DATE, &as_statics(&map));

    assert!(!result.contains("tags"), "got: {result}");
}

#[test]
fn empty_static_string_is_skipped() {
    // Matches how empty captured field values are already treated.
    let map = statics_from_toml(r#"author = """#);
    let result = generate_frontmatter(&[], &[], TEST_DATE, &as_statics(&map));

    assert!(!result.contains("author"), "got: {result}");
}

#[test]
fn no_statics_is_byte_identical_to_before_the_feature() {
    let fields = vec![("kind".to_string(), "musing".to_string(), false)];

    assert_eq!(
        generate_frontmatter(&fields, &[], TEST_DATE, &[]),
        "---\ndate: 2026-01-15\nkind: musing\n---\n"
    );
}

// ─── Read-only parse + single-key line patch (`update` mode) ─────────────────

use pour::output::frontmatter::{
    FrontmatterValue, PatchLineError, PatchOutcome, format_number, patch_frontmatter_line,
    read_frontmatter,
};

/// A realistic daily note: mixed quoting, a block sequence, an explicit `null`,
/// a float, a comment, and a thematic break in the body. Everything the patcher
/// must leave alone lives in here.
const DAILY_NOTE: &str = "---\n\
date created: Wednesday, August 5th 2026\n\
tags:\n\
  - daily\n\
  - habits\n\
# the template owns these two\n\
cannabis: false\n\
water: null\n\
mood: \"content — mostly\"\n\
sleep: 7.5\n\
title: 'Thursday'\n\
---\n\
\n\
# 20260805\n\
\n\
Some body text with a colon: like this.\n\
\n\
---\n\
\n\
## Journal\n\
\n\
The break above is a thematic break, not frontmatter.\n";

#[test]
fn read_frontmatter_reads_top_level_scalars() {
    let map = read_frontmatter(DAILY_NOTE).expect("note has a frontmatter block");

    assert_eq!(map.get("cannabis").map(String::as_str), Some("false"));
    assert_eq!(map.get("water").map(String::as_str), Some("null"));
    assert_eq!(map.get("sleep").map(String::as_str), Some("7.5"));
    // Quotes are stripped, both flavours.
    assert_eq!(
        map.get("mood").map(String::as_str),
        Some("content — mostly")
    );
    assert_eq!(map.get("title").map(String::as_str), Some("Thursday"));
    // A key whose name contains a space is still a key.
    assert_eq!(
        map.get("date created").map(String::as_str),
        Some("Wednesday, August 5th 2026")
    );
}

#[test]
fn read_frontmatter_skips_sequences_and_comments() {
    let map = read_frontmatter(DAILY_NOTE).expect("note has a frontmatter block");

    // The `tags:` key exists with an empty scalar; its items are not keys.
    assert_eq!(map.get("tags").map(String::as_str), Some(""));
    assert!(!map.contains_key("- daily"), "sequence items are not keys");
    assert!(
        !map.keys().any(|k| k.starts_with('#')),
        "comments are not keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

#[test]
fn read_frontmatter_ignores_the_body_entirely() {
    let map = read_frontmatter(DAILY_NOTE).expect("note has a frontmatter block");
    assert!(
        !map.contains_key("Some body text with a colon"),
        "body lines after the closing delimiter are not frontmatter: {map:?}"
    );
}

#[test]
fn read_frontmatter_returns_none_without_a_block() {
    assert_eq!(read_frontmatter("# Just a note\n\nno frontmatter\n"), None);
    // A `---` that never closes is not a block.
    assert_eq!(read_frontmatter("---\nwater: 4\n"), None);
    // The block must open on line 1, as Obsidian requires.
    assert_eq!(read_frontmatter("\n---\nwater: 4\n---\n"), None);
}

#[test]
fn patch_replaces_one_line_and_nothing_else() {
    let (patched, outcome) =
        patch_frontmatter_line(DAILY_NOTE, "water", "16").expect("water is a scalar key");

    assert_eq!(outcome, PatchOutcome::Replaced);
    // The single strongest guarantee this cycle owes the user: the output is
    // the input with exactly one line swapped. Key order, quoting style,
    // comments, the sequence, and the whole body survive byte-for-byte.
    assert_eq!(patched, DAILY_NOTE.replace("water: null", "water: 16"));
}

#[test]
fn patch_inserts_a_missing_key_as_the_last_block_line() {
    let (patched, outcome) =
        patch_frontmatter_line(DAILY_NOTE, "steps", "8000").expect("insert is allowed");

    assert_eq!(outcome, PatchOutcome::Inserted);
    assert_eq!(
        patched,
        DAILY_NOTE.replace(
            "title: 'Thursday'\n---\n",
            "title: 'Thursday'\nsteps: 8000\n---\n"
        )
    );
}

#[test]
fn patch_into_an_empty_block_works() {
    let (patched, outcome) = patch_frontmatter_line("---\n---\n# Note\n", "water", "16").unwrap();
    assert_eq!(outcome, PatchOutcome::Inserted);
    assert_eq!(patched, "---\nwater: 16\n---\n# Note\n");
}

#[test]
fn patch_preserves_crlf_line_endings() {
    let crlf = "---\r\ncannabis: false\r\nwater: null\r\n---\r\n\r\nbody\r\n";
    let (patched, _) = patch_frontmatter_line(crlf, "cannabis", "true").unwrap();
    assert_eq!(patched, crlf.replace("cannabis: false", "cannabis: true"));

    let (inserted, outcome) = patch_frontmatter_line(crlf, "steps", "10").unwrap();
    assert_eq!(outcome, PatchOutcome::Inserted);
    assert_eq!(
        inserted,
        crlf.replace("water: null\r\n---", "water: null\r\nsteps: 10\r\n---")
    );
}

#[test]
fn patch_refuses_a_multiline_value_rather_than_orphan_its_lines() {
    // `tags:` opens a block sequence — replacing its one line would leave the
    // `  - daily` items dangling under whatever came next.
    assert_eq!(
        patch_frontmatter_line(DAILY_NOTE, "tags", "x"),
        Err(PatchLineError::MultilineValue("tags".to_string()))
    );

    let block_scalar = "---\nnote: |\n  first\n  second\n---\n";
    assert_eq!(
        patch_frontmatter_line(block_scalar, "note", "x"),
        Err(PatchLineError::MultilineValue("note".to_string()))
    );
}

#[test]
fn patch_errors_when_there_is_no_frontmatter_block() {
    assert_eq!(
        patch_frontmatter_line("# Just a note\n", "water", "16"),
        Err(PatchLineError::NoFrontmatterBlock)
    );
}

#[test]
fn frontmatter_value_renders_yaml_and_json_per_transport() {
    // Numbers and booleans stay bare in YAML so Obsidian reads them as typed
    // properties; text goes through the existing quoting rules.
    assert_eq!(FrontmatterValue::Number(64.0).to_yaml(), "64");
    assert_eq!(FrontmatterValue::Number(12.5).to_yaml(), "12.5");
    assert_eq!(FrontmatterValue::Bool(true).to_yaml(), "true");
    assert_eq!(FrontmatterValue::Text("plain".into()).to_yaml(), "plain");
    assert_eq!(
        FrontmatterValue::Text("has: colon".into()).to_yaml(),
        "\"has: colon\""
    );

    assert_eq!(FrontmatterValue::Number(64.0).to_json(), "64");
    assert_eq!(FrontmatterValue::Bool(false).to_json(), "false");
    assert_eq!(
        FrontmatterValue::Text("say \"hi\"".into()).to_json(),
        "\"say \\\"hi\\\"\""
    );
}

#[test]
fn format_number_emits_integral_values_bare() {
    assert_eq!(format_number(96.0), "96");
    assert_eq!(format_number(0.0), "0");
    assert_eq!(format_number(-3.0), "-3");
    assert_eq!(format_number(12.5), "12.5");
}
