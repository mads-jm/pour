pub mod dto;
pub mod handlers;
pub mod idempotency;
pub mod startup;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Router, middleware, routing::get, routing::post, routing::put};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

// ---------------------------------------------------------------------------
// Static asset embedding (rust-embed)
// ---------------------------------------------------------------------------

/// All files under `web/` at the repo root are embedded at compile time.
/// Served unauthenticated at `/`, `/app.js`, `/styles.css`, `/manifest.json`,
/// `/favicon.ico`, and `/static/{path}` (contract §3, §12).
#[derive(rust_embed::RustEmbed)]
#[folder = "web/"]
struct StaticAssets;

/// Map a file extension to its Content-Type.
fn content_type_for(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".webmanifest") {
        "application/manifest+json"
    } else {
        "application/octet-stream"
    }
}

/// Cache-Control header value for a static asset (contract §12).
///
/// - Shell HTML and manifest get `no-cache, max-age=0, must-revalidate`
///   (always revalidate; service worker handles offline — Phase 2).
/// - Everything else (JS, CSS, icons) gets `public, max-age=300, must-revalidate`
///   (5-minute browser cache; short enough to see rapid dev-cycle changes).
fn cache_control_for(path: &str) -> &'static str {
    if path.ends_with(".html")
        || path.ends_with(".webmanifest")
        || path == "manifest.json"
        || path == "sw.js"
    {
        "no-cache, max-age=0, must-revalidate"
    } else {
        "public, max-age=300, must-revalidate"
    }
}

/// Serve a single embedded file by asset path (relative to `web/`).
///
/// Returns 404 plain text (NOT the JSON error envelope — static assets are not API resources).
fn serve_asset(asset_path: &str) -> Response {
    match StaticAssets::get(asset_path) {
        Some(file) => {
            let ct = content_type_for(asset_path);
            let cc = cache_control_for(asset_path);
            let body: Vec<u8> = file.data.into_owned();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, ct), (header::CACHE_CONTROL, cc)],
                body,
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "404 Not Found",
        )
            .into_response(),
    }
}

/// Handler for `GET /` — serves `index.html`.
pub async fn index_handler() -> Response {
    serve_asset("index.html")
}

/// Handler for `GET /app.js`.
pub async fn app_js_handler() -> Response {
    serve_asset("app.js")
}

/// Handler for `GET /styles.css`.
pub async fn styles_css_handler() -> Response {
    serve_asset("styles.css")
}

/// Handler for `GET /manifest.json`.
pub async fn manifest_handler() -> Response {
    serve_asset("manifest.json")
}

/// Handler for `GET /favicon.ico`.
pub async fn favicon_handler() -> Response {
    serve_asset("favicon.ico")
}

/// Handler for `GET /queue.js` — IDB queue module.
///
/// Served at root scope alongside app.js. Loaded before app.js via a `<script>`
/// tag so listQueue/removeQueueRecord etc. are available when app.js calls them.
pub async fn queue_js_handler() -> Response {
    serve_asset("queue.js")
}

/// Handler for `GET /sw.js` — service worker script.
///
/// MUST be served at root scope (`/sw.js`, NOT `/static/sw.js`) so the
/// service worker can control all navigation to `/`. A SW served from
/// `/static/sw.js` can only control paths under `/static/`, which would
/// exclude the PWA shell.
///
/// Cache-Control is `no-cache, max-age=0, must-revalidate` per contract §12
/// (same as the shell HTML). The browser re-validates on every page load so
/// a new deploy is detected promptly. This is already returned by
/// `cache_control_for("sw.js")` — no special-casing needed.
pub async fn sw_js_handler() -> Response {
    serve_asset("sw.js")
}

