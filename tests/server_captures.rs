// Integration tests for GET /api/v1/captures/{history_id} (§6.6).
//
// These tests use POUR_HOME to isolate history I/O. Tests that touch POUR_HOME
// must run serially — serialized via ENV_LOCK.
//
// Happy-path tests wire a full Router (submit + captures) so the history_id
// returned by submit can be fed back to captures in the same process.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tokio::sync::Mutex;
use tower::ServiceExt as _;

use pour::server::{AppState, handlers, not_found_handler};
use pour::transport::TransportMode;

// ---------------------------------------------------------------------------
// Env serialization helpers (same pattern as tests/paths.rs)
// ---------------------------------------------------------------------------

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct EnvGuard {
    key: &'static str,
    prior: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prior = std::env::var(key).ok();
        // SAFETY: set_var is process-global. Access is serialized via ENV_LOCK
        // within this test file, but other test binaries may run concurrently.
        // Races are benign here because every test points POUR_HOME at its own tempdir.
        unsafe { std::env::set_var(key, value) };
        EnvGuard { key, prior }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: set_var/remove_var are process-global. Access is serialized
        // via ENV_LOCK within this file; races with other test files are benign
        // because each test points POUR_HOME at its own tempdir.
        unsafe {
            match &self.prior {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Router / state helpers
// ---------------------------------------------------------------------------

/// Convert a platform path to forward-slash string safe for embedding in TOML.
fn fwd(p: &std::path::Path) -> String {
    p.to_str().unwrap().replace('\\', "/")
}

fn make_state(config: pour::config::Config, base: &std::path::Path) -> AppState {
    use pour::data::presets::Presets;
    use pour::server::idempotency::IdempotencyCache;
    use pour::transport::{Transport, fs::FsWriter};
    AppState {
        transport_mode: TransportMode::FileSystem,
        token: "test-token".to_string(),
        config: Arc::new(config),
        transport: Arc::new(Transport::Fs(FsWriter::new(base.to_path_buf()))),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(tokio::sync::Mutex::new(Presets::empty())),
    }
}

/// Router with both submit and captures routes wired up.
fn make_full_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get, routing::post};
    use pour::server::{auth, method_not_allowed_handler, no_store_middleware};

    let api = Router::new()
        .route(
            "/api/v1/submit/:module",
            post(handlers::submit::handler)
                .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)),
        )
        .route(
            "/api/v1/captures/:history_id",
            get(handlers::captures::handler),
        )
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    Router::new()
        .merge(api)
        .fallback(not_found_handler)
        .with_state(state)
}

/// Router with captures only (for tests that need no submit).
fn make_captures_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get};
    use pour::server::{auth, method_not_allowed_handler, no_store_middleware};

    let api = Router::new()
        .route(
            "/api/v1/captures/:history_id",
            get(handlers::captures::handler),
        )
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    Router::new()
        .merge(api)
        .fallback(not_found_handler)
        .with_state(state)
}

fn bearer_get(uri: &str) -> Request<axum::body::Body> {
    Request::builder()
        .uri(uri)
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap()
}

