use crate::config::ModuleConfig;
use crate::output::{CompositeData, render_composite_table};
use chrono::Local;
use std::collections::HashMap;

/// Render a path template by substituting `{{field}}` placeholders and
/// chrono `strftime` specifiers.
///
/// Special tokens (resolved before field lookup):
/// - `{{date}}` — current date in `YYYY-MM-DD` format
/// - `{{time}}` — current time in `HH:MM` format
///
/// Field placeholders (`{{bean}}`, etc.) are replaced with the
/// corresponding value from `field_values`. Unknown placeholders are
/// removed so the path never contains literal `{{…}}` fragments.
///
/// Finally, any chrono `strftime` specifiers (`%Y`, `%m`, `%d`, …) are
/// expanded against the current local time.
///
/// For example, `"Coffee/{{bean}} %Y%m%d.md"` with `bean = "Ethiopian"`
/// becomes `"Coffee/Ethiopian 20260401.md"` on 2026-04-01.
pub fn render_path(
    template: &str,
    field_values: &HashMap<String, String>,
    date_format: Option<&str>,
) -> String {
    let now = Local::now();
    let mut result = template.to_string();

    // Special tokens — {{date}} uses the vault's date_format if configured
    let date_fmt = date_format.unwrap_or("%Y%m%d");
    result = result.replace("{{date}}", &now.format(date_fmt).to_string());
    result = result.replace("{{time}}", &now.format("%H:%M").to_string());

    // Field placeholders
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
    let expanded = now.format(&result).to_string().replace('\\', "/");

    // Sanitize the filename portion (everything after the last `/`) to replace
    // characters that are illegal on Windows filesystems. Directory components
    // are left untouched — only the filename stem + extension are sanitized.
    // This handles cases like {{time}} resolving to "19:30" which contains
    // a colon, illegal on Windows.
    sanitize_path_filename(&expanded)
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
    let mut result = template.to_string();

    // Replace special tokens first so they don't collide with field names.
    result = result.replace("{{time}}", &now.format("%H:%M").to_string());
    result = result.replace("{{date}}", &now.format("%Y-%m-%d").to_string());

    // Replace {{callout}} with the module's configured callout type.
    if let Some(ref callout) = module.callout_type {
        result = result.replace("{{callout}}", callout);
    }

    // Replace composite field placeholders with markdown tables.
    for field_cfg in &module.fields {
        if field_cfg.field_type == crate::config::FieldType::CompositeArray {
            let placeholder = format!("{{{{{}}}}}", field_cfg.name);
            if result.contains(&placeholder)
                && let (Some(subs), Some(rows)) =
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

    // Replace field placeholders, applying wikilink wrapping if configured.
    for (key, value) in fields {
        let placeholder = format!("{{{{{key}}}}}");
        let wrapped = if module
            .fields
            .iter()
            .any(|f| f.name == *key && f.wikilink == Some(true))
        {
            super::apply_wikilink(value.clone())
        } else {
            value.clone()
        };
        result = result.replace(&placeholder, &wrapped);
    }

    // Evaluate any strftime specifiers (e.g. %H:%M, %Y-%m-%d) left in the template.
    now.format(&result).to_string()
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
