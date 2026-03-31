pub mod frontmatter;
pub mod template;

use crate::config::{FieldTarget, FieldType, ModuleConfig, WriteMode};
use crate::transport::Transport;
use anyhow::{Result, bail};
use std::collections::HashMap;

/// Execute a **create** write: generate a new Markdown file with YAML
/// frontmatter and an optional body, then write it via the transport.
///
/// Returns the resolved vault-relative path of the created file.
pub async fn write_create(
    transport: &Transport,
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
) -> Result<String> {
    if module.mode != WriteMode::Create {
        bail!("write_create called on a non-create module");
    }

    let (fm_fields, body_parts) = partition_fields(module, field_values);

    let frontmatter_block = frontmatter::generate_frontmatter(&fm_fields);

    let body = body_parts.join("\n\n");

    let mut content = frontmatter_block;
    if !body.is_empty() {
        content.push('\n');
        content.push_str(&body);
        content.push('\n');
    }

    let vault_path = template::render_path(&module.path);
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
) -> Result<String> {
    if module.mode != WriteMode::Append {
        bail!("write_append called on a non-append module");
    }

    let heading = module.append_under_header.as_deref().unwrap_or("## Log");

    let content = match &module.append_template {
        Some(tmpl) => template::render_append_template(tmpl, field_values),
        None => {
            // Fallback: join all body-target fields with newlines.
            let (_, body_parts) = partition_fields(module, field_values);
            body_parts.join("\n")
        }
    };

    let vault_path = template::render_path(&module.path);
    transport
        .append_under_heading(&vault_path, heading, &content)
        .await?;

    Ok(vault_path)
}

/// Partition field values into frontmatter pairs and body strings.
///
/// Routing rules:
/// - If the field config has an explicit `target`, use it.
/// - Otherwise, `textarea` defaults to body; everything else defaults to
///   frontmatter.
fn partition_fields(
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
) -> (Vec<(String, String)>, Vec<String>) {
    let mut fm_fields: Vec<(String, String)> = Vec::new();
    let mut body_parts: Vec<String> = Vec::new();

    for field_cfg in &module.fields {
        let value = match field_values.get(&field_cfg.name) {
            Some(v) => v.clone(),
            None => continue,
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
                    body_parts.push(value);
                }
            }
        }
    }

    (fm_fields, body_parts)
}
