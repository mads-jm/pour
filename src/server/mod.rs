pub mod dto;
pub mod handlers;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{Router, middleware, routing::get};
use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;

use crate::config::Config;
use crate::transport::{Transport, TransportMode};

use dto::{error_codes, error_response};

/// Shared state threaded through every handler.
#[derive(Clone)]
pub struct AppState {
    pub transport_mode: TransportMode,
    /// Bearer token that the PWA client must present.
    pub token: String,
    /// Full config, shared across handlers (read-only after startup).
    pub config: Arc<Config>,
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
/// 3. Neither present → 401 with JSON error envelope (§5.2).
///
/// All comparisons are constant-time to prevent timing side-channels.
pub async fn auth(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
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
            return next.run(req).await;
        }
        return error_response(
            axum::http::StatusCode::UNAUTHORIZED,
            error_codes::UNAUTHORIZED,
            "Missing or invalid authentication token.",
        );
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
        Some(ref qt) if token_matches(qt, &state.token) => next.run(req).await,
        _ => error_response(
            axum::http::StatusCode::UNAUTHORIZED,
            error_codes::UNAUTHORIZED,
            "Missing or invalid authentication token.",
        ),
    }
}

/// Global fallback handler: 404 with JSON error envelope for any unmatched path.
///
/// This is placed OUTSIDE the auth-protected router so truly unknown paths
/// return 404 (not 401), preventing the server from leaking whether a path
/// requires auth before the client knows the path even exists.
///
/// Because it lives outside the api subrouter, it is also outside the
/// `no_store_middleware` layer. It sets `Cache-Control: no-store` itself
/// to satisfy §12 of the contract.
pub async fn not_found_handler() -> Response {
    let mut resp = error_response(
        axum::http::StatusCode::NOT_FOUND,
        error_codes::NOT_FOUND,
        "Resource not found.",
    );
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    resp
}

/// Method-not-allowed fallback: 405 with JSON error envelope (§5.2).
///
/// Axum sets the `Allow` header automatically before calling this handler,
/// so we only need to supply the envelope body.
pub async fn method_not_allowed_handler() -> Response {
    error_response(
        axum::http::StatusCode::METHOD_NOT_ALLOWED,
        error_codes::METHOD_NOT_ALLOWED,
        "Method not allowed.",
    )
}

/// Response middleware: sets `Cache-Control: no-store` on every response from
/// the api subrouter (§12 of the contract). Applied as the outermost layer so
/// it covers handlers, auth rejections, and the method-not-allowed fallback.
pub async fn no_store_middleware(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    resp
}

/// Build and run the axum server.
///
/// Blocks until the process is interrupted (Ctrl+C).
pub async fn run(
    config: Config,
    transport: Transport,
    port: u16,
    token: String,
) -> Result<()> {
    let state = AppState {
        transport_mode: transport.mode(),
        token,
        config: Arc::new(config),
    };

    // Auth middleware is applied to known routes only via route_layer.
    // The global fallback is outside auth so unknown paths → 404 (not 401).
    // no_store_middleware is the outermost layer: it wraps everything on the
    // api subrouter (auth rejections, method-not-allowed, and handler responses)
    // so that Cache-Control: no-store appears on every /api/* response (§12).
    let api = Router::new()
        .route("/api/v1/health", get(handlers::health::handler))
        .route("/api/v1/config", get(handlers::config::handler))
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    let app = Router::new()
        .merge(api)
        .fallback(not_found_handler)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;
    Ok(())
}