async fn body_json(body: axum::body::Body) -> serde_json::Value {
    let bytes = to_bytes(body, 128 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn assert_error_code(body: axum::body::Body, expected_code: &str) {
    let json = body_json(body).await;
    assert!(
        json["error"].is_object(),
        "expected error envelope, got: {json}"
    );
    assert_eq!(
        json["error"]["code"], expected_code,
        "error code mismatch, got: {json}"
    );
}

fn coffee_config(base_path: &str) -> pour::config::Config {
    let toml = format!(
        r#"
config_version = "0.3.0"
[vault]
base_path = "{base_path}"

[modules.coffee]
mode = "create"
path = "Coffee/note.md"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
required = true
"#
    );
    pour::config::Config::from_toml(&toml).expect("coffee config")
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn captures_rejects_missing_auth() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let base = fwd(tmp.path());
    let state = make_state(coffee_config(&base), tmp.path());
    let router = make_captures_router(state);

    let req = Request::builder()
        .uri("/api/v1/captures/some-id")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_code(resp.into_body(), "unauthorized").await;
}

// ---------------------------------------------------------------------------
// 404 — unknown history_id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn captures_unknown_history_id_returns_404() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let base = fwd(tmp.path());
    let state = make_state(coffee_config(&base), tmp.path());
    let router = make_captures_router(state);

    let resp = router
        .oneshot(bearer_get("/api/v1/captures/nonexistent-id"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_code(resp.into_body(), "not_found").await;
}

// ---------------------------------------------------------------------------
// Happy path — submit then read back
// ---------------------------------------------------------------------------

#[tokio::test]
async fn captures_returns_file_content_after_submit() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let base = fwd(tmp.path());
    let state = make_state(coffee_config(&base), tmp.path());
    let router = make_full_router(state);

    // 1. Submit a capture.
    let submit_req = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(
            json!({ "field_values": { "bean": "Ethiopia Guji" } }).to_string(),
        ))
        .unwrap();
    let submit_resp = router.clone().oneshot(submit_req).await.unwrap();
    assert_eq!(
        submit_resp.status(),
        StatusCode::CREATED,
        "submit should succeed"
    );

    let submit_body = body_json(submit_resp.into_body()).await;
    let history_id = submit_body["history_id"].as_str().unwrap();

    // 2. Read back via captures.
    let captures_resp = router
        .oneshot(bearer_get(&format!("/api/v1/captures/{history_id}")))
        .await
        .unwrap();
    assert_eq!(
        captures_resp.status(),
        StatusCode::OK,
        "captures should return 200"
    );

    let json = body_json(captures_resp.into_body()).await;
    assert_eq!(json["id"], history_id, "id must match the requested history_id");
    assert_eq!(json["module_key"], "coffee");
    assert!(
        json["vault_path"].is_string(),
        "vault_path must be string: {json}"
    );
    assert!(
        json["timestamp"].is_string(),
        "timestamp must be string: {json}"
    );
    assert!(
        json["content"].is_string(),
        "content must be string: {json}"
    );
    assert_eq!(json["transport_mode"], "FileSystem");

    // Content must contain the submitted field value.
    let content = json["content"].as_str().unwrap();
    assert!(
        content.contains("Ethiopia Guji"),
        "content should contain submitted value, got: {content}"
    );
}

// ---------------------------------------------------------------------------
// 404 — vault file deleted after submit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn captures_returns_404_when_vault_file_deleted() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let base = fwd(tmp.path());
    let state = make_state(coffee_config(&base), tmp.path());
    let router = make_full_router(state);

    // 1. Submit to create the file and history record.
    let submit_req = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(
            json!({ "field_values": { "bean": "Kenya AA" } }).to_string(),
        ))
        .unwrap();
    let submit_resp = router.clone().oneshot(submit_req).await.unwrap();
    assert_eq!(submit_resp.status(), StatusCode::CREATED);

    let submit_body = body_json(submit_resp.into_body()).await;
    let history_id = submit_body["history_id"].as_str().unwrap();
    let vault_path = submit_body["vault_path"].as_str().unwrap();

    // 2. Delete the vault file.
    let full_path = tmp.path().join(vault_path);
    std::fs::remove_file(&full_path).expect("test setup: vault file should exist");

    // 3. Captures should return 404.
    let captures_resp = router
        .oneshot(bearer_get(&format!("/api/v1/captures/{history_id}")))
        .await
        .unwrap();
    assert_eq!(
        captures_resp.status(),
        StatusCode::NOT_FOUND,
        "deleted vault file should yield 404"
    );
    assert_error_code(captures_resp.into_body(), "not_found").await;
}

// ---------------------------------------------------------------------------
// Cache-Control: no-store (§12)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn captures_returns_no_store_cache_control_on_404() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let base = fwd(tmp.path());
    let state = make_state(coffee_config(&base), tmp.path());
    let router = make_captures_router(state);

    let resp = router
        .oneshot(bearer_get("/api/v1/captures/no-such-id"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store");
}

#[tokio::test]
async fn captures_returns_no_store_cache_control_on_success() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let base = fwd(tmp.path());
    let state = make_state(coffee_config(&base), tmp.path());
    let router = make_full_router(state);

    // Submit first.
    let submit_req = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(
            json!({ "field_values": { "bean": "Colombia" } }).to_string(),
        ))
        .unwrap();
    let submit_resp = router.clone().oneshot(submit_req).await.unwrap();
    let submit_body = body_json(submit_resp.into_body()).await;
    let history_id = submit_body["history_id"].as_str().unwrap();

    // Read back and check header.
    let captures_resp = router
        .oneshot(bearer_get(&format!("/api/v1/captures/{history_id}")))
        .await
        .unwrap();
    assert_eq!(captures_resp.status(), StatusCode::OK);

    let cc = captures_resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store");
}
