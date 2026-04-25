pub mod handlers;

use std::net::SocketAddr;

use anyhow::Result;
use axum::{Router, middleware, routing::get};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;

use crate::transport::{Transport, TransportMode};

/// Shared state threaded through every handler.
#[derive(Clone)]
pub struct AppState {
    pub transport_mode: TransportMode,
    /// Bearer token that the PWA client must present.
    pub token: String,
}

/// Constant-time token comparison. Returns true iff `candidate == secret`.
fn token_matches(candidate: &str, secret: &str) -> bool {
    candidate.as_bytes().ct_eq(secret.as_bytes()).into()
}

/// Auth middleware.
///
/// Precedence:
/// 1. `Authorization: Bearer <token>` — authoritative. If the header is present
///    with a non-empty token suffix, compare it and reject immediately on mismatch.
///    The query-string is ignored once a header token is found.
/// 2. `?token=<token>` — bootstrap-only. Checked only when the Authorization
///    header is absent or carries an empty token (e.g. bare "Bearer " prefix).
/// 3. Neither present → 401.
///
/// All comparisons are constant-time to prevent timing side-channels.
pub async fn auth(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1. Extract the Authorization header token, if any.
    let header_token: Option<String> = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string());

    if let Some(ref hdr) = header_token {
        // Header is present and non-empty — it is authoritative.
        // Reject immediately on mismatch; do NOT fall through to query.
        if token_matches(hdr, &state.token) {
            return Ok(next.run(req).await);
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 2. No usable header token — check ?token= query param (bootstrap / QR-code path).
    let query_token = req
        .uri()
        .query()
        .and_then(|q| {
            q.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                if k == "token" && !v.is_empty() {
                    Some(v.to_string())
                } else {
                    None
                }
            })
        });

    match query_token {
        Some(ref qt) if token_matches(qt, &state.token) => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Build and run the axum server.
///
/// Blocks until the process is interrupted (Ctrl+C).
pub async fn run(
    transport: Transport,
    port: u16,
    token: String,
) -> Result<()> {
    let state = AppState {
        transport_mode: transport.mode(),
        token,
    };

    let app = Router::new()
        .route("/api/health", get(handlers::health::handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;
    Ok(())
}
