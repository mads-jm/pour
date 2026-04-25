use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::transport::TransportMode;

use super::super::dto::build_config_response;
use super::super::AppState;

pub async fn handler(State(state): State<AppState>) -> Response {
    let transport_mode = match state.transport_mode {
        TransportMode::Api => "API",
        TransportMode::FileSystem => "FileSystem",
    };

    let body = build_config_response(&state.config, transport_mode);

    Json(body).into_response()
}
