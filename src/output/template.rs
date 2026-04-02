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
    now.format(&result)
        .to_string()
        .replace('\\', "/")
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

    // Replace composite field placeholders with markdown tables.
    for field_cfg in &module.fields {
        if field_cfg.field_type == crate::config::FieldType::CompositeArray {
            let placeholder = format!("{{{{{}}}}}", field_cfg.name);
            if result.contains(&placeholder)
                && let (Some(subs), Some(rows)) = (&field_cfg.sub_fields, composite_data.get(&field_cfg.name)) {
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

    // Replace field placeholders.
    for (key, value) in fields {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }

    // Evaluate any strftime specifiers (e.g. %H:%M, %Y-%m-%d) left in the template.
    now.format(&result).to_string()
}
