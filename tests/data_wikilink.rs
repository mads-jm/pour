//! Tests for the shared wikilink stripper (`src/data/wikilink.rs`).

use pour::data::wikilink::strip_wikilink;

#[test]
fn strips_alias() {
    assert_eq!(strip_wikilink("[[Target|Alias]]"), "Target");
}

#[test]
fn strips_fragment() {
    assert_eq!(strip_wikilink("[[Target#Frag]]"), "Target");
}

#[test]
fn strips_fragment_and_alias() {
    assert_eq!(strip_wikilink("[[Target#Frag|Alias]]"), "Target");
}

#[test]
fn strips_plain_wikilink() {
    assert_eq!(strip_wikilink("[[Onyx]]"), "Onyx");
}

#[test]
fn passes_through_bare_value_unchanged() {
    assert_eq!(strip_wikilink("Onyx"), "Onyx");
    assert_eq!(strip_wikilink("not a link"), "not a link");
}

#[test]
fn trims_surrounding_whitespace() {
    assert_eq!(strip_wikilink("  [[Onyx]]  "), "Onyx");
    assert_eq!(strip_wikilink("  Onyx  "), "Onyx");
}

#[test]
fn handles_target_with_spaces() {
    assert_eq!(strip_wikilink("[[Onyx Coffee|the roaster]]"), "Onyx Coffee");
}

#[test]
fn malformed_link_returns_trimmed_input() {
    // Not a well-formed single wikilink — return unchanged (never panics).
    assert_eq!(strip_wikilink("[[unclosed"), "[[unclosed");
    assert_eq!(strip_wikilink("]]backwards[["), "]]backwards[[");
}
