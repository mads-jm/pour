pub mod frontmatter;
pub mod template;
pub mod update;

pub use update::{UpdateOutcome, write_update};

use crate::config::{FieldTarget, FieldType, ModuleConfig, SubFieldConfig, WriteMode};
use crate::transport::Transport;
use crate::visibility::visible_field_indices;
use anyhow::{Result, bail};
use chrono::DateTime;
use std::collections::{HashMap, HashSet};
use unicode_width::UnicodeWidthStr;

/// Composite field data: field_name → rows of cell values.
pub type CompositeData = HashMap<String, Vec<Vec<String>>>;

/// Shape of the auto-injected create-mode `date` key when a module does not set
/// `frontmatter_date_format`. Every module emitted exactly this before the key
/// existed, and must keep doing so.
const DEFAULT_FRONTMATTER_DATE_FORMAT: &str = "%Y-%m-%d";

/// Execute a **create** write: generate a new Markdown file with YAML
/// frontmatter and an optional body, then write it via the transport.
///
/// Returns the resolved vault-relative path of the created file.
///
/// `now` is passed explicitly so that `captured_at`-derived timestamps from
/// the server are threaded through correctly, while the TUI passes `Local::now()`.
#[allow(clippy::too_many_arguments)]
pub async fn write_create(
    transport: &Transport,
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
    composite_data: &CompositeData,
    date_format: Option<&str>,
    callout_overrides: &HashMap<String, String>,
    callout_titles: &HashMap<String, String>,
    now: DateTime<chrono::Local>,
) -> Result<String> {
    if module.mode != WriteMode::Create {
        bail!("write_create called on a non-create module");
    }

    let (mut fm_fields, fm_composites, body_parts) = partition_fields(
        module,
        field_values,
        composite_data,
        callout_overrides,
        callout_titles,
    );

    if let Some(ref icon) = module.icon {
        // Only inject if no user field is already named "icon" (avoid duplicate YAML keys).
        if !fm_fields.iter().any(|(k, _, _)| k == "icon") {
            fm_fields.insert(0, ("icon".to_string(), icon.clone(), false));
        }
    }

    if module.daily_link == Some(true) && !fm_fields.iter().any(|(k, _, _)| k == "daily") {
        let date_fmt = date_format.unwrap_or("%Y%m%d");
        let daily = format!("[[{}]]", now.format(date_fmt));
        fm_fields.push(("daily".to_string(), daily, false));
    }

    // Per-module date shape. Absent → `%Y-%m-%d`, byte-identical to the
    // hardcode this replaced, which governed every create-mode module.
    let date_str = now
        .format(
            module
                .frontmatter_date_format
                .as_deref()
                .unwrap_or(DEFAULT_FRONTMATTER_DATE_FORMAT),
        )
        .to_string();

    // Static module frontmatter, minus any key the capture already claimed.
    // Filtering here (rather than in `generate_frontmatter`) keeps the
    // collision policy in one place: this runs after the `icon` and `daily`
    // injections above, so it covers those too. The capture wins — a static is
    // a default, not an override.
    let statics: Vec<(&str, &toml::Value)> = module
        .frontmatter
        .iter()
        .flatten()
        .filter(|(key, _)| !fm_fields.iter().any(|(k, _, _)| k == *key))
        .map(|(key, value)| (key.as_str(), value))
        .collect();

    let frontmatter_block =
        frontmatter::generate_frontmatter(&fm_fields, &fm_composites, &date_str, &statics);

    let body = body_parts.join("\n\n");

    let mut content = frontmatter_block;
    if !body.is_empty() {
        content.push('\n');
        content.push_str(&body);
        content.push('\n');
    }

    let mut vault_path = template::render_path(&module.path, field_values, date_format, now);

    // If the resolved path has no file extension, treat it as a directory
    // and auto-generate a timestamped filename for uniqueness.
    if !vault_path.contains('.') {
        let date_fmt = date_format.unwrap_or("%Y%m%d");
        let date_str = now.format(date_fmt).to_string();
        let time_str = now.format("%H-%M-%S").to_string();
        vault_path = format!(
            "{}/{} {}.md",
            vault_path.trim_end_matches('/'),
            date_str,
            time_str
        );
    }

    transport.create_file(&vault_path, &content).await?;

    Ok(vault_path)
}

