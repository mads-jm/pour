use crate::config::{SubFieldConfig, SubFieldType};
use crate::output::FrontmatterComposite;

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
