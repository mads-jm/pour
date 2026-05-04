/// Static asset embedding and serving.
///
/// All files under `web/` at the repo root are embedded at compile time via
/// rust-embed. Asset handlers are unauthenticated (contract §3) and set their
/// own Cache-Control headers per §12.
use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

// ---------------------------------------------------------------------------
// rust-embed: embed the `web/` directory at compile time
// ---------------------------------------------------------------------------

/// All files under `web/` at the repo root are embedded at compile time.
/// Served unauthenticated at `/`, `/app.js`, `/styles.css`, `/manifest.json`,
/// `/favicon.ico`, and `/static/{path}` (contract §3, §12).
#[derive(rust_embed::RustEmbed)]
#[folder = "web/"]
pub(crate) struct StaticAssets;

// ---------------------------------------------------------------------------
// Content-Type + Cache-Control helpers
// ---------------------------------------------------------------------------

/// Map a file extension to its Content-Type.
pub(crate) fn content_type_for(path: &str) -> &'static str {
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
pub(crate) fn cache_control_for(path: &str) -> &'static str {
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

// ---------------------------------------------------------------------------
// Core asset serving
// ---------------------------------------------------------------------------

/// Serve a single embedded file by asset path (relative to `web/`).
///
/// Returns 404 plain text (NOT the JSON error envelope — static assets are not API resources).
pub(crate) fn serve_asset(asset_path: &str) -> Response {
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

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

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
