use crate::config::{SubFieldConfig, SubFieldType};
use crate::output::FrontmatterComposite;
use std::collections::BTreeMap;

/// Characters that require a YAML value to be quoted.
const YAML_SPECIAL: &[char] = &[
    ':', '#', '{', '}', '[', ']', ',', '&', '*', '?', '|', '<', '>', '=', '!', '%', '@', '`', '"',
    '\'',
];

/// Characters that only require quoting when they appear at the start of a value.
const YAML_SPECIAL_START: &[char] = &['-'];

/// Generate YAML frontmatter from scalar key-value pairs and composite fields.
///
/// `date_str` is the pre-formatted `YYYY-MM-DD` date string derived from the
/// caller's `now` timestamp (server: `captured_at`-derived; TUI: `Local::now()`).
/// This avoids calling `Local::now()` internally so the frontmatter date honours
/// the `captured_at` contract (§10).
///
/// Rules:
/// - Empty values are skipped.
/// - A `date` field is auto-injected (today, `YYYY-MM-DD`) if not already
///   present, and is always placed first.
/// - Values containing YAML-special characters are double-quoted.
/// - When the field's `list` flag is `true`, comma-separated values
///   (e.g. `"a, b, c"`) are emitted as a YAML sequence. Otherwise the value
///   is treated as a literal string and properly escaped.
/// - Composite fields are emitted as YAML sequence-of-mappings.
/// - `statics` (`[modules.<n>.frontmatter]`) are emitted after the captured
///   fields and before composites, exactly as written in the config — arrays
///   become YAML block sequences, scalars become scalars. The caller is
///   responsible for having already dropped any static whose key collides with
///   a captured field (see `write_create`), since duplicate YAML keys are
///   invalid.
pub fn generate_frontmatter(
    fields: &[(String, String, bool)],
    composites: &[FrontmatterComposite<'_>],
    date_str: &str,
    statics: &[(&str, &toml::Value)],
) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Check whether the caller already supplied a date field.
    let has_date = fields.iter().any(|(k, _, _)| k == "date");

    // Date always comes first.
    if !has_date {
        lines.push(format!("date: {date_str}"));
    }

    for (key, value, list) in fields {
        if value.is_empty() {
            continue;
        }

        // If the key is "date", emit it first (already handled above if missing).
        if key == "date" {
            // Insert date at the front so it stays first.
            let formatted = format_value(value);
            lines.insert(0, format!("date: {formatted}"));
            continue;
        }

        // Comma-separated → YAML list only when the field opts in via `list = true`.
        if *list && value.contains(", ") {
            let items: Vec<&str> = value.split(", ").collect();
            lines.push(format!("{key}:"));
            for item in items {
                let formatted = format_scalar(item);
                lines.push(format!("  - {formatted}"));
            }
        } else {
            let formatted = format_value(value);
            lines.push(format!("{key}: {formatted}"));
        }
    }

    // Static module frontmatter → scalars and block sequences.
    //
    // This cannot reuse the `list = true` path above: that only emits a
    // sequence when the value `contains(", ")`, so a single-element
    // `tags = ["lyra"]` would come out as the scalar `tags: lyra`. An array in
    // config means a sequence in YAML, whatever its length.
    for (key, value) in statics {
        match value {
            toml::Value::Array(items) => {
                if items.is_empty() {
                    continue;
                }
                lines.push(format!("{key}:"));
                for item in items {
                    lines.push(format!("  - {}", format_toml_scalar(item)));
                }
            }
            scalar => {
                let formatted = format_toml_scalar(scalar);
                if formatted.is_empty() {
                    continue;
                }
                lines.push(format!("{key}: {formatted}"));
            }
        }
    }

    // Composite fields → YAML sequence-of-mappings
    for (key, sub_fields, rows) in composites {
        if rows.is_empty() {
            continue;
        }
        lines.push(format!("{key}:"));
        for row in rows {
            format_composite_row(sub_fields, row, &mut lines);
        }
    }

    if lines.is_empty() {
        return String::from("---\n---\n");
    }

    let mut out = String::from("---\n");
    for line in &lines {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("---\n");
    out
}

/// Format a single composite row as YAML sequence item with mappings.
///
/// The first sub-field gets `  - key: val`, subsequent get `    key: val`.
fn format_composite_row(sub_fields: &[SubFieldConfig], row: &[String], lines: &mut Vec<String>) {
    for (i, sub) in sub_fields.iter().enumerate() {
        let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
        if cell.is_empty() {
            continue;
        }

        let formatted = if sub.field_type == SubFieldType::Number {
            // Emit numbers unquoted if they parse cleanly
            if cell.trim().parse::<f64>().is_ok() {
                cell.trim().to_string()
            } else {
                format_scalar(cell)
            }
        } else {
            format_scalar(cell)
        };

        if i == 0
            || lines
                .last()
                .is_none_or(|l| !l.starts_with("  - ") && !l.starts_with("    "))
        {
            // First field in row: sequence item prefix
            lines.push(format!("  - {}: {formatted}", sub.name));
        } else {
            // Continuation fields: indented mapping
            lines.push(format!("    {}: {formatted}", sub.name));
        }
    }
}

/// Render a TOML scalar from `[modules.<n>.frontmatter]` as a YAML scalar.
///
/// Numbers and booleans pass through unquoted — they are typed in the config,
/// so they should stay typed in the frontmatter. Strings go through
/// `format_scalar`, which quotes only when YAML would otherwise misread them.
///
/// Arrays, tables, and datetimes are rejected by config validation
/// (`Config::is_frontmatter_value`) and so cannot arrive here; the fallback
/// keeps this function total rather than panicking if that guard ever moves.
fn format_toml_scalar(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => format_scalar(s),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        other => format_scalar(&other.to_string()),
    }
}

