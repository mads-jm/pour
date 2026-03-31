use chrono::Local;
use std::collections::HashMap;

/// Render a path template by substituting chrono `strftime` specifiers.
///
/// For example, `"Journal/%Y/%Y-%m-%d.md"` becomes
/// `"Journal/2026/2026-03-30.md"` when run on 2026-03-30.
pub fn render_path(template: &str) -> String {
    Local::now().format(template).to_string()
}

/// Render an append-mode template by replacing `{{field}}` placeholders
/// with values from the supplied map.
///
/// Special tokens:
/// - `{{time}}` — current time in `HH:MM` format
/// - `{{date}}` — current date in `YYYY-MM-DD` format
///
/// Placeholders whose key is not found in `fields` (and is not a special
/// token) are left as-is so the caller can see what was unresolved.
pub fn render_append_template(template: &str, fields: &HashMap<String, String>) -> String {
    let now = Local::now();
    let mut result = template.to_string();

    // Replace special tokens first so they don't collide with field names.
    result = result.replace("{{time}}", &now.format("%H:%M").to_string());
    result = result.replace("{{date}}", &now.format("%Y-%m-%d").to_string());

    // Replace field placeholders.
    for (key, value) in fields {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }

    result
}
