use axum::{Json, extract::State};
use serde::Serialize;

use crate::transport::TransportMode;

use super::super::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub transport_mode: &'static str,
    pub version: &'static str,
}

pub async fn handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let transport_mode = match state.transport_mode {
        TransportMode::Api => "API",
        TransportMode::FileSystem => "FileSystem",
    };

    Json(HealthResponse {
        ok: true,
        transport_mode,
        version: env!("CARGO_PKG_VERSION"),
    })
}