/// Format a single scalar value, quoting if necessary.
pub fn format_scalar(value: &str) -> String {
    if needs_quoting(value) {
        // Order matters: backslashes first, then double-quotes, then newlines.
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

/// Format a value for a `key: value` line.
pub fn format_value(value: &str) -> String {
    format_scalar(value)
}

// ── Read-only parse + single-key line patch (`update` mode) ──────────────────
//
// Everything below this line is deliberately *not* a YAML round-trip. Reading
// is safe; re-emitting is what destroys key order, quoting style, and comments.
// So the patcher edits exactly one line's bytes and leaves every other byte of
// the document — frontmatter and body alike — untouched.

/// A frontmatter scalar as `update` mode writes it.
///
/// Typed rather than pre-rendered because the two transports want different
/// encodings of the same value: the filesystem path splices a YAML scalar into
/// a line, while the API path sends a JSON body (`Content-Type: application/json`,
/// per the Local REST API's frontmatter PATCH contract).
#[derive(Debug, Clone, PartialEq)]
pub enum FrontmatterValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

impl FrontmatterValue {
    /// The YAML scalar text spliced into a `key: value` line.
    ///
    /// Numbers and booleans stay bare so Obsidian reads them as typed
    /// properties — `format_value` would quote them (it quotes anything that
    /// parses as a number or matches a YAML reserved word), which is correct
    /// for a `text` capture and wrong for a `counter`.
    pub fn to_yaml(&self) -> String {
        match self {
            FrontmatterValue::Bool(b) => b.to_string(),
            FrontmatterValue::Number(n) => format_number(*n),
            FrontmatterValue::Text(s) => format_value(s),
        }
    }

    /// The JSON body for the API `PATCH … Target-Type: frontmatter` request.
    pub fn to_json(&self) -> String {
        match self {
            FrontmatterValue::Bool(b) => b.to_string(),
            FrontmatterValue::Number(n) => format_number(*n),
            FrontmatterValue::Text(s) => serde_json::Value::String(s.clone()).to_string(),
        }
    }
}

/// Render a number without a trailing `.0` when it is integral.
///
/// Matches the shape a `number` field produces from user input: someone who
/// typed `16` gets `16` back, not `16.0`. Non-integral values keep the shortest
/// round-tripping float form.
pub fn format_number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Byte spans of every line in `s`: `(start, content_end, next_start)`.
///
/// `content_end` excludes the line terminator (both `\n` and `\r\n`);
/// `next_start` includes it. Working in byte offsets — rather than
/// `lines().collect()` and a re-join — is what makes byte-for-byte
/// preservation possible: untouched bytes are copied verbatim, terminators
/// included, so a CRLF note stays CRLF.
fn line_spans(s: &str) -> Vec<(usize, usize, usize)> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let mut end = i;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        let next = if end < bytes.len() { end + 1 } else { end };
        let mut content_end = end;
        if content_end > start && bytes[content_end - 1] == b'\r' {
            content_end -= 1;
        }
        out.push((start, content_end, next));
        i = next;
    }
    out
}

/// Locate the frontmatter block: `(first content byte, byte offset of the
/// closing delimiter line)`.
///
/// The block must open on the very first line, exactly as Obsidian requires.
/// A document whose body merely contains `---` is not a match.
fn frontmatter_span(content: &str) -> Option<(usize, usize)> {
    let lines = line_spans(content);
    let first = *lines.first()?;
    if &content[first.0..first.1] != "---" {
        return None;
    }
    for line in lines.iter().skip(1) {
        let text = &content[line.0..line.1];
        if text == "---" || text == "..." {
            return Some((first.2, line.0));
        }
    }
    None
}

/// Strip one layer of matched YAML quotes and the escapes they imply.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        return value[1..value.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\\\", "\\");
    }
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        return value[1..value.len() - 1].replace("''", "'");
    }
    value.to_string()
}

