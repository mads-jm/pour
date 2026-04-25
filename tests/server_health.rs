// Integration test for GET /api/health.
//
// Uses axum's in-process test approach: build the Router, drive it with
// `tower::ServiceExt::oneshot` — no TCP socket needed.

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use tower::ServiceExt as _;

use pour::server::{AppState, handlers};
use pour::transport::TransportMode;

fn test_state(mode: TransportMode) -> AppState {
    AppState {
        transport_mode: mode,
        token: "test-token".to_string(),
    }
}

fn make_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get};
    use pour::server::auth;

    Router::new()
        .route("/api/health", get(handlers::health::handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state)
}

fn bearer(token: &str) -> Request<axum::body::Body> {
    Request::builder()
        .uri("/api/health")
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

// ---------------------------------------------------------------------------
// Existing 5 tests (unchanged)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_200_with_correct_shape_api_mode() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let resp = router.oneshot(bearer("test-token")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["transport_mode"], "API");
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn health_returns_200_with_correct_shape_filesystem_mode() {
    let state = test_state(TransportMode::FileSystem);
    let router = make_router(state);

    let resp = router.oneshot(bearer("test-token")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["transport_mode"], "FileSystem");
}

#[tokio::test]
async fn health_rejects_wrong_token() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = bearer("wrong-token");
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_rejects_missing_auth() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_accepts_query_token() {
    let state = test_state(TransportMode::FileSystem);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health?token=test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// New 6 tests — auth matrix coverage
// ---------------------------------------------------------------------------

/// Wrong query token alone → 401.
#[tokio::test]
async fn health_rejects_wrong_query_token() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health?token=bad-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Wrong query token + correct Bearer header → 200.
/// Proves that header is authoritative and query is ignored when header is present.
#[tokio::test]
async fn health_correct_bearer_overrides_wrong_query_token() {
    let state = test_state(TransportMode::FileSystem);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health?token=bad-query-token")
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_transport_mode(resp.into_body(), "FileSystem").await;
}

/// `Authorization: Bearer ` (trailing space, empty suffix) → 401.
/// Empty suffix is treated as absent; query also absent → reject.
#[tokio::test]
async fn health_rejects_bearer_with_empty_token() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health")
        .header("Authorization", "Bearer ")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// `Authorization: <raw-token>` (missing `Bearer ` prefix) → 401.
/// Without the prefix the header is not a valid Bearer credential.
#[tokio::test]
async fn health_rejects_authorization_without_bearer_prefix() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health")
        .header("Authorization", "test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Correct query token alone (no Authorization header) → 200.
/// This is the QR-bootstrap path: phone first scan arrives with ?token=.
#[tokio::test]
async fn health_accepts_correct_query_token_alone() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health?token=test-token")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_transport_mode(resp.into_body(), "API").await;
}

/// Empty query token `?token=` (key present, value empty) → 401.
#[tokio::test]
async fn health_rejects_empty_query_token() {
    let state = test_state(TransportMode::Api);
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/health?token=")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
