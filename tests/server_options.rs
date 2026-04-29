// Integration tests for GET /api/v1/options/{module}/{field} (§6.3).

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use tower::ServiceExt as _;

use pour::server::{AppState, handlers, not_found_handler};
use pour::transport::TransportMode;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_state_with_fs(config: pour::config::Config, base_path: &str) -> AppState {
    use pour::data::presets::Presets;
    use pour::server::idempotency::IdempotencyCache;
    use pour::transport::{Transport, fs::FsWriter};
    AppState {
        transport_mode: TransportMode::FileSystem,
        token: "test-token".to_string(),
        config: Arc::new(config),
        transport: Arc::new(Transport::Fs(FsWriter::new(std::path::PathBuf::from(
            base_path,
        )))),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(tokio::sync::Mutex::new(Presets::empty())),
    }
}

fn make_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get};
    use pour::server::{auth, method_not_allowed_handler, no_store_middleware};

    let api = Router::new()
        .route(
            "/api/v1/options/:module/:field",
            get(handlers::options::handler),
        )
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    Router::new()
        .merge(api)
        .fallback(not_found_handler)
        .with_state(state)
}

fn bearer(token: &str, uri: &str) -> Request<axum::body::Body> {
    Request::builder()
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .unwrap()
}

async fn body_json(body: axum::body::Body) -> serde_json::Value {
    let bytes = to_bytes(body, 65536).await.unwrap();
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
        "error code mismatch, got: {}",
        json["error"]["code"]
    );
}

fn coffee_config() -> pour::config::Config {
    let toml = r#"
config_version = "0.3.0"
[vault]
base_path = "/test/vault"

[modules.coffee]
mode = "create"
path = "Coffee/%Y%m%d.md"

[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating"

[modules.hidden]
mobile_visible = false
mode = "create"
path = "Hidden/%Y%m%d.md"

[[modules.hidden.fields]]
name = "value"
field_type = "dynamic_select"
prompt = "Value"
source = "Hidden/Values"
"#;
    pour::config::Config::from_toml(toml).expect("coffee config")
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn options_rejects_missing_auth() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/options/coffee/bean")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_code(resp.into_body(), "unauthorized").await;
}

// ---------------------------------------------------------------------------
// 404 cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn options_unknown_module_returns_404() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/nonexistent/bean"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_code(resp.into_body(), "not_found").await;
}

#[tokio::test]
async fn options_unknown_field_returns_404() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let resp = router
        .oneshot(bearer(
            "test-token",
            "/api/v1/options/coffee/nonexistent_field",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_code(resp.into_body(), "not_found").await;
}

#[tokio::test]
async fn options_mobile_invisible_module_returns_404() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/hidden/value"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_code(resp.into_body(), "not_found").await;
}

// ---------------------------------------------------------------------------
// 400 — not a dynamic_select
// ---------------------------------------------------------------------------

#[tokio::test]
async fn options_non_dynamic_select_returns_400_validation_failed() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    // `notes` is a textarea, not a dynamic_select
    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/coffee/notes"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_error_code(resp.into_body(), "validation_failed").await;
}

#[tokio::test]
async fn options_number_field_returns_400_validation_failed() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/coffee/rating"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_error_code(resp.into_body(), "validation_failed").await;
}

// ---------------------------------------------------------------------------
// 200 — tier: "empty" (source dir doesn't exist on fs)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn options_empty_tier_when_source_not_found() {
    // /tmp/Coffee/Beans does not exist → transport fails → cache empty → empty tier
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/coffee/bean"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    assert!(json["options"].is_array(), "options must be array");
    // Tier is either "cache" (if state cache has data) or "empty"
    let tier = json["tier"].as_str().unwrap_or("");
    assert!(
        tier == "empty" || tier == "cache" || tier == "transport",
        "unexpected tier: {tier}"
    );
    assert!(
        json["source_path"].is_string(),
        "source_path must be string"
    );
}

// ---------------------------------------------------------------------------
// 200 — tier: "transport" (source dir exists with .md files)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn options_transport_tier_when_source_dir_has_files() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_str().unwrap();

    // Create Coffee/Beans directory with .md files
    let beans_dir = tmp.path().join("Coffee").join("Beans");
    std::fs::create_dir_all(&beans_dir).unwrap();
    std::fs::write(beans_dir.join("Ethiopia Guji.md"), "---\n---\n").unwrap();
    std::fs::write(beans_dir.join("Kenya.md"), "---\n---\n").unwrap();

    let state = make_state_with_fs(coffee_config(), base);
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/coffee/bean"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    assert_eq!(json["tier"], "transport");
    let options = json["options"].as_array().unwrap();
    assert!(options.len() >= 2, "should have at least 2 options");
    let names: Vec<&str> = options.iter().map(|o| o.as_str().unwrap()).collect();
    assert!(
        names.contains(&"Ethiopia Guji"),
        "should contain Ethiopia Guji"
    );
    assert!(names.contains(&"Kenya"), "should contain Kenya");
    assert_eq!(json["source_path"], "Coffee/Beans");
}

// ---------------------------------------------------------------------------
// Response shape
// ---------------------------------------------------------------------------

#[tokio::test]
async fn options_response_has_required_fields() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/coffee/bean"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    assert!(json["options"].is_array(), "options must be array");
    assert!(
        json["source_path"].is_string(),
        "source_path must be string"
    );
    assert!(json["tier"].is_string(), "tier must be string");
}

// ---------------------------------------------------------------------------
// Cache-Control: no-store (§12)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn options_returns_no_store_cache_control() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/coffee/bean"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store");
}

#[tokio::test]
async fn options_error_response_no_store_cache_control() {
    let state = make_state_with_fs(coffee_config(), "/tmp");
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("test-token", "/api/v1/options/nonexistent/bean"))
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
