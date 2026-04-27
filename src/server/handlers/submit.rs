/// Handler for `POST /api/v1/submit/{module}` (§6.4).
///
/// The keystone endpoint. Mirrors the TUI's `handle_submit` pipeline:
/// 1. Validate auth (middleware).
/// 2. Content-Type + body-size checks.
/// 3. Idempotency key handling (§9).
/// 4. Module lookup (404 for unknown or mobile_visible=false).
/// 5. captured_at validation and resolution (§10).
/// 6. Field validation (required, number parseable, show_when filtering).
/// 7. Auto-create for novel dynamic_select values (best-effort).
/// 8. Main write (`write_create` / `write_append`).
/// 9. History recording.
/// 10. 201 response with Location header.
use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::RequestExt as _;
use axum::body::to_bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{Local, Utc};
use serde_json::json;

use super::super::{
    AppState,
    dto::{
        AutoCreatedDto, PostCreateCommandDto, SubmitRequest, SubmitResponse, SubmitWarningDto,
        error_codes, error_response, error_response_with_details, extra_error_codes,
    },
    idempotency::IdempotencyOutcome,
    is_length_limit_error,
};
// Logging: module name + vault path + autocreate count on success; module name +
// error code on failure. NO field values, NO composite data, NO user content (§14).
use crate::config::{FieldType, WriteMode};
use crate::data::cache::Cache;
use crate::data::history::History;
use crate::output;
use crate::visibility::visible_field_indices;

/// Maximum age for `captured_at` — 30 days past.
const CAPTURED_AT_MAX_AGE_SECS: i64 = 30 * 24 * 3600;
/// Maximum future skew for `captured_at` — 5 minutes.
const CAPTURED_AT_MAX_FUTURE_SECS: i64 = 5 * 60;

pub async fn handler(
    State(state): State<AppState>,
    Path(module_key): Path<String>,
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    // -- Content-Type check (§4, §6.4) -----------------------------------
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("application/json") {
        return error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            error_codes::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be application/json.",
        );
    }

    // -- Idempotency-Key handling (§9) ------------------------------------
    let idempotency_key = req
        .headers()
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(ref key) = idempotency_key {
        // Validate format: 1–256 ASCII printable characters.
        if key.is_empty() || key.len() > 256 || !key.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
            return error_response(
                StatusCode::BAD_REQUEST,
                error_codes::VALIDATION_FAILED,
                "Idempotency-Key must be 1–256 ASCII printable characters.",
            );
        }

        match state.idempotency.get_or_insert_in_flight(key) {
            IdempotencyOutcome::InFlight => {
                return error_response(
                    StatusCode::CONFLICT,
                    extra_error_codes::IDEMPOTENCY_REPLAY_IN_FLIGHT,
                    "This Idempotency-Key is currently being processed.",
                );
            }
            IdempotencyOutcome::Replay { status, body } => {
                let mut resp = Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("Idempotency-Replay", "true")
                    .body(axum::body::Body::from(body))
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
                resp.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-store"),
                );
                return resp;
            }
            IdempotencyOutcome::Fresh => {
                // Proceed; call complete() at the end.
            }
        }
    }

    // Delegate to inner handler and wrap with idempotency complete().
    let resp = submit_inner(&state, module_key, req).await;

    if let Some(ref key) = idempotency_key {
        // Capture the response bytes to store in cache then re-wrap.
        let (parts, body) = resp.into_parts();
        let bytes = to_bytes(body, usize::MAX).await.unwrap_or_default();
        // Contract §9: only 2xx terminal successes are stored in the idempotency
        // cache. 4xx and 5xx are NOT cached — release the in-flight marker so
        // the client can fix the problem and retry with the same key.
        if parts.status.is_success() {
            state.idempotency.complete(key, parts.status, bytes.to_vec());
        } else {
            state.idempotency.release(key);
        }
        let mut rebuilt = Response::from_parts(parts, axum::body::Body::from(bytes));
        rebuilt.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        );
        rebuilt
    } else {
        resp
    }
}

