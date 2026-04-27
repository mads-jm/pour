// Integration tests for static asset serving (Step E).
//
// Verifies:
//   - Static assets are served without auth (contract §3)
//   - Correct Content-Type headers per file extension
//   - Cache-Control discipline per contract §12:
//       * index.html: no-cache, max-age=0, must-revalidate
//       * app.js / styles.css: public, max-age=300, must-revalidate
//       * manifest.json: no-cache, max-age=0, must-revalidate
//   - no_store_middleware does NOT apply to static routes
//   - 404 for missing assets is plain text (not JSON error envelope)

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use tower::ServiceExt as _;

use pour::server::{AppState, handlers, not_found_handler};
use pour::transport::TransportMode;

// ---------------------------------------------------------------------------
// Helpers (mirrors server_health.rs)
// ---------------------------------------------------------------------------

fn minimal_config() -> pour::config::Config {
    let toml = r#"
config_version = "0.3.0"
[vault]
base_path = "/test/vault"

[modules.test]
mode = "create"
path = "Test/%Y%m%d.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"#;
    pour::config::Config::from_toml(toml).expect("minimal test config should parse")
}

fn test_state() -> AppState {
    use pour::data::presets::Presets;
    use pour::server::idempotency::IdempotencyCache;
    use pour::transport::{Transport, fs::FsWriter};
    AppState {
        transport_mode: TransportMode::FileSystem,
        token: "test-token".to_string(),
        config: Arc::new(minimal_config()),
        transport: Arc::new(Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp")))),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(tokio::sync::Mutex::new(Presets::empty())),
    }
}

/// Build the full router including static asset routes and api subrouter.
/// Mirrors the production topology from server::run().
fn make_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get, routing::post, routing::put};
    use pour::server::{
        auth, method_not_allowed_handler, no_store_middleware,
        index_handler, app_js_handler, styles_css_handler,
        manifest_handler, favicon_handler, static_asset_handler,
        sw_js_handler, queue_js_handler,
    };

    let api = Router::new()
        .route("/api/v1/health", get(handlers::health::handler))
        .route("/api/v1/config", get(handlers::config::handler))
        .route("/api/v1/options/:module/:field", get(handlers::options::handler))
        .route("/api/v1/submit/:module", post(handlers::submit::handler))
        .route("/api/v1/captures/:history_id", get(handlers::captures::handler))
        .route("/api/v1/history", get(handlers::history::handler))
        .route("/api/v1/presets/:module", get(handlers::presets::get_handler))
        .route("/api/v1/presets/:module/order", put(handlers::presets::order_handler))
        .route(
            "/api/v1/presets/:module/:name",
            put(handlers::presets::put_handler).delete(handlers::presets::delete_handler),
        )
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    Router::new()
        .merge(api)
        .route("/", get(index_handler))
        .route("/app.js", get(app_js_handler))
        .route("/styles.css", get(styles_css_handler))
        .route("/manifest.json", get(manifest_handler))
        .route("/favicon.ico", get(favicon_handler))
        .route("/sw.js", get(sw_js_handler))
        .route("/queue.js", get(queue_js_handler))
        .route("/static/*path", get(static_asset_handler))
        .fallback(not_found_handler)
        .with_state(state)
}

fn get_req(uri: &str) -> Request<axum::body::Body> {
    Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap()
}

fn authed_req(uri: &str) -> Request<axum::body::Body> {
    Request::builder()
        .uri(uri)
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap()
}

// ---------------------------------------------------------------------------
// §3: Static assets do not require auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn index_served_without_auth() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET / must return 200 with no auth token");
}

#[tokio::test]
async fn app_js_served_without_auth() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET /app.js must return 200 with no auth token");
}

#[tokio::test]
async fn styles_css_served_without_auth() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/styles.css")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET /styles.css must return 200 with no auth token");
}

#[tokio::test]
async fn manifest_served_without_auth() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/manifest.json")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET /manifest.json must return 200 with no auth token");
}

// ---------------------------------------------------------------------------
// Content-Type headers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn index_returns_html_content_type() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("text/html"),
        "GET / must return text/html; got: {ct:?}"
    );
}

#[tokio::test]
async fn app_js_returns_javascript_content_type() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("javascript"),
        "GET /app.js must return application/javascript; got: {ct:?}"
    );
}

#[tokio::test]
async fn styles_css_returns_css_content_type() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/styles.css")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("text/css"),
        "GET /styles.css must return text/css; got: {ct:?}"
    );
}

#[tokio::test]
async fn manifest_returns_json_content_type() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/manifest.json")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("json"),
        "GET /manifest.json must return application/json; got: {ct:?}"
    );
}

// ---------------------------------------------------------------------------
// §12: Cache-Control discipline
// ---------------------------------------------------------------------------

#[tokio::test]
async fn index_has_no_cache_header() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "no-cache, max-age=0, must-revalidate",
        "PWA shell HTML must use no-cache revalidation per §12; got: {cc:?}"
    );
}

#[tokio::test]
async fn app_js_has_max_age_300_header() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "public, max-age=300, must-revalidate",
        "JS assets must use public max-age=300 per §12; got: {cc:?}"
    );
}

