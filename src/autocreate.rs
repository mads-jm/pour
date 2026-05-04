/// Auto-creation of bare notes for novel `dynamic_select` values.
///
/// When `allow_create = true` on a `dynamic_select` field and the submitted
/// value is not present in the existing options list (case-insensitive), Pour
/// creates a minimal note at `{source}/{sanitized_value}.md` before writing
/// the main module output.
use crate::config::{FieldConfig, FieldType, ModuleConfig, TemplateConfig};
use crate::data::cache::Cache;
use crate::output::frontmatter::format_value;
use crate::transport::Transport;
use chrono::{DateTime, Local};
use std::collections::HashMap;

/// A record of a note that was auto-created during form submission.
#[derive(Debug, Clone)]
pub struct AutoCreatedNote {
    /// Vault-relative path of the created note (e.g. `beans/Ethiopia Guji.md`).
    pub vault_path: String,
    /// The display value the user typed (unsanitized).
    pub value: String,
}

/// Windows reserved device names that cannot be used as filenames.
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Sanitize a user-typed value into a safe filename stem.
///
/// - Replaces characters invalid in cross-platform filenames with `-`.
/// - Collapses consecutive `-` into one.
/// - Trims leading/trailing whitespace and `-`.
/// - Rejects Windows reserved device names.
/// - Returns `None` if the result is empty or reserved.
pub fn sanitize_filename(value: &str) -> Option<String> {
    const INVALID: &[char] = &[':', '?', '*', '<', '>', '|', '"', '\\', '/'];

    let sanitized: String = value
        .trim()
        .chars()
        .map(|c| if INVALID.contains(&c) { '-' } else { c })
        .collect();

    // Collapse consecutive dashes
    let collapsed = collapse_dashes(&sanitized);
    let trimmed = collapsed.trim_matches('-').to_string();

    if trimmed.is_empty() {
        return None;
    }

    // Reject Windows reserved device names (case-insensitive, with or without extension)
    let upper = trimmed.to_uppercase();
    if WINDOWS_RESERVED.contains(&upper.as_str()) {
        return None;
    }

    Some(trimmed)
}

fn collapse_dashes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
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

/// Check whether `value` already exists in `options` using a
/// case-insensitive comparison.
pub fn is_existing_option(value: &str, options: &[String]) -> bool {
    let lower = value.trim().to_lowercase();
    options.iter().any(|o| o.trim().to_lowercase() == lower)
}

/// Build the minimal frontmatter content for an auto-created note.
///
/// Format:
/// ```markdown
/// ---
/// date: YYYY-MM-DD
/// ---
/// ```
pub fn build_note_content(date: &str) -> String {
    format!("---\ndate: {date}\n---\n")
}

/// Derive the vault-relative path for an auto-created note.
///
/// `source` is the directory path (e.g. `beans`), `filename_stem` is the
/// sanitized value. Returns `{source}/{filename_stem}.md`.
pub fn note_vault_path(source: &str, filename_stem: &str) -> String {
    let source = source.trim_end_matches('/');
    format!("{source}/{filename_stem}.md")
}

/// Run auto-creation for all eligible fields in the module.
///
/// For each `dynamic_select` field with `allow_create = true`:
/// 1. Retrieve the submitted value and the existing options.
/// 2. Skip if the value is empty or already exists (case-insensitive).
/// 3. Sanitize the value into a safe filename.
/// 4. Create the note via the transport layer (best-effort: log and continue on failure).
/// 5. Append the new value to the cache for the field's source.
///
/// Diagnostic messages (skips, failures) are appended to `deferred_stderr` rather
/// than printed directly, because this function runs while the TUI raw mode is active.
/// The caller is responsible for draining and printing those messages after restoring
/// the terminal.
///
/// Returns a list of notes that were successfully created.
pub async fn run(
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
    field_options: &HashMap<String, Vec<String>>,
    transport: &Transport,
    cache: &mut Cache,
    today: &str,
    deferred_stderr: &mut Vec<String>,
) -> Vec<AutoCreatedNote> {
    let mut created = Vec::new();

    for field in &module.fields {
        if !is_eligible(field) {
            continue;
        }

        let source = match field.source.as_deref() {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };

        let value = match field_values.get(&field.name) {
            Some(v) if !v.trim().is_empty() => v.as_str(),
            _ => continue,
        };

        let existing = field_options
            .get(&field.name)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        // Check both the raw value and its sanitized form against existing options.
        // This prevents re-creation when a user types "foo:bar" and "foo-bar" already
        // exists from a previous auto-create.
        let stem = match sanitize_filename(value) {
            Some(s) => s,
            None => {
                deferred_stderr.push(format!(
                    "pour: auto-create skipped — empty or reserved filename after sanitization (field '{}', value '{}')",
                    field.name, value
                ));
                continue;
            }
        };

        if is_existing_option(value, existing) || is_existing_option(&stem, existing) {
            continue;
        }

        let vault_path = note_vault_path(source, &stem);
        let content = build_note_content(today);

        match transport.create_file(&vault_path, &content).await {
            Ok(()) => {
                // Update cache: append new stem to the source's cached list.
                let mut cached = cache.get(source).unwrap_or_default();
                if !is_existing_option(&stem, &cached) {
                    cached.push(stem.clone());
                    cache.set(source, cached);
                }

                created.push(AutoCreatedNote {
                    vault_path,
                    value: value.to_string(),
                });
            }
            Err(e) => {
                deferred_stderr.push(format!("pour: auto-create failed for '{vault_path}': {e}"));
            }
        }
    }

    created
}

/// Build YAML frontmatter content for a template-driven auto-created note.
///
/// Produces frontmatter with:
/// 1. `date` — always first
/// 2. `name` — the raw user-typed value, always second
/// 3. Each template field in declaration order — using the submitted value,
///    falling back to the field's default. Fields with neither are omitted.
pub fn build_templated_note_content(
    template: &TemplateConfig,
    name: &str,
    field_values: &HashMap<String, String>,
    today: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("date: {}", format_value(today)));
    lines.push(format!("name: {}", format_value(name)));

    for field in &template.fields {
        let value = field_values
            .get(&field.name)
            .filter(|v| !v.is_empty())
            .or(field.default.as_ref());

        if let Some(val) = value {
            lines.push(format!("{}: {}", field.name, format_value(val)));
        }
    }

    lines.push("---".to_string());
    let mut result = lines.join("\n");
    result.push('\n');
    result
}

/// Resolve a template path pattern into a concrete vault-relative path.
///
/// Substitutes `{{name}}` with the sanitized filename stem and expands
/// strftime specifiers (`%Y`, `%m`, etc.) against `now`.
/// Returns `None` if filename sanitization fails.
///
/// `now` is passed explicitly so callers can supply a `captured_at`-derived
/// timestamp (server) or `Local::now()` (TUI) without internal clock calls.
pub fn resolve_template_path(
    template_path: &str,
    name: &str,
    now: DateTime<Local>,
) -> Option<String> {
    let stem = sanitize_filename(name)?;
    // Expand strftime BEFORE substituting {{name}} to prevent user-typed
    // percent sequences (e.g. "Ethiopia %Y") from being interpreted as
    // format specifiers.
    let expanded = now.format(template_path).to_string().replace('\\', "/");
    let resolved = expanded.replace("{{name}}", &stem);
    Some(resolved)
}

/// Whether this field is eligible for auto-creation.
fn is_eligible(field: &FieldConfig) -> bool {
    field.field_type == FieldType::DynamicSelect && field.allow_create == Some(true)
}
