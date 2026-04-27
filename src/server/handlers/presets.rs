/// Handlers for preset CRUD endpoints (§6.7–§6.10).
///
/// All five endpoints share this file:
/// - `GET  /api/v1/presets/{module}`         → `get_handler`
/// - `PUT  /api/v1/presets/{module}/{name}`  → `put_handler`
/// - `DELETE /api/v1/presets/{module}/{name}` → `delete_handler`
/// - `PUT  /api/v1/presets/{module}/order`   → `order_handler`
///
/// Route ordering note: `/order` is a fixed path segment that must be registered
/// BEFORE the `/{name}` wildcard in `src/server/mod.rs`, otherwise axum matches
/// the literal "order" as a preset name and the reorder endpoint is unreachable.
/// See the router in `src/server/mod.rs` — `order` route is registered first.
use axum::Json;
use axum::RequestExt as _;
use axum::body::to_bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::super::{
    AppState,
    dto::{
        PresetDto, PresetReorderRequest, PresetUpsertRequest, PresetUpsertResponse,
        PresetsListResponse, error_codes, error_response, error_response_with_details,
    },
    is_length_limit_error,
};
use crate::data::presets::{ReorderError, SetResult};

// ---------------------------------------------------------------------------
// GET /api/v1/presets/{module}  (§6.7)
// ---------------------------------------------------------------------------

pub async fn get_handler(
    State(state): State<AppState>,
    Path(module): Path<String>,
) -> Response {
    // 404 if module unknown.
    if !state.config.modules.contains_key(&module) {
        return error_response(
            StatusCode::NOT_FOUND,
            error_codes::NOT_FOUND,
            "Unknown module.",
        );
    }

    let presets = state.presets.lock().await;
    // None → empty list (module exists but has no presets yet, per §6.7).
    let list: Vec<PresetDto> = presets
        .module_presets(&module)
        .unwrap_or(&[])
        .iter()
        .map(PresetDto::from)
        .collect();

    Json(PresetsListResponse { presets: list }).into_response()
}

// ---------------------------------------------------------------------------
// PUT /api/v1/presets/{module}/{name}  (§6.8)
// ---------------------------------------------------------------------------

pub async fn put_handler(
    State(state): State<AppState>,
    Path((module, name)): Path<(String, String)>,
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    // Module 404.
    if !state.config.modules.contains_key(&module) {
        return error_response(
            StatusCode::NOT_FOUND,
            error_codes::NOT_FOUND,
            "Unknown module.",
        );
    }

    // Reject empty name.
    if name.trim().is_empty() {
        return error_response_with_details(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Preset name must not be empty.",
            json!({ "field": "name" }),
        );
    }

    // Reject names containing `/` (defensive belt-and-suspenders; axum's path
    // matcher should already prevent these from routing here, but verify).
    if name.contains('/') {
        return error_response(
            StatusCode::NOT_FOUND,
            error_codes::NOT_FOUND,
            "Preset name may not contain '/'.",
        );
    }

    // Reject the reserved name "order" (case-insensitive). The route
    // `/presets/:module/order` is fixed and registered before `/:name`, so
    // a PUT for the literal name "order" would be routed to the order_handler
    // instead — but a client could still attempt it via percent-encoding tricks.
    // Belt-and-suspenders: block it here so the name never enters storage.
    if name.to_ascii_lowercase() == "order" {
        return error_response_with_details(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "'order' is a reserved name and cannot be used as a preset name.",
            serde_json::json!({ "field": "name", "code": "reserved_name" }),
        );
    }

    // Content-Type check.
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

    // Read body (limited to 256 KiB via DefaultBodyLimit on the route).
    let body_bytes = match to_bytes(req.into_limited_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            if is_length_limit_error(&e) {
                return error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    error_codes::PAYLOAD_TOO_LARGE,
                    "Request body exceeds the 256 KiB limit.",
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

    let upsert: PresetUpsertRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => {
            return error_response_with_details(
                StatusCode::BAD_REQUEST,
                error_codes::VALIDATION_FAILED,
                "Invalid JSON body.",
                json!({ "engine_error": e.to_string() }),
            );
        }
    };

    let mut presets = state.presets.lock().await;
    let result = match presets.api_set(&module, &name, upsert.description, upsert.values) {
        Ok(r) => r,
        Err(e) => {
            return error_response_with_details(
                StatusCode::INTERNAL_SERVER_ERROR,
                error_codes::INTERNAL_ERROR,
                "Failed to save preset.",
                json!({ "engine_error": e.to_string() }),
            );
        }
    };

    // Retrieve the now-stored preset to build the response.
    let preset_entry = presets
        .module_presets(&module)
        .and_then(|list| list.iter().find(|p| p.name == name))
        .map(PresetDto::from);

    let preset_dto = match preset_entry {
        Some(p) => p,
        None => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                error_codes::INTERNAL_ERROR,
                "Preset saved but could not be read back.",
            );
        }
    };

    let body = PresetUpsertResponse { preset: preset_dto };

    match result {
        SetResult::Created => {
            use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
            let encoded_name = utf8_percent_encode(&name, NON_ALPHANUMERIC).to_string();
            let location = format!("/api/v1/presets/{}/{}", module, encoded_name);
            let mut resp = (StatusCode::CREATED, Json(body)).into_response();
            if let Ok(loc) = HeaderValue::from_str(&location) {
                resp.headers_mut().insert(header::LOCATION, loc);
            }
            resp
        }
        SetResult::Updated => (StatusCode::OK, Json(body)).into_response(),
    }
}

