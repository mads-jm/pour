/// Step 1 — Parse + validate the incoming request.
///
/// Covers:
/// - Body read (honours `DefaultBodyLimit` via `into_limited_body`).
/// - Module lookup (404 for unknown or `mobile_visible = false`).
/// - Body deserialization.
/// - `captured_at` window validation and resolution (§10).
/// - Visibility filtering (`show_when`).
/// - Field-level validation (required, number parseable).
///
/// Returns `ValidatedData` on success; returns a ready `Response` on any
/// failure so the caller can short-circuit immediately.
use std::collections::{HashMap, HashSet};

use axum::RequestExt as _;
use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::Response;
use chrono::{DateTime, Local, Utc};
use serde_json::json;

use crate::config::FieldType;
use crate::server::{
    AppState,
    dto::{error_codes, error_response, error_response_with_details, extra_error_codes},
    is_length_limit_error,
};
use crate::visibility::visible_field_indices;

/// Maximum age for `captured_at` — 30 days past.
pub(super) const CAPTURED_AT_MAX_AGE_SECS: i64 = 30 * 24 * 3600;
/// Maximum future skew for `captured_at` — 5 minutes.
pub(super) const CAPTURED_AT_MAX_FUTURE_SECS: i64 = 5 * 60;

/// Fully-validated, parsed data extracted from a single submit request.
///
/// All ownership is taken here; downstream steps receive references into this.
pub(super) struct ValidatedData {
    pub field_values: HashMap<String, String>,
    /// Owned set of visible field names (resolved by `show_when` filtering).
    pub visible_names: HashSet<String>,
    pub now_utc: DateTime<Utc>,
    pub now_local: DateTime<Local>,
    pub auto_create_inputs: HashMap<String, HashMap<String, String>>,
    pub composite_data: HashMap<String, Vec<Vec<String>>>,
    pub callout_overrides: HashMap<String, String>,
    pub callout_titles: HashMap<String, String>,
}

/// Consumes the raw request, validates everything, and returns `ValidatedData`.
///
/// On any failure returns `Err(Response)` — the response is ready to return
/// from the handler without further processing.
pub(super) async fn run(
    state: &AppState,
    module_key: &str,
    req: axum::http::Request<axum::body::Body>,
) -> Result<ValidatedData, Response> {
    // -- Module lookup (404 for unknown or mobile_visible=false) ----------
    let module = match state.config.modules.get(module_key) {
        Some(m) if m.is_mobile_visible() => m,
        _ => {
            return Err(error_response(
                StatusCode::NOT_FOUND,
                error_codes::NOT_FOUND,
                "Unknown module.",
            ));
        }
    };

    // -- Body read (respects DefaultBodyLimit via into_limited_body) ------
    let body_bytes = match to_bytes(req.into_limited_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            if is_length_limit_error(&e) {
                return Err(error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    error_codes::PAYLOAD_TOO_LARGE,
                    "Request body exceeds the 1 MiB limit.",
                ));
            }
            return Err(error_response_with_details(
                StatusCode::INTERNAL_SERVER_ERROR,
                error_codes::INTERNAL_ERROR,
                "Failed to read request body.",
                json!({ "engine_error": e.to_string() }),
            ));
        }
    };

    let submit = match serde_json::from_slice::<crate::server::dto::SubmitRequest>(&body_bytes) {
        Ok(s) => s,
        Err(e) => {
            return Err(error_response_with_details(
                StatusCode::BAD_REQUEST,
                error_codes::VALIDATION_FAILED,
                "Invalid JSON body.",
                json!({ "engine_error": e.to_string() }),
            ));
        }
    };

    // -- captured_at resolution (§10) -------------------------------------
    let server_now_utc = Utc::now();
    let (now_utc, now_local) = match submit.captured_at {
        Some(captured) => {
            let age_secs = (server_now_utc - captured).num_seconds();
            let future_secs = (captured - server_now_utc).num_seconds();
            if age_secs > CAPTURED_AT_MAX_AGE_SECS || future_secs > CAPTURED_AT_MAX_FUTURE_SECS {
                return Err(error_response_with_details(
                    StatusCode::BAD_REQUEST,
                    error_codes::VALIDATION_FAILED,
                    "captured_at is outside the allowed range (30 days past, 5 minutes future).",
                    json!({ "code": extra_error_codes::CAPTURED_AT_OUT_OF_RANGE }),
                ));
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
    let visible_names_ref: HashSet<&str> = visible_indices
        .iter()
        .map(|&i| module.fields[i].name.as_str())
        .collect();

    // Belt-and-suspenders per §6.4: strip invisible field values.
    let field_values: HashMap<String, String> = submit
        .field_values
        .into_iter()
        .filter(|(k, _)| visible_names_ref.contains(k.as_str()))
        .collect();

    // -- Field validation -------------------------------------------------
    let mut field_errors: Vec<serde_json::Value> = Vec::new();

    for field in &module.fields {
        if !visible_names_ref.contains(field.name.as_str()) {
            continue;
        }

        let value = field_values
            .get(&field.name)
            .map(|s| s.as_str())
            .unwrap_or("");

        if field.required.unwrap_or(false) && value.trim().is_empty() {
            field_errors.push(json!({ "field": field.name, "code": "required" }));
            continue;
        }

        if field.field_type == FieldType::Number && !value.trim().is_empty() {
            let ok = value.trim().parse::<i64>().is_ok() || value.trim().parse::<f64>().is_ok();
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
        return Err(error_response_with_details(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Submit rejected because required fields are empty or invalid.",
            json!({ "fields": field_errors }),
        ));
    }

    // Owned set of visible names for downstream steps.
    let visible_names: HashSet<String> = visible_names_ref.iter().map(|&s| s.to_string()).collect();

    Ok(ValidatedData {
        field_values,
        visible_names,
        now_utc,
        now_local,
        auto_create_inputs: submit.auto_create_inputs,
        composite_data: submit.composite_data,
        callout_overrides: submit.callout_overrides,
        callout_titles: submit.callout_titles,
    })
}
