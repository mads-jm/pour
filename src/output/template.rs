use crate::config::ModuleConfig;
use crate::output::{CompositeData, render_composite_table};
use crate::visibility::visible_field_indices;
use chrono::{DateTime, Local};
use std::collections::{HashMap, HashSet};

/// What to do with a `{{key}}` placeholder whose key is not present in the
/// variable map.
enum OnUnknown {
    /// Remove the placeholder entirely (path mode — keeps filenames clean).
    Strip,
    /// Leave the placeholder as literal text (append mode — caller can see what
    /// was unresolved).
    Leave,
}

/// Core `{{key}}` substitution kernel shared by both render functions.
///
/// Iterates over `vars` and replaces every `{{key}}` occurrence with the
/// corresponding value.  After all known keys are substituted, remaining
/// unresolved placeholders are handled according to `on_unknown`:
/// - `Strip` — each `{{...}}` span is removed.
/// - `Leave` — unresolved spans are left as-is.
fn substitute_keys(
    template: &str,
    vars: &HashMap<String, String>,
    on_unknown: OnUnknown,
) -> String {
    let mut result = template.to_owned();

    for (key, value) in vars {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }

    if let OnUnknown::Strip = on_unknown {
        while let Some(start) = result.find("{{") {
            if let Some(end) = result[start..].find("}}") {
                result.replace_range(start..start + end + 2, "");
            } else {
                break;
            }
        }
    }

    result
}

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
///
/// `now` is passed explicitly so callers can supply a `captured_at`-derived
/// timestamp (server) or `Local::now()` (TUI) without internal clock calls.
pub fn render_path(
    template: &str,
    field_values: &HashMap<String, String>,
    date_format: Option<&str>,
    now: DateTime<Local>,
) -> String {
    // Step 1: Expand strftime specifiers on the raw template FIRST so that
    // user-supplied field values containing `%` are never passed through chrono.
    let strftime_expanded = now.format(template).to_string();

    // Step 2: Replace special tokens using already-formatted strings.
    // These are resolved after strftime so their output (e.g. "2026-04-01") is
    // treated as literal text and not re-processed.
    let date_fmt = date_format.unwrap_or("%Y%m%d");
    let mut special_vars: HashMap<String, String> = HashMap::new();
    special_vars.insert("date".to_string(), now.format(date_fmt).to_string());
    special_vars.insert("time".to_string(), now.format("%H:%M").to_string());
    // Substitute special tokens first (they don't use Strip — they're known tokens;
    // we use a merged approach: insert specials, then do one substitution pass).
    let after_special = substitute_keys(&strftime_expanded, &special_vars, OnUnknown::Leave);

    // Step 3: Substitute field placeholders with Strip for unknown keys.
    let result = substitute_keys(&after_special, field_values, OnUnknown::Strip);

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
///
/// `now` is passed explicitly so callers can supply a `captured_at`-derived
/// timestamp (server) or `Local::now()` (TUI) without internal clock calls.
pub fn render_append_template(
    template: &str,
    fields: &HashMap<String, String>,
    module: &ModuleConfig,
    composite_data: &CompositeData,
    callout_overrides: &HashMap<String, String>,
    callout_titles: &HashMap<String, String>,
    now: DateTime<Local>,
) -> String {
    // Compute visible field names once; hidden fields render as empty string.
    let visible_indices = visible_field_indices(&module.fields, fields);
    let visible_names: HashSet<&str> = visible_indices
        .iter()
        .map(|&i| module.fields[i].name.as_str())
        .collect();

    // Step 1: Expand strftime specifiers on the raw template FIRST so that
    // user-supplied field values containing `%` are never passed through chrono.
    let strftime_expanded = now.format(template).to_string();

    // Step 2: Replace special tokens — date always uses %Y-%m-%d in append mode.
    let mut special_vars: HashMap<String, String> = HashMap::new();
    special_vars.insert("time".to_string(), now.format("%H:%M").to_string());
    special_vars.insert("date".to_string(), now.format("%Y-%m-%d").to_string());

    // Resolve {{callout}} token from module config or runtime overrides.
    let callout_resolved = callout_overrides
        .get("_callout_type")
        .or(module.callout_type.as_ref());
    if let Some(callout) = callout_resolved {
        special_vars.insert("callout".to_string(), callout.clone());
    }

    let mut result = substitute_keys(&strftime_expanded, &special_vars, OnUnknown::Leave);

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

    // Step 3: Build the resolved variable map for field substitution.
    // Declared fields that are not visible resolve to empty string.
    // Undeclared fields (not in module.fields) are substituted normally.
    // Wikilink and callout block rendering happen here as value transforms.
    let mut resolved_vars: HashMap<String, String> = HashMap::new();
    for (key, value) in fields {
        let field_cfg = module.fields.iter().find(|f| f.name == *key);
        let resolved =
            if declared_names.contains(key.as_str()) && !visible_names.contains(key.as_str()) {
                // Declared field that is currently hidden — clear its placeholder.
                String::new()
            } else if field_cfg.is_some_and(|f| f.wikilink == Some(true)) {
                let list = field_cfg.map(|f| f.list).unwrap_or(false);
                super::apply_wikilink(value.clone(), list)
            } else if let Some(callout) = callout_overrides
                .get(key)
                .or_else(|| field_cfg.and_then(|f| f.callout.as_ref()))
            {
                if value.is_empty() {
                    String::new()
                } else {
                    let title = callout_titles
                        .get(key)
                        .map(|s| s.as_str())
                        .or_else(|| field_cfg.and_then(|f| f.callout_title.as_deref()))
                        .map(str::trim)
                        .filter(|s| !s.is_empty());
                    let mut block = match title {
                        Some(t) => format!("> [!{callout}] {t}"),
                        None => format!("> [!{callout}]"),
                    };
                    for line in value.lines() {
                        block.push_str("\n> ");
                        block.push_str(line);
                    }
                    block
                }
            } else {
                value.clone()
            };
        resolved_vars.insert(key.clone(), resolved);
    }

    // Substitute field placeholders; leave unknowns as-is (Leave mode).
    substitute_keys(&result, &resolved_vars, OnUnknown::Leave)
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
