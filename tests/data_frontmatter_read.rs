//! Tests for the shared frontmatter reader (`src/data/frontmatter_read.rs`).
//!
//! Covers the round-trip on Pour-authored frontmatter plus robustness on richer
//! externally-edited YAML the reader must not crash on.

use pour::data::frontmatter_read::{FrontmatterValue, parse_frontmatter};
use pour::output::frontmatter::generate_frontmatter;

fn scalar<'a>(fm: &'a pour::data::frontmatter_read::Frontmatter, key: &str) -> Option<&'a str> {
    fm.get(key).and_then(FrontmatterValue::as_scalar)
}

#[test]
fn parses_pour_authored_scalars() {
    let note = "---\ndate: 2026-07-13\nroaster: Onyx\nmethod: V60\n---\nbody text\n";
    let fm = parse_frontmatter(note);
    assert_eq!(scalar(&fm, "date"), Some("2026-07-13"));
    assert_eq!(scalar(&fm, "roaster"), Some("Onyx"));
    assert_eq!(scalar(&fm, "method"), Some("V60"));
}

#[test]
fn round_trips_pour_writer_output() {
    // Feed the hand-rolled writer's output back through the reader. This is the
    // reader/writer round-trip fidelity check (ADR-007).
    let fields = vec![
        ("roaster".to_string(), "Onyx: Coffee".to_string(), false), // colon → quoted
        ("bean".to_string(), "Ethiopia, Guji".to_string(), false),  // comma, not a list
        ("tags".to_string(), "a, b, c".to_string(), true),          // list = true → sequence
    ];
    let written = generate_frontmatter(&fields, &[], "2026-07-13");

    let fm = parse_frontmatter(&written);
    assert_eq!(scalar(&fm, "date"), Some("2026-07-13"));
    // Quoted scalar round-trips back to the raw value.
    assert_eq!(scalar(&fm, "roaster"), Some("Onyx: Coffee"));
    // A comma value NOT declared as a list stays a single scalar.
    assert_eq!(scalar(&fm, "bean"), Some("Ethiopia, Guji"));
    // A list-declared value round-trips to a sequence.
    assert_eq!(
        fm.get("tags").and_then(FrontmatterValue::as_list),
        Some(["a".to_string(), "b".to_string(), "c".to_string()].as_slice())
    );
}

#[test]
fn parses_block_sequences() {
    let note = "---\ntags:\n  - coffee\n  - v60\nauthor: me\n---\n";
    let fm = parse_frontmatter(note);
    assert_eq!(
        fm.get("tags").and_then(FrontmatterValue::as_list),
        Some(["coffee".to_string(), "v60".to_string()].as_slice())
    );
    assert_eq!(scalar(&fm, "author"), Some("me"));
}

#[test]
fn unescapes_double_quoted_values() {
    let note = "---\nnote: \"line one\\nline two\"\npath: \"a\\\\b\"\n---\n";
    let fm = parse_frontmatter(note);
    assert_eq!(scalar(&fm, "note"), Some("line one\nline two"));
    assert_eq!(scalar(&fm, "path"), Some("a\\b"));
}

#[test]
fn no_frontmatter_block_yields_empty() {
    assert!(parse_frontmatter("just body, no fences\n").is_empty());
    assert!(parse_frontmatter("").is_empty());
    // A block that never closes still parses what it can, but a note that does
    // not START with a fence yields nothing.
    assert!(parse_frontmatter("text\n---\nkey: value\n---\n").is_empty());
}

#[test]
fn tolerates_bom_and_crlf() {
    let note = "\u{feff}---\r\nroaster: Onyx\r\n---\r\n";
    let fm = parse_frontmatter(note);
    assert_eq!(scalar(&fm, "roaster"), Some("Onyx"));
}

#[test]
fn degrades_on_richer_yaml_without_crashing() {
    // Nested mappings, flow collections, comments, and a quoted key — none of
    // which Pour writes. The reader must not crash; it extracts the top-level
    // scalars it understands and skips the rest.
    let note = r#"---
# a comment line
title: My Note
nested:
  child: value
  deeper:
    x: 1
flow: {a: 1, b: 2}
list_inline: [1, 2, 3]
url: https://example.com/path
roaster: Onyx
---
body
"#;
    let fm = parse_frontmatter(note);
    // Understood top-level scalars survive.
    assert_eq!(scalar(&fm, "title"), Some("My Note"));
    assert_eq!(scalar(&fm, "roaster"), Some("Onyx"));
    // A value with a colon (URL) keeps everything after the first colon.
    assert_eq!(scalar(&fm, "url"), Some("https://example.com/path"));
    // Flow collections are captured as raw scalars (not modelled, but no crash).
    assert!(fm.contains_key("flow"));
    // The nested mapping key exists but its indented children are skipped.
    assert!(fm.contains_key("nested"));
}

#[test]
fn handles_empty_and_whitespace_values() {
    let note = "---\nempty:\nblank:   \nfilled: x\n---\n";
    let fm = parse_frontmatter(note);
    // `empty:` with no sequence → empty scalar.
    assert_eq!(scalar(&fm, "empty"), Some(""));
    assert_eq!(scalar(&fm, "blank"), Some(""));
    assert_eq!(scalar(&fm, "filled"), Some("x"));
}