/// Handler for `GET /static/*path` — fallback for any other embedded asset.
///
/// Axum's wildcard `*path` captures include a leading `/`; strip it before
/// looking up the embedded asset so that `/static/icon.svg` resolves to `icon.svg`.
///
/// Root-scope assets that have their own named routes (`/app.js`, `/sw.js`,
/// `/styles.css`, `/manifest.json`, etc.) MUST NOT be accessible via `/static/`.
/// Serving the SW at `/static/sw.js` would silently allow installing a SW with
/// the wrong scope — a scope-poisoning risk.
pub async fn static_asset_handler(Path(path): Path<String>) -> Response {
    let asset_path = path.trim_start_matches('/');
    // Block assets that are served exclusively at root scope via dedicated routes.
    // A SW installed from /static/sw.js can only control /static/* paths, which
    // is wrong; it must be at /sw.js (root scope) to control navigation.
    const ROOT_SCOPE_ASSETS: &[&str] = &[
        "sw.js",
        "app.js",
        "queue.js",
        "styles.css",
        "manifest.json",
        "index.html",
        "favicon.ico",
    ];
    if ROOT_SCOPE_ASSETS.contains(&asset_path) {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "404 Not Found",
        )
            .into_response();
    }
    serve_asset(asset_path)
}

use crate::config::Config;
use crate::data::presets::Presets;
use crate::transport::{Transport, TransportMode};

use dto::{error_codes, error_response};
use idempotency::IdempotencyCache;

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

/// Build the axum Router for the given `AppState`.
///
/// Extracted so that `run` (port-based) and `serve_on_listener` (pre-bound
/// listener, used by integration tests) share identical topology.
pub fn build_app(state: AppState) -> Router {
    // Auth middleware is applied to known routes only via route_layer.
    // The global fallback is outside auth so unknown paths → 404 (not 401).
    // no_store_middleware is the outermost layer: it wraps everything on the
    // api subrouter (auth rejections, method-not-allowed, and handler responses)
    // so that Cache-Control: no-store appears on every /api/* response (§12).
    //
    // Body size limits (§13):
    //   - submit: 1 MiB via DefaultBodyLimit::max per-route
    //   - presets PUT: 256 KiB via DefaultBodyLimit::max per-route
    //   - all other routes: 16 KiB (axum's DefaultBodyLimit global default)
    let api = Router::new()
        .route("/api/v1/health", get(handlers::health::handler))
        .route("/api/v1/config", get(handlers::config::handler))
        .route(
            "/api/v1/options/:module/:field",
            get(handlers::options::handler),
        )
        .route(
            "/api/v1/submit/:module",
            post(handlers::submit::handler)
                .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)),
        )
        .route(
            "/api/v1/captures/:history_id",
            get(handlers::captures::handler),
        )
        .route("/api/v1/history", get(handlers::history::handler))
        .route(
            "/api/v1/presets/:module",
            get(handlers::presets::get_handler),
        )
        .route(
            "/api/v1/presets/:module/order",
            put(handlers::presets::order_handler)
                .layer(axum::extract::DefaultBodyLimit::max(256 * 1024)),
        )
        .route(
            "/api/v1/presets/:module/:name",
            put(handlers::presets::put_handler)
                .delete(handlers::presets::delete_handler)
                .layer(axum::extract::DefaultBodyLimit::max(256 * 1024)),
        )
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    // Static asset routes are OUTSIDE the api subrouter.
    // They are unauthenticated (contract §3) and have their own Cache-Control
    // headers set per §12 — the no_store_middleware MUST NOT apply to them.
    // Adding them directly to the outer router, before the global fallback,
    // ensures they bypass both the auth middleware and the no_store_middleware.
    Router::new()
        .merge(api)
        // Unauthenticated static asset routes (contract §3, §12)
        .route("/", get(index_handler))
        .route("/app.js", get(app_js_handler))
        .route("/styles.css", get(styles_css_handler))
        .route("/manifest.json", get(manifest_handler))
        .route("/favicon.ico", get(favicon_handler))
        // Service worker: MUST be at root scope (not /static/) so it can control
        // all navigation. Cache-Control: no-cache per contract §12. (TASK-2.2.1)
        .route("/sw.js", get(sw_js_handler))
        // IDB queue module: loaded before app.js via <script> tag. (TASK-2.1.1)
        .route("/queue.js", get(queue_js_handler))
        .route("/static/*path", get(static_asset_handler))
        .fallback(not_found_handler)
        .with_state(state)
}

/// Serve the app on a pre-bound `TcpListener`.
///
/// Used by integration tests to inject a port-0 listener so the OS assigns
/// an ephemeral port. Production code calls `run` instead.
pub async fn serve_on_listener(listener: tokio::net::TcpListener, state: AppState) -> Result<()> {
    let app = build_app(state);
    axum::serve(listener, app).await?;
    Ok(())
}

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

    let vault_path = &config.vault.base_path;
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
