//! Frontmatter *reader* — parses the `---`-fenced YAML block of a note into
//! key/value data.
//!
//! # Scope (deliberately narrow)
//!
//! This reader targets the constrained YAML subset Pour's own *writer* emits
//! (`src/output/frontmatter.rs`): top-level `key: value` scalars, double-quoted
//! values carrying YAML-special characters, and block sequences produced by the
//! writer's comma-expansion (`key:` followed by `  - item` lines). It is the
//! building block the **FS-scan fallback path** consumes to filter captures on
//! disk; on the API `/search/` path Obsidian returns pre-parsed frontmatter, so
//! this reader is not on the API happy path.
//!
//! It is **not** a general-purpose YAML parser. Anchors, multi-document
//! streams, flow collections, nested mappings, and block scalars are not
//! modelled. The contract is: parse what Pour writes faithfully, and **never
//! crash** on richer externally-edited YAML — unrecognised lines are skipped
//! rather than erroring. See ADR-007.
//!
//! Shared foundation: the priors resolver uses this for the FS-fallback path,
//! and lookup-fields L1 will reuse it.

use std::collections::BTreeMap;

/// A parsed value from a frontmatter block.
///
/// Only the two shapes Pour's writer emits are represented. Sequences preserve
/// order. Everything else the reader encounters is normalised into one of these
/// (a lone scalar, or a sequence).
#[derive(Debug, Clone, PartialEq)]
pub enum FrontmatterValue {
    /// A single scalar value (string), with surrounding quotes removed.
    Scalar(String),
    /// A YAML block sequence (`- item` lines), values in document order.
    List(Vec<String>),
}

impl FrontmatterValue {
    /// Borrow the value as a scalar string, if it is one.
    pub fn as_scalar(&self) -> Option<&str> {
        match self {
            FrontmatterValue::Scalar(s) => Some(s.as_str()),
            FrontmatterValue::List(_) => None,
        }
    }

    /// Borrow the value as a list of strings, if it is one.
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            FrontmatterValue::List(items) => Some(items),
            FrontmatterValue::Scalar(_) => None,
        }
    }
}

/// Parsed frontmatter: an ordered-by-key map of field name → value.
///
/// Backed by a `BTreeMap` for deterministic iteration; callers key by field
/// name, so insertion order is not meaningful here.
pub type Frontmatter = BTreeMap<String, FrontmatterValue>;

/// Parse the leading `---`-fenced frontmatter block of a note.
///
/// Returns an empty map when the note has no frontmatter block (does not start
/// with `---`), or when the block is empty. Never returns an error: malformed
/// or unrecognised lines inside the block are skipped, honouring the "must not
/// crash on richer YAML" contract.
///
/// The opening fence must be the first line (a leading UTF-8 BOM is tolerated).
/// The block ends at the first subsequent `---` or `...` line.
pub fn parse_frontmatter(note: &str) -> Frontmatter {
    let mut result = Frontmatter::new();

    // Tolerate a leading BOM and normalise CRLF by trimming `\r` per line.
    let note = note.strip_prefix('\u{feff}').unwrap_or(note);

    let mut lines = note.lines();

    // The opening fence must be the very first line.
    match lines.next() {
        Some(first) if first.trim_end() == "---" => {}
        _ => return result,
    }

    // Collect the frontmatter body up to the closing fence.
    let mut body: Vec<&str> = Vec::new();
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            break;
        }
        body.push(line);
    }

    let mut i = 0;
    while i < body.len() {
        let line = body[i];

        // Skip blank lines and comments.
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        // Only consider top-level keys (no leading indentation). Indented lines
        // that are not part of a recognised sequence are skipped — this is where
        // richer nested YAML degrades gracefully.
        if line.starts_with([' ', '\t']) {
            i += 1;
            continue;
        }

        // A top-level entry is `key:` or `key: value`. Split on the first colon.
        let Some(colon) = find_key_colon(line) else {
            // Not a `key:` line we understand — skip it.
            i += 1;
            continue;
        };

        let key = line[..colon].trim().to_string();
        if key.is_empty() {
            i += 1;
            continue;
        }
        let rest = line[colon + 1..].trim();

        if rest.is_empty() {
            // `key:` with no inline value → look ahead for a block sequence.
            let (items, consumed) = collect_sequence(&body[i + 1..]);
            if !items.is_empty() {
                result.insert(key, FrontmatterValue::List(items));
                i += 1 + consumed;
            } else {
                // Empty value with no sequence — record as empty scalar.
                result.insert(key, FrontmatterValue::Scalar(String::new()));
                i += 1;
            }
        } else {
            result.insert(key, FrontmatterValue::Scalar(unquote_scalar(rest)));
            i += 1;
        }
    }

    result
}

/// Find the byte index of the colon that separates a top-level `key:` from its
/// value, ignoring colons that appear inside a quoted region.
///
/// Pour's writer never emits colons in bare keys, but externally-edited notes
/// might carry `url: https://…` where the value has a colon; we want the *first*
/// colon (the key/value separator), so a simple search suffices, but we skip
/// colons inside a leading quote to avoid mis-splitting a quoted key. Returns
/// `None` if there is no colon.
fn find_key_colon(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    // If the line starts with a quote, the key is quoted — find the closing
    // quote, then the following colon.
    if bytes.first() == Some(&b'"') {
        if let Some(close) = line[1..].find('"') {
            let after = 1 + close + 1;
            return line[after..].find(':').map(|c| after + c);
        }
        return None;
    }
    line.find(':')
}

/// Collect a YAML block sequence (`  - item` lines) starting at `lines[0]`.
///
/// Returns the parsed items and the number of lines consumed. Stops at the
/// first line that is not a sequence item (blank lines within the sequence are
/// tolerated and skipped). Items are unquoted.
fn collect_sequence(lines: &[&str]) -> (Vec<String>, usize) {
    let mut items = Vec::new();
    let mut consumed = 0;

    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            // A blank line ends the block if we have not seen items yet;
            // otherwise it terminates the sequence.
            break;
        }
        // Sequence items are indented `- item`. Require indentation so a
        // top-level `- ...` (not something Pour writes) does not get absorbed.
        if !line.starts_with([' ', '\t']) {
            break;
        }
        let Some(item) = trimmed.strip_prefix("- ") else {
            // Indented non-item line (e.g. a nested mapping) — stop; the outer
            // loop will skip it gracefully.
            break;
        };
        items.push(unquote_scalar(item.trim()));
        consumed += 1;
    }

    (items, consumed)
}

/// Remove surrounding double quotes from a scalar and unescape the sequences
/// Pour's writer emits (`\\`, `\"`, `\n`, `\r`).
///
/// Bare (unquoted) scalars are returned trimmed and unchanged. Single-quoted
/// scalars (which Pour never writes but Obsidian might) have their quotes
/// stripped with YAML's `''` → `'` unescaping.
fn unquote_scalar(value: &str) -> String {
    let value = value.trim();

    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        let inner = &value[1..value.len() - 1];
        return unescape_double_quoted(inner);
    }

    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        let inner = &value[1..value.len() - 1];
        return inner.replace("''", "'");
    }

    value.to_string()
}

/// Unescape a double-quoted YAML scalar body. Mirrors the escapes Pour's writer
/// produces in `format_scalar`, in reverse.
fn unescape_double_quoted(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                // Unknown escape — keep the following char literally.
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
