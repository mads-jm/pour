/// Router construction for the Pour server.
///
/// `build_app` and `serve_on_listener` are extracted here so that the router
/// topology lives in one place, separate from server lifecycle (`run`,
/// `run_with_shutdown`) and from the static asset handlers.
use anyhow::Result;
use axum::{Router, middleware, routing::get, routing::post, routing::put};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use super::static_assets::{
    app_js_handler, favicon_handler, index_handler, manifest_handler, queue_js_handler,
    static_asset_handler, styles_css_handler, sw_js_handler,
};
use super::{AppState, auth, method_not_allowed_handler, no_store_middleware, not_found_handler};
use crate::server::handlers;

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
