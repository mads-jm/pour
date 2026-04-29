use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::transport::TransportMode;

use super::super::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub version: &'static str,
    pub schema_version: &'static str,
    pub transport_mode: &'static str,
    pub vault_base_path: String,
    pub capabilities: Vec<&'static str>,
}

pub async fn handler(State(state): State<AppState>) -> Response {
    let transport_mode = match state.transport_mode {
        TransportMode::Api => "API",
        TransportMode::FileSystem => "FileSystem",
    };

    let body = HealthResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
        schema_version: "1",
        transport_mode,
        vault_base_path: state.config.vault.base_path.clone(),
        capabilities: vec![
            "composite_array",
            "create_template",
            "post_create_command",
            "show_when",
            "presets",
            "history",
            "idempotency_key",
            "captured_at",
        ],
    };

    Json(body).into_response()
}
