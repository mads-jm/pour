// Integration test for GET /api/v1/health.
//
// Uses axum's in-process test approach: build the Router, drive it with
// `tower::ServiceExt::oneshot` — no TCP socket needed.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use tower::ServiceExt as _;

use pour::server::{AppState, handlers, not_found_handler};
use pour::transport::TransportMode;

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

fn test_state(mode: TransportMode) -> AppState {
    use pour::data::presets::Presets;
    use pour::server::idempotency::IdempotencyCache;
    use pour::transport::{Transport, fs::FsWriter};
    AppState {
        transport_mode: mode,
        token: "test-token".to_string(),
        config: Arc::new(minimal_config()),
        transport: Arc::new(Transport::Fs(FsWriter::new(std::path::PathBuf::from(
            "/tmp",
        )))),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(tokio::sync::Mutex::new(Presets::empty())),
    }
}

/// Mirrors the production router topology: route_layer (auth on known routes only),
/// method_not_allowed_fallback, no_store_middleware (outermost), and the global
/// not_found_handler fallback outside auth.
fn make_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get};
    use pour::server::{auth, method_not_allowed_handler, no_store_middleware};

    let api = Router::new()
        .route("/api/v1/health", get(handlers::health::handler))
        .route("/api/v1/config", get(handlers::config::handler))
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    Router::new()
        .merge(api)
        .fallback(not_found_handler)
        .with_state(state)
}

fn bearer(token: &str) -> Request<axum::body::Body> {
    Request::builder()
        .uri("/api/v1/health")
        .header("Authorization", format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .unwrap()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse the response body as JSON and assert transport_mode matches expected.
async fn assert_transport_mode(body: axum::body::Body, expected: &str) {
    let bytes = to_bytes(body, 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        json["transport_mode"], expected,
        "transport_mode mismatch: expected {expected:?}, got {}",
        json["transport_mode"]
    );
}

/// Assert the response body is a §5.2 error envelope with the given code.
async fn assert_error_envelope(body: axum::body::Body, expected_code: &str) {
    let bytes = to_bytes(body, 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        json["error"].is_object(),
        "response must have top-level 'error' object; got: {json}"
    );
    assert_eq!(
        json["error"]["code"], expected_code,
        "error.code mismatch: expected {expected_code:?}, got {}",
        json["error"]["code"]
    );
    assert!(
        json["error"]["message"].is_string(),
        "error.message must be a string"
    );
}

// ---------------------------------------------------------------------------
// Tests — health shape and auth (migrated to /api/v1/health)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_200_with_correct_shape_api_mode() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let resp = router.oneshot(bearer("test-token")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["transport_mode"], "API");
    assert!(json["version"].is_string());
    // Step B additions
    assert_eq!(json["schema_version"], "1");
    assert_eq!(json["vault_base_path"], "/test/vault");
    assert!(json["capabilities"].is_array());
    let caps: Vec<&str> = json["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(caps.contains(&"composite_array"));
    assert!(caps.contains(&"create_template"));
    assert!(caps.contains(&"post_create_command"));
    assert!(caps.contains(&"show_when"));
    assert!(caps.contains(&"presets"));
    assert!(caps.contains(&"history"));
    assert!(caps.contains(&"idempotency_key"));
    assert!(caps.contains(&"captured_at"));
    assert_eq!(caps.len(), 8);
}

#[tokio::test]
async fn health_returns_200_with_correct_shape_filesystem_mode() {
    let state = test_state(TransportMode::FileSystem);
    let router = make_router(state);

    let resp = router.oneshot(bearer("test-token")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["transport_mode"], "FileSystem");
    assert_eq!(json["schema_version"], "1");
    assert!(json["capabilities"].is_array());
}

#[tokio::test]
async fn health_rejects_wrong_token_with_envelope() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let resp = router.oneshot(bearer("wrong-token")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_envelope(resp.into_body(), "unauthorized").await;
}

#[tokio::test]
async fn health_rejects_missing_auth_with_envelope() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_envelope(resp.into_body(), "unauthorized").await;
}

#[tokio::test]
async fn health_accepts_query_token() {
    let state = test_state(TransportMode::FileSystem);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health?token=test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Cache-Control header (MAJOR #8)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_cache_control_no_store() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let resp = router.oneshot(bearer("test-token")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "no-store",
        "Cache-Control header must be no-store on /api/v1/health"
    );
}

// ---------------------------------------------------------------------------
// Auth matrix coverage (migrated to /api/v1/health)
// ---------------------------------------------------------------------------

/// Wrong query token alone → 401 with envelope.
#[tokio::test]
async fn health_rejects_wrong_query_token() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health?token=bad-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_envelope(resp.into_body(), "unauthorized").await;
}