async fn submit_inner(
    state: &AppState,
    module_key: String,
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    // -- Module lookup (404 for unknown or mobile_visible=false) ----------
    let module = match state.config.modules.get(&module_key) {
        Some(m) if m.is_mobile_visible() => m,
        _ => {
            return error_response(
                StatusCode::NOT_FOUND,
                error_codes::NOT_FOUND,
                "Unknown module.",
            );
        }
    };

    // -- Parse body -------------------------------------------------------
    // Use `into_limited_body()` so the `DefaultBodyLimit` extension (set by
    // the per-route `.layer(DefaultBodyLimit::max(…))`) is honoured when we
    // read the raw body stream. Without this, `DefaultBodyLimit` is silently
    // bypassed because it only applies to axum extractors, not direct
    // `Body::poll_frame` / `to_bytes` calls.
    let body_bytes = match to_bytes(req.into_limited_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            // `into_limited_body()` produces a `LengthLimitError` when the
            // DefaultBodyLimit cap is exceeded. Map that to 413; everything
            // else is a genuine read failure (→ 500).
            if is_length_limit_error(&e) {
                return error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    error_codes::PAYLOAD_TOO_LARGE,
                    "Request body exceeds the 1 MiB limit.",
                );
            }
            return error_response_with_details(
                StatusCode::INTERNAL_SERVER_ERROR,
                error_codes::INTERNAL_ERROR,
                "Failed to read request body.",
                json!({ "engine_error": e.to_string() }),
            );
        }
    };

    let submit: SubmitRequest = match serde_json::from_slice(&body_bytes) {
        Ok(s) => s,
        Err(e) => {
            return error_response_with_details(
                StatusCode::BAD_REQUEST,
                error_codes::VALIDATION_FAILED,
                "Invalid JSON body.",
                json!({ "engine_error": e.to_string() }),
            );
        }
    };

    // -- captured_at resolution (§10) -------------------------------------
    let server_now_utc = Utc::now();
    let (now_utc, now_local) = match submit.captured_at {
        Some(captured) => {
            // Validate window: (now - 30 days) ≤ captured_at ≤ (now + 5 min).
            let age_secs = (server_now_utc - captured).num_seconds();
            let future_secs = (captured - server_now_utc).num_seconds();
            if age_secs > CAPTURED_AT_MAX_AGE_SECS || future_secs > CAPTURED_AT_MAX_FUTURE_SECS {
                return error_response_with_details(
                    StatusCode::BAD_REQUEST,
                    error_codes::VALIDATION_FAILED,
                    "captured_at is outside the allowed range (30 days past, 5 minutes future).",
                    json!({ "code": extra_error_codes::CAPTURED_AT_OUT_OF_RANGE }),
                );
            }
            let local = captured.with_timezone(&Local);
            (captured, local)
        }
        None => {
            let local = server_now_utc.with_timezone(&Local);
            (server_now_utc, local)
        }
    };

    // -- Visibility filtering ---------------------------------------------
    let visible_indices = visible_field_indices(&module.fields, &submit.field_values);
    let visible_names: HashSet<&str> = visible_indices
        .iter()
        .map(|&i| module.fields[i].name.as_str())
        .collect();

    // Filter field_values to visible-only (belt-and-suspenders per §6.4).
    let field_values: HashMap<String, String> = submit
        .field_values
        .into_iter()
        .filter(|(k, _)| visible_names.contains(k.as_str()))
        .collect();

    // -- Field validation -------------------------------------------------
    let mut field_errors: Vec<serde_json::Value> = Vec::new();

    for field in &module.fields {
        if !visible_names.contains(field.name.as_str()) {
            continue;
        }

        let value = field_values.get(&field.name).map(|s| s.as_str()).unwrap_or("");

        // Required check.
        if field.required.unwrap_or(false) && value.trim().is_empty() {
            field_errors.push(json!({ "field": field.name, "code": "required" }));
            continue;
        }

        // Number parsability check.
        if field.field_type == FieldType::Number && !value.trim().is_empty() {
            let ok = value.trim().parse::<i64>().is_ok()
                || value.trim().parse::<f64>().is_ok();
            if !ok {
                field_errors.push(json!({
                    "field": field.name,
                    "code": "invalid_number",
                    "value": value
                }));
            }
        }
    }

    if !field_errors.is_empty() {
        tracing::warn!(
            module = %module_key,
            code = %error_codes::VALIDATION_FAILED,
            "submit failed"
        );
        return error_response_with_details(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Submit rejected because required fields are empty or invalid.",
            json!({ "fields": field_errors }),
        );
    }

    // -- Auto-create (before main write, §6.4) ----------------------------
    let today = now_local.format("%Y-%m-%d").to_string();
    let mut auto_created_dtos: Vec<AutoCreatedDto> = Vec::new();
    let mut post_create_command_dtos: Vec<PostCreateCommandDto> = Vec::new();
    let mut warnings: Vec<SubmitWarningDto> = Vec::new();

    // Load options cache once for auto-create eligibility checks.
    let mut cache = Cache::load();

    for field in &module.fields {
        if field.field_type != FieldType::DynamicSelect || field.allow_create != Some(true) {
            continue;
        }
        if !visible_names.contains(field.name.as_str()) {
            continue;
        }

        let value = match field_values.get(&field.name) {
            Some(v) if !v.trim().is_empty() => v.clone(),
            _ => continue,
        };

        // Fetch existing options to check novelty.
        let source = match field.source.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => s,
            None => continue,
        };

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

        // Novel value — decide which creation path to use.
        let has_template = field.create_template.is_some();
        let sub_inputs = submit.auto_create_inputs.get(&field.name);

        if has_template {
            let template_name = field.create_template.as_deref().unwrap();

            match sub_inputs {
                None => {
                    // create_template set + novel value + no sub-form → reject.
                    return error_response_with_details(
                        StatusCode::BAD_REQUEST,
                        error_codes::VALIDATION_FAILED,
                        "auto_create_inputs required for templated create_template field with novel value.",
                        json!({
                            "code": extra_error_codes::AUTO_CREATE_INPUT_REQUIRED,
                            "fields": [{ "field": field.name, "code": "auto_create_input_required" }]
                        }),
                    );
                }
                Some(inputs) => {
                    // Templated path.
                    let template = match state
                        .config
                        .templates
                        .as_ref()
                        .and_then(|ts| ts.get(template_name))
                    {
                        Some(t) => t,
                        None => {
                            warnings.push(SubmitWarningDto {
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
                        now_local,
                    ) {
                        Some(p) => p,
                        None => {
                            warnings.push(SubmitWarningDto {
                                code: "autocreate_failed",
                                field: Some(field.name.clone()),
                                message: "failed to sanitize filename".to_string(),
                            });
                            continue;
                        }
                    };

                    let content = crate::autocreate::build_templated_note_content(
                        template,
                        &value,
                        inputs,
                        &today,
                    );

                    match state.transport.create_file(&vault_path, &content).await {
                        Ok(()) => {
                            // Cache update.
                            let mut cached = cache.get(source).unwrap_or_default();
                            if !crate::autocreate::is_existing_option(&stem, &cached) {
                                cached.push(stem.clone());
                                cache.set(source, cached);
                            }

                            // post_create_command (best-effort, API transport only).
                            let fired = if let Some(ref cmd) = field.post_create_command {
                                match state.transport.execute_command(cmd).await {
                                    Ok(()) => state.transport_mode == crate::transport::TransportMode::Api,
                                    Err(_) => false,
                                }
                            } else {
                                false
                            };

                            if let Some(ref cmd) = field.post_create_command {
                                post_create_command_dtos.push(PostCreateCommandDto {
                                    field: field.name.clone(),
                                    command: cmd.clone(),
                                    fired,
                                });
                            }

                            auto_created_dtos.push(AutoCreatedDto {
                                field: field.name.clone(),
                                value: value.clone(),
                                vault_path,
                                templated: true,
                            });
                        }
                        Err(e) => {
                            // Log the internal error server-side (no user content in
                            // the log — the error string is filesystem/transport-internal).
                            tracing::warn!(
                                field = %field.name,
                                error = %e,
                                "autocreate file creation failed"
                            );
                            warnings.push(SubmitWarningDto {
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

                    auto_created_dtos.push(AutoCreatedDto {
                        field: field.name.clone(),
                        value: value.clone(),
                        vault_path,
                        templated: false,
                    });
                }
                Err(e) => {
                    warnings.push(SubmitWarningDto {
                        code: "autocreate_failed",
                        field: Some(field.name.clone()),
                        message: e.to_string(),
                    });
                }
            }
        }
    }

    // Persist cache after autocreate operations (best-effort).
    let _ = cache.save();

    // -- Main write -------------------------------------------------------
    let date_fmt = state.config.vault.date_format.as_deref();

    let write_result = match module.mode {
        WriteMode::Create => {
            output::write_create(
                &state.transport,
                module,
                &field_values,
                &submit.composite_data,
                date_fmt,
                &submit.callout_overrides,
                &submit.callout_titles,
                now_local,
            )
            .await
        }
        WriteMode::Append => {
            output::write_append(
                &state.transport,
                module,
                &field_values,
                &submit.composite_data,
                date_fmt,
                &submit.callout_overrides,
                &submit.callout_titles,
                now_local,
            )
            .await
        }
    };

    let vault_path = match write_result {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                module = %module_key,
                code = %error_codes::WRITE_ERROR,
                "submit failed"
            );
            return error_response_with_details(
                StatusCode::INTERNAL_SERVER_ERROR,
                error_codes::WRITE_ERROR,
                "Engine write failed.",
                json!({ "engine_error": e.to_string() }),
            );
        }
    };

    // -- History recording ------------------------------------------------
    // Derive first_field: first non-textarea, non-composite visible field value.
    let first_field = module
        .fields
        .iter()
        .filter(|f| {
            visible_names.contains(f.name.as_str())
                && f.field_type != FieldType::Textarea
                && f.field_type != FieldType::CompositeArray
        })
        .find_map(|f| field_values.get(&f.name))
        .map(|v| v.as_str());

    let history_id = {
        let mut history = History::load();
        match history.record(&module_key, &vault_path, first_field, now_utc) {
            Ok(id) => id,
            Err(e) => {
                // History failure is non-fatal — log as warning and synthesize an id.
                warnings.push(SubmitWarningDto {
                    code: "history_record_failed",
                    field: None,
                    message: format!("history record failed: {e}"),
                });
                // Synthesize a fallback id; no counter needed here since this
                // path is only reached when History::record fails (rare).
                format!("{}-fallback-{}", now_utc.format("%Y%m%dT%H%M%S%3f"), module_key)
            }
        }
    };

    // -- Domain log: submit ok (module + vault path + autocreate count; NO field values §14) ---
    tracing::info!(
        module = %module_key,
        vault_path = %vault_path,
        autocreate = auto_created_dtos.len(),
        "submit ok"
    );

    // -- Build response ---------------------------------------------------
    let transport_mode_str = match state.transport_mode {
        crate::transport::TransportMode::Api => "API",
        crate::transport::TransportMode::FileSystem => "FileSystem",
    };

    let captured_at_str = now_utc.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let body = SubmitResponse {
        vault_path: vault_path.clone(),
        transport_mode: transport_mode_str,
        auto_created: auto_created_dtos,
        post_create_commands: post_create_command_dtos,
        history_id: history_id.clone(),
        captured_at: captured_at_str,
        warnings,
    };

    let location = format!("/api/v1/history/{}", history_id);
    let mut resp = (StatusCode::CREATED, Json(body)).into_response();
    if let Ok(loc) = HeaderValue::from_str(&location) {
        resp.headers_mut().insert(header::LOCATION, loc);
    }
    resp
}

/// Normalize raw directory listing items to file stems.
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