#[tokio::test]
async fn styles_css_has_max_age_300_header() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/styles.css")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "public, max-age=300, must-revalidate",
        "CSS assets must use public max-age=300 per §12; got: {cc:?}"
    );
}

#[tokio::test]
async fn manifest_has_no_cache_header() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/manifest.json")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "no-cache, max-age=0, must-revalidate",
        "manifest.json must use no-cache revalidation per §12; got: {cc:?}"
    );
}

// ---------------------------------------------------------------------------
// no_store_middleware does NOT apply to static routes (§12 compliance)
// ---------------------------------------------------------------------------

/// Static assets must NOT carry Cache-Control: no-store.
/// That header is set only by the api subrouter's no_store_middleware.
#[tokio::test]
async fn index_does_not_have_no_store() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_ne!(
        cc, "no-store",
        "GET / must NOT carry Cache-Control: no-store (that is for /api/* only per §12)"
    );
    // Also confirm it's not a substring (e.g. no-store mixed with other directives)
    assert!(
        !cc.contains("no-store"),
        "GET / must not contain no-store in Cache-Control; got: {cc:?}"
    );
}

#[tokio::test]
async fn app_js_does_not_have_no_store() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        !cc.contains("no-store"),
        "GET /app.js must not contain no-store; got: {cc:?}"
    );
}

// ---------------------------------------------------------------------------
// 404 for missing assets — plain text, not JSON envelope
// ---------------------------------------------------------------------------

#[tokio::test]
async fn missing_static_asset_returns_404() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/static/nonexistent.html")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn missing_static_asset_is_not_json_envelope() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/static/nonexistent.html")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("text/plain"),
        "Static 404 must be plain text, not JSON; got: {ct:?}"
    );
    let bytes = to_bytes(resp.into_body(), 256).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        !body.contains("\"error\""),
        "Static 404 body must not be a JSON error envelope; got: {body:?}"
    );
}

// ---------------------------------------------------------------------------
// API auth still enforced on /api/* routes
// ---------------------------------------------------------------------------

/// Verify that adding static routes did not accidentally break API auth.
#[tokio::test]
async fn api_health_still_requires_auth_after_static_routes_added() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/api/v1/health")).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "API endpoint must still require auth even after static routes are mounted"
    );
}

/// API endpoint with valid auth still works.
#[tokio::test]
async fn api_health_works_with_valid_auth() {
    let router = make_router(test_state());
    let resp = router.oneshot(authed_req("/api/v1/health")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

/// API /api/v1/health still carries Cache-Control: no-store (not replaced by static rules).
#[tokio::test]
async fn api_health_still_has_no_store() {
    let router = make_router(test_state());
    let resp = router.oneshot(authed_req("/api/v1/health")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "no-store",
        "API /api/v1/health must still have Cache-Control: no-store; got: {cc:?}"
    );
}

// ---------------------------------------------------------------------------
// Body content sanity checks
// ---------------------------------------------------------------------------

/// GET / body contains the word "Pour" — minimal sanity that we're serving real HTML.
#[tokio::test]
async fn index_body_contains_pour() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 8192).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("Pour"),
        "index.html body should contain 'Pour'; got first 200 chars: {:?}",
        &body[..body.len().min(200)]
    );
}

/// GET /app.js body is non-empty JavaScript.
#[tokio::test]
async fn app_js_body_is_non_empty() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    assert!(!bytes.is_empty(), "app.js body must not be empty");
}

/// app.js contains the escapeHtml helper — locks XSS regression (CRITICAL #1).
#[tokio::test]
async fn app_js_contains_escape_html() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("escapeHtml"),
        "app.js must contain escapeHtml function to guard against XSS"
    );
}

/// app.js contains crypto.getRandomValues fallback — proves uuidv4 works on http:// (MAJOR #2).
#[tokio::test]
async fn app_js_contains_uuid_fallback() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("crypto.getRandomValues"),
        "app.js must contain crypto.getRandomValues fallback path in uuidv4()"
    );
}

// ---------------------------------------------------------------------------
// Phase 1.5: show_when fix + preset selector — content-check assertions
// ---------------------------------------------------------------------------

/// app.js binds "change" on the form element (not per-field), which ensures
/// <select> change events bubble correctly and trigger recomputeVisibility.
#[tokio::test]
async fn app_js_form_change_listener_bound_on_form() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("form.addEventListener(\"change\""),
        "app.js must bind 'change' listener on the form element (not per-field) to catch <select> events"
    );
}

/// app.js also binds "input" on the form element, covering text/number keystrokes.
#[tokio::test]
async fn app_js_form_input_listener_bound_on_form() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("form.addEventListener(\"input\""),
        "app.js must bind 'input' listener on the form element for text/number reactivity"
    );
}

/// app.js uses readCurrentFieldValues() which reads ALL fields unconditionally,
/// so the controlling field's fresh value is always available to computeVisible.
#[tokio::test]
async fn app_js_has_read_current_field_values() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("function readCurrentFieldValues"),
        "app.js must have readCurrentFieldValues() that reads all field values regardless of visibility"
    );
}