/// Execute an **append** write: render the append template and insert it
/// under the configured heading via the transport.
///
/// Returns the resolved vault-relative path of the target file.
///
/// `now` is passed explicitly so that `captured_at`-derived timestamps from
/// the server are threaded through correctly, while the TUI passes `Local::now()`.
#[allow(clippy::too_many_arguments)]
pub async fn write_append(
    transport: &Transport,
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
    composite_data: &CompositeData,
    date_format: Option<&str>,
    callout_overrides: &HashMap<String, String>,
    callout_titles: &HashMap<String, String>,
    now: DateTime<chrono::Local>,
) -> Result<String> {
    if module.mode != WriteMode::Append {
        bail!("write_append called on a non-append module");
    }

    let heading = module.append_under_header.as_deref().unwrap_or("## Log");

    let content = match &module.append_template {
        Some(tmpl) => template::render_append_template(
            tmpl,
            field_values,
            module,
            composite_data,
            callout_overrides,
            callout_titles,
            now,
        ),
        None => {
            // Fallback: join all body-target fields with newlines.
            let (_, _, body_parts) = partition_fields(
                module,
                field_values,
                composite_data,
                callout_overrides,
                callout_titles,
            );
            body_parts.join("\n")
        }
    };

    let vault_path = template::render_path(&module.path, field_values, date_format, now);
    transport
        .append_under_heading(
            &vault_path,
            heading,
            &content,
            module.append_shallow == Some(true),
        )
        .await?;

    Ok(vault_path)
}

/// A composite field destined for frontmatter: name, sub-field configs, and row data.
pub type FrontmatterComposite<'a> = (String, &'a [SubFieldConfig], Vec<Vec<String>>);

/// Scalar frontmatter fields: (key, value, list_flag).
/// `list_flag` enables comma-split into YAML sequence when true.
type FmFields = Vec<(String, String, bool)>;

/// Partition field values into frontmatter pairs, composite frontmatter, and body strings.
///
/// Routing rules:
/// - If the field config has an explicit `target`, use it.
/// - Otherwise, `textarea` defaults to body; everything else defaults to
///   frontmatter.
/// - `composite_array` fields default to frontmatter.
fn partition_fields<'a>(
    module: &'a ModuleConfig,
    field_values: &HashMap<String, String>,
    composite_data: &CompositeData,
    callout_overrides: &HashMap<String, String>,
    callout_titles: &HashMap<String, String>,
) -> (FmFields, Vec<FrontmatterComposite<'a>>, Vec<String>) {
    let mut fm_fields: FmFields = Vec::new();
    let mut fm_composites: Vec<FrontmatterComposite<'a>> = Vec::new();
    let mut body_parts: Vec<String> = Vec::new();

    let visible_indices = visible_field_indices(&module.fields, field_values);
    let visible_names: HashSet<&str> = visible_indices
        .iter()
        .map(|&i| module.fields[i].name.as_str())
        .collect();

    for field_cfg in &module.fields {
        if !visible_names.contains(field_cfg.name.as_str()) {
            continue;
        }
        // Composite array fields
        if field_cfg.field_type == FieldType::CompositeArray {
            if let (Some(subs), Some(rows)) =
                (&field_cfg.sub_fields, composite_data.get(&field_cfg.name))
            {
                // Strip empty rows
                let non_empty: Vec<Vec<String>> = rows
                    .iter()
                    .filter(|row| row.iter().any(|cell| !cell.trim().is_empty()))
                    .cloned()
                    .collect();

                if !non_empty.is_empty() {
                    // composite_array defaults to frontmatter
                    let target = field_cfg.effective_target();
                    match target {
                        FieldTarget::Frontmatter => {
                            // Emit to both frontmatter (YAML array for Dataview)
                            // and body (markdown table for readability).
                            let table = render_composite_table(subs, &non_empty);
                            if !table.is_empty() {
                                body_parts.push(table);
                            }
                            fm_composites.push((field_cfg.name.clone(), subs, non_empty));
                        }
                        FieldTarget::Body => {
                            // Body-only: render as markdown table
                            let table = render_composite_table(subs, &non_empty);
                            if !table.is_empty() {
                                body_parts.push(table);
                            }
                        }
                    }
                }
            }
            continue;
        }

        let raw = match field_values.get(&field_cfg.name) {
            Some(v) => v.clone(),
            None => continue,
        };

        let value = if field_cfg.wikilink == Some(true) {
            apply_wikilink(raw, field_cfg.list)
        } else {
            raw
        };

        let target = field_cfg.effective_target();

        match target {
            FieldTarget::Frontmatter => {
                fm_fields.push((field_cfg.name.clone(), value, field_cfg.list));
            }
            FieldTarget::Body => {
                if !value.is_empty() {
                    let callout = callout_overrides
                        .get(&field_cfg.name)
                        .or(field_cfg.callout.as_ref());
                    if let Some(callout) = callout {
                        let title = callout_titles
                            .get(&field_cfg.name)
                            .map(|s| s.as_str())
                            .or(field_cfg.callout_title.as_deref())
                            .map(str::trim)
                            .filter(|s| !s.is_empty());
                        // Wrap in Obsidian callout: prefix each line with "> "
                        let mut block = match title {
                            Some(t) => format!("> [!{callout}] {t}"),
                            None => format!("> [!{callout}]"),
                        };
                        for line in value.lines() {
                            block.push_str("\n> ");
                            block.push_str(line);
                        }
                        body_parts.push(block);
                    } else {
                        body_parts.push(value);
                    }
                }
            }
        }
    }

    (fm_fields, fm_composites, body_parts)
}