/// Wrong query token + correct Bearer header → 200.
/// Proves that header is authoritative and query is ignored when header is present.
#[tokio::test]
async fn health_correct_bearer_overrides_wrong_query_token() {
    let state = test_state(TransportMode::FileSystem);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health?token=bad-query-token")
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_transport_mode(resp.into_body(), "FileSystem").await;
}

/// `Authorization: Bearer ` (trailing space, empty suffix) → 401 with envelope.
/// Empty suffix is treated as absent; query also absent → reject.
#[tokio::test]
async fn health_rejects_bearer_with_empty_token() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health")
        .header("Authorization", "Bearer ")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_envelope(resp.into_body(), "unauthorized").await;
}

/// `Authorization: <raw-token>` (missing `Bearer ` prefix) → 401 with envelope.
/// Without the prefix the header is not a valid Bearer credential.
#[tokio::test]
async fn health_rejects_authorization_without_bearer_prefix() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health")
        .header("Authorization", "test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_envelope(resp.into_body(), "unauthorized").await;
}

/// Correct query token alone (no Authorization header) → 200.
/// This is the QR-bootstrap path: phone first scan arrives with ?token=.
#[tokio::test]
async fn health_accepts_correct_query_token_alone() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health?token=test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_transport_mode(resp.into_body(), "API").await;
}

/// Empty query token `?token=` (key present, value empty) → 401 with envelope.
#[tokio::test]
async fn health_rejects_empty_query_token() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health?token=")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_envelope(resp.into_body(), "unauthorized").await;
}

// ---------------------------------------------------------------------------
// 404 / unknown paths (CRITICAL #4 + MAJOR #5 + MAJOR #6)
// ---------------------------------------------------------------------------

/// Unknown path with NO auth → 404 with envelope (not 401).
/// Proves the fallback is outside the auth middleware.
#[tokio::test]
async fn unknown_path_no_auth_returns_404_with_envelope() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/nonexistent")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_envelope(resp.into_body(), "not_found").await;
}

/// Unknown path WITH valid auth → still 404.
/// Auth doesn't change the outcome for truly unknown paths.
#[tokio::test]
async fn unknown_path_with_auth_returns_404_with_envelope() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/nonexistent")
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_envelope(resp.into_body(), "not_found").await;
}

/// Trailing slash `/api/v1/health/` → 404 (contract §4: no trailing-slash alias).
#[tokio::test]
async fn trailing_slash_health_returns_404() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/health/")
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// The old `/api/health` path (unversioned) → 404 per the contract:
/// "No alias from `/api/health` to `/api/v1/health`."
#[tokio::test]
async fn old_unversioned_health_path_returns_404() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health")
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// OPTIONS → 405 (MAJOR #7 + MAJOR #1 + MAJOR #2 contract requirements)
// ---------------------------------------------------------------------------

/// OPTIONS to a known path → 405 with §5.2 envelope, Allow header, Cache-Control,
/// and Content-Type: application/json.
#[tokio::test]
async fn options_to_known_path_returns_405_with_envelope() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/config")
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);

    // Allow header must be present and include GET (axum sets this automatically).
    let allow = resp
        .headers()
        .get("allow")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        allow.contains("GET"),
        "Allow header must list at least GET; got: {allow:?}"
    );

    // Cache-Control: no-store (§12).
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store", "Cache-Control must be no-store on 405");

    // Content-Type: application/json.
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("application/json"),
        "Content-Type must be application/json on 405; got: {ct:?}"
    );

    // §5.2 envelope with code "method_not_allowed".
    assert_error_envelope(resp.into_body(), "method_not_allowed").await;
}

// ---------------------------------------------------------------------------
// Cache-Control: no-store on error responses (MAJOR #2)
// ---------------------------------------------------------------------------

/// 401 (wrong token) must carry Cache-Control: no-store.
#[tokio::test]
async fn unauthorized_returns_cache_control_no_store() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let resp = router.oneshot(bearer("wrong-token")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store", "Cache-Control must be no-store on 401");
}

/// 404 (unknown path) must carry Cache-Control: no-store.
#[tokio::test]
async fn not_found_returns_cache_control_no_store() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/nonexistent")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store", "Cache-Control must be no-store on 404");
}