/// renderForm renders ALL fields (including hidden ones) so recomputeVisibility
/// can toggle them. Previously, hidden fields were skipped at render time, so
/// the DOM elements never existed for the toggle to target.
#[tokio::test]
async fn app_js_renders_all_fields_with_hidden_attribute() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    // The fix: group.hidden = !visible.has(field.name) is set for every field,
    // no longer skipping hidden fields before appending to the form.
    assert!(
        body.contains("group.hidden = !visible.has(field.name)"),
        "app.js must set group.hidden on every field-group div (not skip non-visible fields)"
    );
}

/// app.js fetches presets on form open via GET /api/v1/presets/<module>.
#[tokio::test]
async fn app_js_fetches_presets_on_form_open() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("\"/api/v1/presets/\""),
        "app.js must fetch GET /api/v1/presets/<module> when opening the form"
    );
}

/// app.js renders a <none> chip in the preset row to allow clearing the form.
#[tokio::test]
async fn app_js_has_none_preset_chip() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("<none>"),
        "app.js must include a '<none>' chip as the first preset option"
    );
}

/// app.js respects preset_exclude when applying a preset.
#[tokio::test]
async fn app_js_preset_apply_respects_exclude() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("preset_exclude"),
        "app.js applyPreset must check field.preset_exclude before overwriting"
    );
}

/// app.js calls recomputeVisibility after preset apply so show_when fields update.
#[tokio::test]
async fn app_js_preset_apply_triggers_recompute() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    // applyPreset calls recomputeVisibility at its end
    assert!(
        body.contains("recomputeVisibility(fields, allVals)"),
        "app.js applyPreset must call recomputeVisibility after applying values"
    );
}

/// app.js has the idempotency-key-persists comment AND _pendingIdempotencyKey pattern.
/// Guards against regression where a new key is generated on every submit tap,
/// which would cause duplicate vault entries on 5xx-then-success retry cycles.
#[tokio::test]
async fn app_js_idempotency_key_persists_across_retries() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("// idempotency-key persists across retries"),
        "app.js must contain the idempotency persistence comment"
    );
    assert!(
        body.contains("_pendingIdempotencyKey"),
        "app.js must use _pendingIdempotencyKey to persist the key across retries"
    );
}

/// GET /static/icon.svg returns SVG content.
#[tokio::test]
async fn static_icon_svg_served() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/static/icon.svg")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("image/svg"),
        "icon.svg must return image/svg+xml; got: {ct:?}"
    );
}

// ---------------------------------------------------------------------------
// TASK-2.2.1: /sw.js route — status, headers, content-type, no auth
// ---------------------------------------------------------------------------

/// GET /sw.js returns 200 without any auth token (it's a static asset per §3).
#[tokio::test]
async fn sw_js_served_without_auth() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/sw.js")).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "GET /sw.js must return 200 with no auth token (static asset per §3)"
    );
}

/// GET /sw.js returns Content-Type: application/javascript; charset=utf-8
/// (contract §12 — service workers must be served as JS or browsers refuse them).
#[tokio::test]
async fn sw_js_returns_javascript_content_type() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/sw.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("javascript"),
        "GET /sw.js must return application/javascript; got: {ct:?}"
    );
}

/// GET /sw.js carries Cache-Control: no-cache, max-age=0, must-revalidate
/// (contract §12 — same as shell HTML so the browser re-validates on every load).
#[tokio::test]
async fn sw_js_has_no_cache_header() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/sw.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "no-cache, max-age=0, must-revalidate",
        "/sw.js must use no-cache revalidation per §12; got: {cc:?}"
    );
}

/// GET /sw.js must NOT carry Cache-Control: no-store (that is for /api/* only).
#[tokio::test]
async fn sw_js_does_not_have_no_store() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/sw.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        !cc.contains("no-store"),
        "GET /sw.js must not contain no-store; got: {cc:?}"
    );
}

/// GET /sw.js body is non-empty and contains the CACHE_VERSION constant.
/// This guards against serving an empty stub or the wrong file.
#[tokio::test]
async fn sw_js_body_contains_cache_version() {
    let router = make_router(test_state());
    let resp = router.oneshot(get_req("/sw.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 524288).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap_or("");
    assert!(
        body.contains("CACHE_VERSION"),
        "/sw.js must contain CACHE_VERSION constant; got first 200 chars: {:?}",
        &body[..body.len().min(200)]
    );
}

/// GET /sw.js is served at root scope, NOT under /static/.
/// A SW at /static/sw.js can only control /static/* paths — it cannot
/// intercept navigation to / or form submits to /api/v1/submit/*.
#[tokio::test]
async fn sw_js_at_root_scope_not_static() {
    let router = make_router(test_state());
    // /sw.js at root → 200
    let resp = router.clone().oneshot(get_req("/sw.js")).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "/sw.js must be accessible at root scope"
    );
    // /static/sw.js → 404 (not embedded under static/)
    let resp2 = router.oneshot(get_req("/static/sw.js")).await.unwrap();
    assert_eq!(
        resp2.status(),
        StatusCode::NOT_FOUND,
        "/static/sw.js must return 404 — SW must only be served at root scope"
    );
}
