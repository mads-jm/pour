/// Step 3 — Auto-create novel `dynamic_select` values (§6.4, best-effort).
///
/// For each visible `dynamic_select` field with `allow_create = true`:
///   1. Fetch existing options (API → disk scan → cache fallback).
///   2. If the submitted value is novel, create a stub or templated note.
///   3. On transport error, push a warning — auto-create failure is
///      non-fatal and must not block the main write.
///
/// Populates `ctx.auto_created`, `ctx.post_create_commands`, and
/// `ctx.warnings` on the `SubmitContext`.
use axum::http::StatusCode;
use axum::response::Response;
use serde_json::json;

use super::SubmitContext;
use crate::config::FieldType;
use crate::data::cache::Cache;
use crate::server::{
    AppState,
    dto::{
        AutoCreatedDto, PostCreateCommandDto, SubmitWarningDto, error_codes,
        error_response_with_details, extra_error_codes,
    },
};

/// Run auto-create for all eligible fields.
///
/// Returns `Err(Response)` only for hard validation failures (e.g.
/// `create_template` set + novel value + no `auto_create_inputs` provided).
/// Transport errors are converted to warnings.
pub(super) async fn run(ctx: &mut SubmitContext, state: &AppState) -> Result<(), Response> {
    let module_key = &ctx.module_key;
    let module = state
        .config
        .modules
        .get(module_key)
        .expect("module already validated in validate step");

    let today = ctx.now_local.format("%Y-%m-%d").to_string();
    let mut cache = Cache::load();

    for field in &module.fields {
        if field.field_type != FieldType::DynamicSelect || field.allow_create != Some(true) {
            continue;
        }
        if !ctx.visible_names.contains(&field.name) {
            continue;
        }

        let value = match ctx.field_values.get(&field.name) {
            Some(v) if !v.trim().is_empty() => v.clone(),
            _ => continue,
        };

        let source = match field.source.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => s,
            None => continue,
        };

        // Fetch existing options; update cache as a side effect.
        let existing = if let Ok(items) = state.transport.list_directory(source).await
            && !items.is_empty()
        {
            let norm = normalize_options(items);
            cache.set(source, norm.clone());
            norm
        } else {
            cache.get(source).unwrap_or_default()
        };

        let stem = match crate::autocreate::sanitize_filename(&value) {
            Some(s) => s,
            None => continue,
        };

        if crate::autocreate::is_existing_option(&value, &existing)
            || crate::autocreate::is_existing_option(&stem, &existing)
        {
            continue;
        }

        // Novel value — choose creation path.
        let has_template = field.create_template.is_some();
        let sub_inputs = ctx.auto_create_inputs.get(&field.name);

        if has_template {
            let template_name = field.create_template.as_deref().unwrap();

            match sub_inputs {
                None => {
                    return Err(error_response_with_details(
                        StatusCode::BAD_REQUEST,
                        error_codes::VALIDATION_FAILED,
                        "auto_create_inputs required for templated create_template field with novel value.",
                        json!({
                            "code": extra_error_codes::AUTO_CREATE_INPUT_REQUIRED,
                            "fields": [{ "field": field.name, "code": "auto_create_input_required" }]
                        }),
                    ));
                }
                Some(inputs) => {
                    let template = match state
                        .config
                        .templates
                        .as_ref()
                        .and_then(|ts| ts.get(template_name))
                    {
                        Some(t) => t,
                        None => {
                            ctx.warnings.push(SubmitWarningDto {
                                code: "autocreate_failed",
                                field: Some(field.name.clone()),
                                message: format!("template '{template_name}' not found"),
                            });
                            continue;
                        }
                    };

                    let vault_path = match crate::autocreate::resolve_template_path(
                        &template.path,
                        &value,
                        ctx.now_local,
                    ) {
                        Some(p) => p,
                        None => {
                            ctx.warnings.push(SubmitWarningDto {
                                code: "autocreate_failed",
                                field: Some(field.name.clone()),
                                message: "failed to sanitize filename".to_string(),
                            });
                            continue;
                        }
                    };

                    let content = crate::autocreate::build_templated_note_content(
                        template, &value, inputs, &today,
                    );

                    match state.transport.create_file(&vault_path, &content).await {
                        Ok(()) => {
                            let mut cached = cache.get(source).unwrap_or_default();
                            if !crate::autocreate::is_existing_option(&stem, &cached) {
                                cached.push(stem.clone());
                                cache.set(source, cached);
                            }

                            let fired = if let Some(ref cmd) = field.post_create_command {
                                match state.transport.execute_command(cmd).await {
                                    Ok(()) => {
                                        state.transport_mode == crate::transport::TransportMode::Api
                                    }
                                    Err(_) => false,
                                }
                            } else {
                                false
                            };

                            if let Some(ref cmd) = field.post_create_command {
                                ctx.post_create_commands.push(PostCreateCommandDto {
                                    field: field.name.clone(),
                                    command: cmd.clone(),
                                    fired,
                                });
                            }

                            ctx.auto_created.push(AutoCreatedDto {
                                field: field.name.clone(),
                                value: value.clone(),
                                vault_path,
                                templated: true,
                            });
                        }
                        Err(e) => {
                            tracing::warn!(
                                field = %field.name,
                                error = %e,
                                "autocreate file creation failed"
                            );
                            ctx.warnings.push(SubmitWarningDto {
                                code: "autocreate_failed",
                                field: Some(field.name.clone()),
                                message: "failed to create autocreate file".to_string(),
                            });
                        }
                    }
                }
            }
        } else {
            // Bare stub path.
            let vault_path = crate::autocreate::note_vault_path(source, &stem);
            let content = crate::autocreate::build_note_content(&today);

            match state.transport.create_file(&vault_path, &content).await {
                Ok(()) => {
                    let mut cached = cache.get(source).unwrap_or_default();
                    if !crate::autocreate::is_existing_option(&stem, &cached) {
                        cached.push(stem.clone());
                        cache.set(source, cached);
                    }
                    ctx.auto_created.push(AutoCreatedDto {
                        field: field.name.clone(),
                        value: value.clone(),
                        vault_path,
                        templated: false,
                    });
                }
                Err(e) => {
                    ctx.warnings.push(SubmitWarningDto {
                        code: "autocreate_failed",
                        field: Some(field.name.clone()),
                        message: e.to_string(),
                    });
                }
            }
        }
    }

    // Persist cache after all autocreate operations (best-effort).
    let _ = cache.save();

    Ok(())
}

/// Normalize raw directory-listing items to file stems.
fn normalize_options(items: Vec<String>) -> Vec<String> {
    use std::path::Path;
    items
        .into_iter()
        .filter(|s| !s.ends_with('/'))
        .map(|s| {
            Path::new(&s)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&s)
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