/// Wrap a value in Obsidian wikilink syntax: `[[value]]`.
///
/// When `list` is `true`, comma-separated values are wrapped individually:
/// `"Onyx, Stumptown"` becomes `"[[Onyx]], [[Stumptown]]"`.
/// When `list` is `false` (default), the whole value is wrapped as a single
/// wikilink: `"Onyx, Stumptown"` becomes `"[[Onyx, Stumptown]]"`.
/// No-ops on items already wrapped (starts with `[[` and ends with `]]`).
pub fn apply_wikilink(value: String, list: bool) -> String {
    if list && value.contains(", ") {
        value
            .split(", ")
            .map(wrap_single_wikilink)
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        wrap_single_wikilink(&value)
    }
}

fn wrap_single_wikilink(value: &str) -> String {
    if value.starts_with("[[") && value.ends_with("]]") {
        value.to_string()
    } else {
        format!("[[{value}]]")
    }
}

/// Render composite rows as a markdown table.
pub fn render_composite_table(sub_fields: &[SubFieldConfig], rows: &[Vec<String>]) -> String {
    if rows.is_empty() || sub_fields.is_empty() {
        return String::new();
    }

    let headers: Vec<&str> = sub_fields.iter().map(|s| s.prompt.as_str()).collect();

    // Calculate column widths using display width (handles CJK and wide chars).
    let mut widths: Vec<usize> = headers.iter().map(|h| UnicodeWidthStr::width(*h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }
    }

    let mut out = String::new();

    // Header row — pad with spaces to display width.
    out.push('|');
    for (i, header) in headers.iter().enumerate() {
        let w = widths[i];
        let display_w = UnicodeWidthStr::width(*header);
        let padding = w.saturating_sub(display_w);
        out.push_str(&format!(" {}{} |", header, " ".repeat(padding)));
    }
    out.push('\n');

    // Separator row
    out.push('|');
    for width in &widths {
        out.push_str(&format!("-{}-|", "-".repeat(*width)));
    }
    out.push('\n');

    // Data rows — pad with spaces to display width.
    for row in rows {
        out.push('|');
        for (i, width) in widths.iter().enumerate() {
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            let display_w = UnicodeWidthStr::width(cell);
            let padding = width.saturating_sub(display_w);
            out.push_str(&format!(" {}{} |", cell, " ".repeat(padding)));
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}
