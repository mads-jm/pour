use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};

use crate::transport::TransportMode;

use super::super::AppState;
use super::super::dto::build_config_response;

pub async fn handler(State(state): State<AppState>) -> Response {
    let transport_mode = match state.transport_mode {
        TransportMode::Api => "API",
        TransportMode::FileSystem => "FileSystem",
    };

    let body = build_config_response(&state.config, transport_mode);

    Json(body).into_response()
}