// ---------------------------------------------------------------------------
// DELETE /api/v1/presets/{module}/{name}  (§6.9)
// ---------------------------------------------------------------------------

pub async fn delete_handler(
    State(state): State<AppState>,
    Path((module, name)): Path<(String, String)>,
) -> Response {
    // Module 404.
    if !state.config.modules.contains_key(&module) {
        return error_response(
            StatusCode::NOT_FOUND,
            error_codes::NOT_FOUND,
            "Unknown module.",
        );
    }

    // Reject empty name.
    if name.trim().is_empty() {
        return error_response_with_details(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Preset name must not be empty.",
            json!({ "field": "name" }),
        );
    }

    let mut presets = state.presets.lock().await;
    match presets.api_remove(&module, &name) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error_response(
            StatusCode::NOT_FOUND,
            error_codes::NOT_FOUND,
            "Preset not found.",
        ),
        Err(e) => error_response_with_details(
            StatusCode::INTERNAL_SERVER_ERROR,
            error_codes::INTERNAL_ERROR,
            "Failed to delete preset.",
            json!({ "engine_error": e.to_string() }),
        ),
    }
}

// ---------------------------------------------------------------------------
// PUT /api/v1/presets/{module}/order  (§6.10)
// ---------------------------------------------------------------------------

pub async fn order_handler(
    State(state): State<AppState>,
    Path(module): Path<String>,
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    // Module 404.
    if !state.config.modules.contains_key(&module) {
        return error_response(
            StatusCode::NOT_FOUND,
            error_codes::NOT_FOUND,
            "Unknown module.",
        );
    }

    // Content-Type check.
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

    // Read body.
    let body_bytes = match to_bytes(req.into_limited_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            if is_length_limit_error(&e) {
                return error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    error_codes::PAYLOAD_TOO_LARGE,
                    "Request body exceeds the 256 KiB limit.",
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

    let reorder_req: PresetReorderRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => {
            return error_response_with_details(
                StatusCode::BAD_REQUEST,
                error_codes::VALIDATION_FAILED,
                "Invalid JSON body.",
                json!({ "engine_error": e.to_string() }),
            );
        }
    };

    let mut presets = state.presets.lock().await;
    match presets.api_reorder(&module, reorder_req.names) {
        Ok(()) => {
            // Return the post-reorder list.
            let list: Vec<PresetDto> = presets
                .module_presets(&module)
                .unwrap_or(&[])
                .iter()
                .map(PresetDto::from)
                .collect();
            Json(PresetsListResponse { presets: list }).into_response()
        }
        Err(ReorderError::DuplicateNames(duplicates)) => error_response_with_details(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Reorder list contains duplicate preset names.",
            json!({ "duplicates": duplicates }),
        ),
        Err(ReorderError::MissingNames(missing)) => error_response_with_details(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Reorder list is missing preset names.",
            json!({ "missing": missing }),
        ),
        Err(ReorderError::ExtraNames(extra)) => error_response_with_details(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Reorder list contains unknown preset names.",
            json!({ "extra": extra }),
        ),
        Err(ReorderError::SaveFailed(e)) => error_response_with_details(
            StatusCode::INTERNAL_SERVER_ERROR,
            error_codes::INTERNAL_ERROR,
            "Failed to persist reordered presets.",
            json!({ "engine_error": e.to_string() }),
        ),
    }
}