/// Read the top-level scalar keys of a document's frontmatter block.
///
/// Returns `None` when the document has no frontmatter block at all. Nested
/// mappings, block sequence items, blank lines, and comments are skipped —
/// this exists to answer "what is `water` right now?", not to model YAML.
///
/// Values are returned with surrounding quotes stripped and no type coercion:
/// `water: null` yields `"null"`, and it is the caller's job to decide that a
/// counter reads it as `0`.
pub fn read_frontmatter(content: &str) -> Option<BTreeMap<String, String>> {
    let (start, end) = frontmatter_span(content)?;
    let block = &content[start..end];

    let mut map = BTreeMap::new();
    for (s, ce, _) in line_spans(block) {
        let line = &block[s..ce];
        // Indented lines belong to a parent key; `-` opens a sequence item.
        if line.starts_with(' ') || line.starts_with('\t') || line.starts_with('-') {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.is_empty() {
            continue;
        }
        map.insert(key.trim().to_string(), unquote(value.trim()));
    }
    Some(map)
}

/// Whether a single-key patch replaced an existing line or inserted a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOutcome {
    Replaced,
    /// The key was absent from the frontmatter block and has been added
    /// (spec §2.4 — capture first, then tell the user their template is stale).
    Inserted,
}

/// Why a single-key patch refused to touch the document.
#[derive(Debug, Clone, PartialEq)]
pub enum PatchLineError {
    /// The document has no `---` frontmatter block opening on line 1.
    NoFrontmatterBlock,
    /// The key's current value spans more than one line — a block sequence or
    /// a block scalar. Replacing only the `key:` line would orphan its
    /// continuation lines, so the patch aborts rather than corrupt the note.
    MultilineValue(String),
}

impl std::fmt::Display for PatchLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchLineError::NoFrontmatterBlock => {
                write!(f, "note has no frontmatter block to patch")
            }
            PatchLineError::MultilineValue(key) => write!(
                f,
                "frontmatter key '{key}' holds a multi-line value; refusing to patch it"
            ),
        }
    }
}

impl std::error::Error for PatchLineError {}

/// Replace (or insert) a single top-level frontmatter key, line-level.
///
/// `yaml_value` is spliced verbatim after `key: ` — callers pass
/// [`FrontmatterValue::to_yaml`], which has already done any quoting.
///
/// Guarantees, all pinned by tests: key order is unchanged, every untouched
/// line survives byte-for-byte (including its original terminator and any
/// comment), and the body is never read let alone rewritten. A missing key is
/// appended as the last line of the existing block.
pub fn patch_frontmatter_line(
    content: &str,
    key: &str,
    yaml_value: &str,
) -> Result<(String, PatchOutcome), PatchLineError> {
    let (start, end) = frontmatter_span(content).ok_or(PatchLineError::NoFrontmatterBlock)?;
    let block = &content[start..end];
    let spans = line_spans(block);

    for (i, &(s, ce, _)) in spans.iter().enumerate() {
        let line = &block[s..ce];
        if line.starts_with(' ') || line.starts_with('\t') || line.starts_with('-') {
            continue;
        }
        let Some((line_key, line_value)) = line.split_once(':') else {
            continue;
        };
        if line_key.trim() != key {
            continue;
        }

        // Block scalar (`key: |`, `key: >`) or a sequence/mapping opened on the
        // following lines — either way the value is not this one line.
        let rest = line_value.trim();
        let opens_block = rest.starts_with('|') || rest.starts_with('>');
        let continues = rest.is_empty()
            && spans.get(i + 1).is_some_and(|&(ns, nce, _)| {
                let next = &block[ns..nce];
                next.starts_with(' ') || next.starts_with('\t') || next.starts_with('-')
            });
        if opens_block || continues {
            return Err(PatchLineError::MultilineValue(key.to_string()));
        }

        let mut out = String::with_capacity(content.len() + yaml_value.len());
        out.push_str(&content[..start + s]);
        out.push_str(key);
        out.push_str(": ");
        out.push_str(yaml_value);
        out.push_str(&content[start + ce..]);
        return Ok((out, PatchOutcome::Replaced));
    }

    // Key absent — append it as the block's last line, matching the line
    // terminator the document already uses.
    let terminator = if content[..start].ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out = String::with_capacity(content.len() + key.len() + yaml_value.len() + 4);
    out.push_str(&content[..end]);
    out.push_str(key);
    out.push_str(": ");
    out.push_str(yaml_value);
    out.push_str(terminator);
    out.push_str(&content[end..]);
    Ok((out, PatchOutcome::Inserted))
}

/// YAML bare words that must be quoted to prevent type coercion.
const YAML_RESERVED: &[&str] = &["true", "false", "null", "yes", "no", "on", "off"];

/// Determine whether a YAML scalar needs quoting.
fn needs_quoting(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    // Reserved bare words (case-insensitive).
    if YAML_RESERVED.iter().any(|&w| value.eq_ignore_ascii_case(w)) {
        return true;
    }
    // Numeric-looking strings would be parsed as numbers by a YAML parser.
    if value.trim().parse::<f64>().is_ok() {
        return true;
    }
    if value.starts_with(YAML_SPECIAL_START) {
        return true;
    }
    // Literal newlines or carriage returns require quoting.
    if value.contains('\n') || value.contains('\r') {
        return true;
    }
    value.chars().any(|c| YAML_SPECIAL.contains(&c))
}
