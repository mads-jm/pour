use crate::config::ModuleConfig;
use crate::output::{CompositeData, render_composite_table};
use crate::visibility::visible_field_indices;
use chrono::Local;
use std::collections::{HashMap, HashSet};

/// Render a path template by substituting `{{field}}` placeholders and
/// chrono `strftime` specifiers.
///
/// Special tokens (resolved before field lookup):
/// - `{{date}}` — current date in `YYYY-MM-DD` format
/// - `{{time}}` — current time in `HH:MM` format
///
/// Processing order (prevents `%` in user values from corrupting output):
/// 1. Expand strftime specifiers (`%Y`, `%m`, `%d`, …) on the raw template.
/// 2. Replace special tokens (`{{date}}`, `{{time}}`).
/// 3. Substitute field placeholders (`{{bean}}`, etc.) from `field_values`.
///    Unknown placeholders are removed so the path stays clean.
///
/// For example, `"Coffee/{{bean}} %Y%m%d.md"` with `bean = "Ethiopian"`
/// becomes `"Coffee/Ethiopian 20260401.md"` on 2026-04-01.
pub fn render_path(
    template: &str,
    field_values: &HashMap<String, String>,
    date_format: Option<&str>,
) -> String {
    let now = Local::now();

    // Step 1: Expand strftime specifiers on the raw template FIRST so that
    // user-supplied field values containing `%` are never passed through chrono.
    let strftime_expanded = now.format(template).to_string();
    let mut result = strftime_expanded;

    // Step 2: Replace special tokens using already-formatted strings.
    // These are resolved after strftime so their output (e.g. "2026-04-01") is
    // treated as literal text and not re-processed.
    let date_fmt = date_format.unwrap_or("%Y%m%d");
    result = result.replace("{{date}}", &now.format(date_fmt).to_string());
    result = result.replace("{{time}}", &now.format("%H:%M").to_string());

    // Step 3: Substitute field placeholders. Values are already-resolved strings
    // that will never be seen by chrono.
    for (key, value) in field_values {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }

    // Strip any remaining unresolved placeholders so the path stays clean.
    while let Some(start) = result.find("{{") {
        if let Some(end) = result[start..].find("}}") {
            result.replace_range(start..start + end + 2, "");
        } else {
            break;
        }
    }

    // Normalize to forward slashes so the API transport receives a consistent
    // vault-relative path, and PathBuf::join on Windows can handle it cleanly
    // when the fs transport joins against a backslash-style base path.
    let normalized = result.replace('\\', "/");

    // Sanitize the filename portion (everything after the last `/`) to replace
    // characters that are illegal on Windows filesystems. Directory components
    // are left untouched — only the filename stem + extension are sanitized.
    // This handles cases like {{time}} resolving to "19:30" which contains
    // a colon, illegal on Windows.
    sanitize_path_filename(&normalized)
}

/// Render an append-mode template by replacing `{{field}}` placeholders
/// with values from the supplied map.
///
/// Special tokens:
/// - `{{time}}` — current time in `HH:MM` format
/// - `{{date}}` — current date in `YYYY-MM-DD` format
///
/// Composite fields (`composite_array`) are expanded as markdown tables
/// when their `{{field_name}}` placeholder appears in the template.
///
/// Placeholders whose key is not found in `fields` (and is not a special
/// token) are left as-is so the caller can see what was unresolved.
pub fn render_append_template(
    template: &str,
    fields: &HashMap<String, String>,
    module: &ModuleConfig,
    composite_data: &CompositeData,
) -> String {
    let now = Local::now();

    // Compute visible field names once; hidden fields render as empty string.
    let visible_indices = visible_field_indices(&module.fields, fields);
    let visible_names: HashSet<&str> = visible_indices
        .iter()
        .map(|&i| module.fields[i].name.as_str())
        .collect();

    // Step 1: Expand strftime specifiers on the raw template FIRST so that
    // user-supplied field values containing `%` are never passed through chrono.
    let strftime_expanded = now.format(template).to_string();
    let mut result = strftime_expanded;

    // Step 2: Replace special tokens using already-formatted strings.
    result = result.replace("{{time}}", &now.format("%H:%M").to_string());
    result = result.replace("{{date}}", &now.format("%Y-%m-%d").to_string());

    // Replace {{callout}} with the module's configured callout type.
    if let Some(ref callout) = module.callout_type {
        result = result.replace("{{callout}}", callout);
    }

    // Replace composite field placeholders with markdown tables.
    // If the field is not visible, replace its placeholder with empty string.
    for field_cfg in &module.fields {
        if field_cfg.field_type == crate::config::FieldType::CompositeArray {
            let placeholder = format!("{{{{{}}}}}", field_cfg.name);
            if result.contains(&placeholder) {
                if !visible_names.contains(field_cfg.name.as_str()) {
                    result = result.replace(&placeholder, "");
                } else if let (Some(subs), Some(rows)) =
                    (&field_cfg.sub_fields, composite_data.get(&field_cfg.name))
                {
                    // Strip empty rows
                    let non_empty: Vec<Vec<String>> = rows
                        .iter()
                        .filter(|row| row.iter().any(|cell| !cell.trim().is_empty()))
                        .cloned()
                        .collect();
                    let table = render_composite_table(subs, &non_empty);
                    result = result.replace(&placeholder, &table);
                }
            }
        }
    }

    // Build a set of all declared field names so we can distinguish "declared
    // but hidden" from "not declared in this module at all".
    let declared_names: HashSet<&str> = module.fields.iter().map(|f| f.name.as_str()).collect();

    // Step 3: Substitute field placeholders. Values are already-resolved strings
    // that will never be seen by chrono.
    // Declared fields that are not visible resolve to empty string.
    // Undeclared fields (not in module.fields) are substituted normally.
    for (key, value) in fields {
        let placeholder = format!("{{{{{key}}}}}");
        let resolved = if declared_names.contains(key.as_str())
            && !visible_names.contains(key.as_str())
        {
            // Declared field that is currently hidden — clear its placeholder.
            String::new()
        } else if module
            .fields
            .iter()
            .any(|f| f.name == *key && f.wikilink == Some(true))
        {
            super::apply_wikilink(value.clone())
        } else {
            value.clone()
        };
        result = result.replace(&placeholder, &resolved);
    }

    result
}

/// Sanitize the filename portion of a vault-relative path.
///
/// Splits on the last `/`, sanitizes only the filename part by replacing
/// characters illegal on Windows (`?`, `*`, `<`, `>`, `|`, `"`, `:`) with `-`,
/// and collapses consecutive dashes. Directory components are preserved as-is.
///
/// Note: `\` and `/` are NOT replaced here — they are path separators, not
/// part of the filename. The input should already be forward-slash normalized.
fn sanitize_path_filename(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => {
            let dir = &path[..=pos];
            let filename = &path[pos + 1..];
            format!("{dir}{}", sanitize_filename_chars(filename))
        }
        None => sanitize_filename_chars(path),
    }
}

/// Replace filesystem-illegal characters in a filename with `-` and collapse
/// consecutive dashes.
fn sanitize_filename_chars(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            ':' | '?' | '*' | '<' | '>' | '|' | '"' => '-',
            _ => c,
        })
        .collect();

    // Collapse consecutive dashes
    let mut result = String::with_capacity(sanitized.len());
    let mut prev_dash = false;
    for c in sanitized.chars() {
        if c == '-' {
            if !prev_dash {
                result.push(c);
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    result
}
