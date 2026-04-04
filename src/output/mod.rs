pub mod frontmatter;
pub mod template;

use crate::config::{FieldTarget, FieldType, ModuleConfig, SubFieldConfig, WriteMode};
use crate::transport::Transport;
use crate::visibility::visible_field_indices;
use anyhow::{Result, bail};
use std::collections::{HashMap, HashSet};

/// Composite field data: field_name → rows of cell values.
pub type CompositeData = HashMap<String, Vec<Vec<String>>>;

/// Execute a **create** write: generate a new Markdown file with YAML
/// frontmatter and an optional body, then write it via the transport.
///
/// Returns the resolved vault-relative path of the created file.
pub async fn write_create(
    transport: &Transport,
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
    composite_data: &CompositeData,
    date_format: Option<&str>,
) -> Result<String> {
    if module.mode != WriteMode::Create {
        bail!("write_create called on a non-create module");
    }

    let (fm_fields, fm_composites, body_parts) =
        partition_fields(module, field_values, composite_data);

    let frontmatter_block = frontmatter::generate_frontmatter(&fm_fields, &fm_composites);

    let body = body_parts.join("\n\n");

    let mut content = frontmatter_block;
    if !body.is_empty() {
        content.push('\n');
        content.push_str(&body);
        content.push('\n');
    }

    let mut vault_path = template::render_path(&module.path, field_values, date_format);

    // If the resolved path has no file extension, treat it as a directory
    // and auto-generate a timestamped filename for uniqueness.
    if !vault_path.contains('.') {
        let now = chrono::Local::now();
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
pub async fn write_append(
    transport: &Transport,
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
    composite_data: &CompositeData,
    date_format: Option<&str>,
) -> Result<String> {
    if module.mode != WriteMode::Append {
        bail!("write_append called on a non-append module");
    }

    let heading = module.append_under_header.as_deref().unwrap_or("## Log");

    let content = match &module.append_template {
        Some(tmpl) => template::render_append_template(tmpl, field_values, module, composite_data),
        None => {
            // Fallback: join all body-target fields with newlines.
            let (_, _, body_parts) = partition_fields(module, field_values, composite_data);
            body_parts.join("\n")
        }
    };

    let vault_path = template::render_path(&module.path, field_values, date_format);
    transport
        .append_under_heading(&vault_path, heading, &content)
        .await?;

    Ok(vault_path)
}

/// A composite field destined for frontmatter: name, sub-field configs, and row data.
pub type FrontmatterComposite<'a> = (String, &'a [SubFieldConfig], Vec<Vec<String>>);

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
) -> (
    Vec<(String, String)>,
    Vec<FrontmatterComposite<'a>>,
    Vec<String>,
) {
    let mut fm_fields: Vec<(String, String)> = Vec::new();
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
                    let target = field_cfg
                        .target
                        .as_ref()
                        .cloned()
                        .unwrap_or(FieldTarget::Frontmatter);
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
            apply_wikilink(raw)
        } else {
            raw
        };

        let target = field_cfg.target.as_ref().cloned().unwrap_or_else(|| {
            if field_cfg.field_type == FieldType::Textarea {
                FieldTarget::Body
            } else {
                FieldTarget::Frontmatter
            }
        });

        match target {
            FieldTarget::Frontmatter => {
                fm_fields.push((field_cfg.name.clone(), value));
            }
            FieldTarget::Body => {
                if !value.is_empty() {
                    if let Some(ref callout) = field_cfg.callout {
                        // Wrap in Obsidian callout: prefix each line with "> "
                        let mut block = format!("> [!{callout}]");
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
/// Handles comma-separated values by wrapping each item individually,
/// so `"Onyx, Stumptown"` becomes `"[[Onyx]], [[Stumptown]]"`.
/// No-ops on items already wrapped (starts with `[[` and ends with `]]`).
pub fn apply_wikilink(value: String) -> String {
    if value.contains(", ") {
        value
            .split(", ")
            .map(|item| wrap_single_wikilink(item))
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

    // Calculate column widths (minimum: header length)
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let mut out = String::new();

    // Header row
    out.push('|');
    for (i, header) in headers.iter().enumerate() {
        out.push_str(&format!(" {:width$} |", header, width = widths[i]));
    }
    out.push('\n');

    // Separator row
    out.push('|');
    for width in &widths {
        out.push_str(&format!("-{}-|", "-".repeat(*width)));
    }
    out.push('\n');

    // Data rows
    for row in rows {
        out.push('|');
        for (i, width) in widths.iter().enumerate() {
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!(" {:width$} |", cell, width = width));
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}
