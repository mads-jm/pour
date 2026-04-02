use crate::config::{SubFieldConfig, SubFieldType};
use crate::output::FrontmatterComposite;
use chrono::Local;

/// Characters that require a YAML value to be quoted.
const YAML_SPECIAL: &[char] = &[
    ':', '#', '{', '}', '[', ']', ',', '&', '*', '?', '|', '<', '>', '=', '!', '%', '@', '`', '"',
    '\'',
];

/// Characters that only require quoting when they appear at the start of a value.
const YAML_SPECIAL_START: &[char] = &['-'];

/// Generate YAML frontmatter from scalar key-value pairs and composite fields.
///
/// Rules:
/// - Empty values are skipped.
/// - A `date` field is auto-injected (today, `YYYY-MM-DD`) if not already
///   present, and is always placed first.
/// - Values containing YAML-special characters are double-quoted.
/// - Comma-separated values (e.g. `"a, b, c"`) are emitted as a YAML list.
/// - Composite fields are emitted as YAML sequence-of-mappings.
pub fn generate_frontmatter(
    fields: &[(String, String)],
    composites: &[FrontmatterComposite<'_>],
) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Check whether the caller already supplied a date field.
    let has_date = fields.iter().any(|(k, _)| k == "date");

    // Date always comes first.
    if !has_date {
        let today = Local::now().format("%Y-%m-%d").to_string();
        lines.push(format!("date: {today}"));
    }

    for (key, value) in fields {
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

        // Comma-separated → YAML list.
        if value.contains(", ") {
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

/// Format a single scalar value, quoting if necessary.
fn format_scalar(value: &str) -> String {
    if needs_quoting(value) {
        // Escape any existing double-quotes inside the value.
        let escaped = value.replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

/// Format a value for a `key: value` line.
fn format_value(value: &str) -> String {
    format_scalar(value)
}

/// Determine whether a YAML scalar needs quoting.
fn needs_quoting(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    if value.starts_with(YAML_SPECIAL_START) {
        return true;
    }
    value.chars().any(|c| YAML_SPECIAL.contains(&c))
}
