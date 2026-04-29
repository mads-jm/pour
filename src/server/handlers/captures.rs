/// Handler for `GET /api/v1/captures/{history_id}` (§6.6).
///
/// Reads back the vault file content for a prior capture, identified by its
/// opaque `history_id` returned by `POST /api/v1/submit/{module}`.
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::super::{
    AppState,
    dto::{
        CaptureResponse, error_codes, error_response, error_response_with_details,
        extra_error_codes,
    },
};
use crate::data::history::History;
use crate::transport::TransportReadError;

pub async fn handler(State(state): State<AppState>, Path(history_id): Path<String>) -> Response {
    // Load history and find the entry.
    let history = History::load();
    let entry = match history.find_by_id(&history_id) {
        Some(e) => e.clone(),
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                error_codes::NOT_FOUND,
                "Unknown history id.",
            );
        }
    };

    // Read file via transport.
    // Pattern-match the typed error — no substring matching on error messages.
    let content = match state.transport.read_file(&entry.vault_path).await {
        Ok(c) => {
            tracing::debug!(history_id = %history_id, "captures: served");
            c
        }
        Err(TransportReadError::NotFound) => {
            tracing::warn!(history_id = %history_id, "captures: vault file missing");
            return error_response(
                StatusCode::NOT_FOUND,
                error_codes::NOT_FOUND,
                "Vault file no longer exists.",
            );
        }
        Err(TransportReadError::Unreachable(msg)) => {
            return error_response_with_details(
                StatusCode::BAD_GATEWAY,
                error_codes::TRANSPORT_ERROR,
                "Transport unreachable.",
                json!({ "engine_error": msg }),
            );
        }
        Err(TransportReadError::Other(msg)) => {
            return error_response_with_details(
                StatusCode::INTERNAL_SERVER_ERROR,
                extra_error_codes::READ_ERROR,
                "Failed to read vault file.",
                json!({ "engine_error": msg }),
            );
        }
    };

    let transport_mode_str = match state.transport_mode {
        crate::transport::TransportMode::Api => "API",
        crate::transport::TransportMode::FileSystem => "FileSystem",
    };

    let timestamp_str = entry.timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    Json(CaptureResponse {
        id: history_id,
        module_key: entry.module_key.clone(),
        timestamp: timestamp_str,
        vault_path: entry.vault_path.clone(),
        content,
        transport_mode: transport_mode_str,
    })
    .into_response()
}
