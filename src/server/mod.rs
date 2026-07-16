pub(crate) mod dto; // request/response types — not part of external Rust API
pub mod handlers;
pub mod idempotency;
pub(crate) mod routing; // build_app/serve_on_listener re-exported below
pub mod startup;
pub(crate) mod static_assets; // individual handlers re-exported below

// Re-export the public surface tests and callers depend on.
pub use routing::{build_app, serve_on_listener};
pub use static_assets::{
    app_js_handler, favicon_handler, index_handler, manifest_handler, queue_js_handler,
    static_asset_handler, styles_css_handler, sw_js_handler,
};

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::data::presets::Presets;
use crate::transport::{Transport, TransportMode};

use dto::{error_codes, error_response};
use idempotency::IdempotencyCache;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the error comes from axum/hyper's body-length limit.
///
/// # Documented fragility
/// Axum's `DefaultBodyLimit` produces a `LengthLimitError` when the cap is
/// exceeded, but axum does not expose a typed error for this case — the only
/// public API is `to_bytes(into_limited_body(), …)` returning an `axum::Error`
/// whose `Display` contains "length limit exceeded". If hyper or axum renames
/// this message in a future release, handlers that call this helper will return
/// 500 instead of 413 until the string is updated here. By centralising the
/// check in one place, a single-point regression is much cheaper to fix than
/// hunting across every handler.
pub fn is_length_limit_error(e: &axum::Error) -> bool {
    e.to_string().contains("length limit exceeded")
}

/// Shared state threaded through every handler.
///
/// # TUI / server isolation note
/// `Presets` is loaded independently by the TUI (via `src/main.rs`) and by the
/// server here. Both read from / write to the same on-disk `presets.json` file,
/// but they do NOT share a live in-memory instance. This is intentional: the TUI
/// and `pour serve` are separate processes (`pour serve` runs instead of the TUI,
/// not alongside it). Concurrent mutation from two processes is not a supported
/// workflow; documents should remain internally consistent because each write is
/// atomic (temp-file + rename).
#[derive(Clone)]
pub struct AppState {
    pub transport_mode: TransportMode,
    /// Bearer token that the PWA client must present.
    pub token: String,
    /// Full config, shared across handlers (read-only after startup).
    pub config: Arc<Config>,
    /// Transport — Arc-wrapped for cheap clone across handler invocations.
    pub transport: Arc<Transport>,
    /// In-memory idempotency cache for POST /api/v1/submit/{module} (§9).
    pub idempotency: Arc<IdempotencyCache>,
    /// Presets — shared mutable state for CRUD endpoints (§6.7–§6.10).
    pub presets: Arc<Mutex<Presets>>,
}

/// Constant-time token comparison. Returns true iff `candidate == secret`.
fn token_matches(candidate: &str, secret: &str) -> bool {
    candidate.as_bytes().ct_eq(secret.as_bytes()).into()
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

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
pub async fn auth(State(state): State<AppState>, req: Request, next: Next) -> Response {
    // 1. Extract the Authorization header token, if any.
    let header_token: Option<String> = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string());

    // Capture path for rejection logging (no token values, no headers — §14).
    let path = req.uri().path().to_string();

    if let Some(ref hdr) = header_token {
        // Header is present and non-empty — it is authoritative.
        // Reject immediately on mismatch; do NOT fall through to query.
        if token_matches(hdr, &state.token) {
            tracing::debug!("auth: accepted_via_header");
            return next.run(req).await;
        }
        tracing::warn!(path = %path, "auth: rejected");
        return error_response(
            axum::http::StatusCode::UNAUTHORIZED,
            error_codes::UNAUTHORIZED,
            "Missing or invalid authentication token.",
        );
    }

    // 2. No usable header token — check ?token= query param (bootstrap / QR-code path).
    let query_token = req.uri().query().and_then(|q| {
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
        Some(ref qt) if token_matches(qt, &state.token) => {
            tracing::info!("auth: accepted_via_query");
            next.run(req).await
        }
        _ => {
            tracing::warn!(path = %path, "auth: rejected");
            error_response(
                axum::http::StatusCode::UNAUTHORIZED,
                error_codes::UNAUTHORIZED,
                "Missing or invalid authentication token.",
            )
        }
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
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
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
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

/// Initialize the tracing subscriber.
///
/// Reads `POUR_LOG` env var for level filtering; defaults to `info` for all
/// targets.  Uses `try_init` so that a second call (e.g. from integration tests
/// that share a global subscriber) silently no-ops instead of panicking.
///
/// Examples:
///   `POUR_LOG=debug pour serve`           — verbose, all targets
///   `POUR_LOG=pour=debug,tower_http=warn` — fine-grained
pub fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("POUR_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,pour=info,tower_http=info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .compact()
        .try_init();
}

// ---------------------------------------------------------------------------
// Server lifecycle
// ---------------------------------------------------------------------------

/// Build and run the axum server, shutting down when `shutdown` resolves.
///
/// The TUI handoff calls this with a `tokio::signal::ctrl_c()` future so that
/// Ctrl+C stops the server but does not exit the process — allowing the TUI to
/// resume. The CLI `run` delegates here with the same signal, but lets the
/// process exit naturally after `run` returns.
pub async fn run_with_shutdown<F>(
    config: Config,
    transport: Transport,
    port: u16,
    token: String,
    listener: tokio::net::TcpListener,
    shutdown: F,
) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    init_logging();

    let transport_mode = transport.mode();

    let vault_path = config.vault.effective_base_path();
    let transport_label = match transport_mode {
        TransportMode::Api => "API",
        TransportMode::FileSystem => "FileSystem",
    };
    // Use the actual bound address from the listener, not the requested port,
    // so that port-0 (OS-assigned) binds are logged correctly in tests.
    let bound_addr = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| format!("0.0.0.0:{port}"));
    tracing::info!(
        address = %bound_addr,
        transport = transport_label,
        vault = %vault_path,
        "serving"
    );

    let presets = Presets::load();
    let state = AppState {
        transport_mode,
        token,
        config: Arc::new(config),
        transport: Arc::new(transport),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(Mutex::new(presets)),
    };

    let app = build_app(state);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

/// Build and run the axum server.
///
/// Blocks until the process is interrupted (Ctrl+C via OS signal).
/// The CLI relies on the process being killed externally; the shutdown future
/// never resolves so axum blocks indefinitely, matching the old behavior.
pub async fn run(config: Config, transport: Transport, port: u16, token: String) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // `std::future::pending()` never resolves — the OS SIGINT kills the process
    // directly, which is the same as before this refactor.
    run_with_shutdown(
        config,
        transport,
        port,
        token,
        listener,
        std::future::pending(),
    )
    .await
}
