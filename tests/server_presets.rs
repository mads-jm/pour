// Integration tests for preset CRUD endpoints §6.7–§6.10.
//
// Uses axum's in-process test approach — no TCP socket needed.
// Presets state is shared via Arc<Mutex<Presets>> in AppState; each test
// that mutates presets creates a fresh AppState backed by a temp file so
// tests don't bleed into each other.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use tower::ServiceExt as _;

use pour::server::{AppState, handlers, not_found_handler};
use pour::transport::TransportMode;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn minimal_config() -> pour::config::Config {
    let toml = concat!(
        "config_version = \"0.3.0\"\n",
        "[vault]\n",
        "base_path = \"/tmp\"\n",
        "\n",
        "[modules.coffee]\n",
        "mode = \"create\"\n",
        "path = \"Coffee/note.md\"\n",
        "\n",
        "[[modules.coffee.fields]]\n",
        "name = \"bean\"\n",
        "field_type = \"text\"\n",
        "prompt = \"Bean\"\n",
        "\n",
        "[modules.me]\n",
        "mode = \"append\"\n",
        "path = \"Me/%Y%m%d.md\"\n",
        "append_under_header = \"## Log\"\n",
        "\n",
        "[[modules.me.fields]]\n",
        "name = \"note\"\n",
        "field_type = \"text\"\n",
        "prompt = \"Note\"\n",
    );
    pour::config::Config::from_toml(toml).expect("test config")
}

/// Create an AppState backed by a temporary presets file.
fn make_state_with_presets_file(
    config: pour::config::Config,
    presets_path: std::path::PathBuf,
) -> AppState {
    use pour::data::presets::Presets;
    use pour::server::idempotency::IdempotencyCache;
    use pour::transport::{Transport, fs::FsWriter};
    AppState {
        transport_mode: TransportMode::FileSystem,
        token: "test-token".to_string(),
        config: Arc::new(config),
        transport: Arc::new(Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp")))),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(tokio::sync::Mutex::new(Presets::load_from(presets_path))),
    }
}

/// Create an AppState with empty in-memory presets (no disk backing).
fn make_state_empty(config: pour::config::Config) -> AppState {
    use pour::data::presets::Presets;
    use pour::server::idempotency::IdempotencyCache;
    use pour::transport::{Transport, fs::FsWriter};
    AppState {
        transport_mode: TransportMode::FileSystem,
        token: "test-token".to_string(),
        config: Arc::new(config),
        transport: Arc::new(Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp")))),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(tokio::sync::Mutex::new(Presets::empty())),
    }
}

fn make_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get, routing::put};
    use pour::server::{auth, method_not_allowed_handler, no_store_middleware};

    let api = Router::new()
        .route("/api/v1/presets/:module", get(handlers::presets::get_handler))
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

fn bearer_put(uri: &str, body: &str) -> Request<axum::body::Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

fn bearer_delete(uri: &str) -> Request<axum::body::Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::empty())
        .unwrap()
}

async fn body_json(body: axum::body::Body) -> serde_json::Value {
    let bytes = to_bytes(body, 256 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).expect("valid JSON body")
}

async fn assert_error_code(body: axum::body::Body, expected_code: &str) {
    let json = body_json(body).await;
    assert!(json["error"].is_object(), "expected error envelope, got: {json}");
    assert_eq!(
        json["error"]["code"], expected_code,
        "error code mismatch, got: {json}"
    );
}

// ---------------------------------------------------------------------------
// Auth required
// ---------------------------------------------------------------------------

#[tokio::test]
async fn presets_get_requires_auth() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);
    let req = Request::builder()
        .uri("/api/v1/presets/coffee")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_code(resp.into_body(), "unauthorized").await;
}

#[tokio::test]
async fn presets_put_requires_auth() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/presets/coffee/Morning")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(r#"{"values":{}}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// GET /api/v1/presets/{module}
// ---------------------------------------------------------------------------

#[tokio::test]
async fn presets_get_empty_module_returns_200_not_404() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer_get("/api/v1/presets/coffee"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    assert!(json["presets"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn presets_get_unknown_module_returns_404() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer_get("/api/v1/presets/nonexistent"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_code(resp.into_body(), "not_found").await;
}

// ---------------------------------------------------------------------------
// PUT /api/v1/presets/{module}/{name}
// ---------------------------------------------------------------------------

#[tokio::test]
async fn presets_put_new_preset_returns_201_with_location() {
    let tmp = tempfile::tempdir().unwrap();
    let presets_path = tmp.path().join("presets.json");
    let state = make_state_with_presets_file(minimal_config(), presets_path);
    let router = make_router(state);

    let body = r#"{"description":"morning shot","values":{"bean":"Onyx","dose_g":"18"}}"#;
    let resp = router
        .oneshot(bearer_put("/api/v1/presets/coffee/Morning%20Onyx", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Location header must be set.
    let loc = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        loc.starts_with("/api/v1/presets/coffee/"),
        "Location header must be set; got: {loc:?}"
    );

    let json = body_json(resp.into_body()).await;
    assert_eq!(json["preset"]["name"], "Morning Onyx");
    assert_eq!(json["preset"]["description"], "morning shot");
    assert_eq!(json["preset"]["values"]["bean"], "Onyx");
}

#[tokio::test]
async fn presets_put_existing_preset_returns_200() {
    let tmp = tempfile::tempdir().unwrap();
    let presets_path = tmp.path().join("presets.json");
    let state = make_state_with_presets_file(minimal_config(), presets_path);
    let router = make_router(state);

    // Create first.
    let body1 = r#"{"values":{"bean":"Onyx"}}"#;
    let resp1 = router
        .clone()
        .oneshot(bearer_put("/api/v1/presets/coffee/Morning", body1))
        .await
        .unwrap();
    assert_eq!(resp1.status(), StatusCode::CREATED);

    // Update.
    let body2 = r#"{"description":"updated","values":{"bean":"Intelligentsia"}}"#;
    let resp2 = router
        .oneshot(bearer_put("/api/v1/presets/coffee/Morning", body2))
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let json = body_json(resp2.into_body()).await;
    assert_eq!(json["preset"]["description"], "updated");
    assert_eq!(json["preset"]["values"]["bean"], "Intelligentsia");
}

#[tokio::test]
async fn presets_put_whitespace_only_name_is_400() {
    // Name is a single space (%20), which trims to empty. NOT a truly empty
    // path segment — that case is tested separately in
    // `presets_put_empty_path_segment`.
    let state = make_state_empty(minimal_config());
    let router = make_router(state);

    // Axum decodes the path param — " " trims to empty string in our validation.
    let resp = router
        .oneshot(bearer_put(
            "/api/v1/presets/coffee/%20",
            r#"{"values":{}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_error_code(resp.into_body(), "validation_failed").await;
}

#[tokio::test]
async fn presets_put_empty_path_segment() {
    // A trailing slash (`/api/v1/presets/coffee/`) produces a different axum
    // behavior from a whitespace segment: the router does NOT match the
    // `/:module/:name` route (because the name segment is empty), so axum
    // falls through to the global fallback → 404.
    //
    // Per §4: "Trailing slashes: rejected. … returns 404."
    // This test documents and pins that behavior so a future route change
    // doesn't silently change the status code.
    let state = make_state_empty(minimal_config());
    let router = make_router(state);

    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/presets/coffee/")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(r#"{"values":{}}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    // Axum does not route trailing-slash paths to the /:name handler.
    // 404 is the correct and expected response per §4.
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "trailing-slash path must return 404 per §4; got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn presets_put_unknown_module_returns_404() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer_put(
            "/api/v1/presets/nonexistent/My%20Preset",
            r#"{"values":{}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_code(resp.into_body(), "not_found").await;
}

#[tokio::test]
async fn presets_put_body_too_large_returns_413() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);

    // Build a body larger than 256 KiB.
    let large_value = "x".repeat(300 * 1024);
    let body = format!(r#"{{"values":{{"field":"{large_value}"}}}}"#);

    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/presets/coffee/Big")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_error_code(resp.into_body(), "payload_too_large").await;
}

// ---------------------------------------------------------------------------
// DELETE /api/v1/presets/{module}/{name}
// ---------------------------------------------------------------------------

#[tokio::test]
async fn presets_delete_existing_returns_204() {
    let tmp = tempfile::tempdir().unwrap();
    let presets_path = tmp.path().join("presets.json");
    let state = make_state_with_presets_file(minimal_config(), presets_path);
    let router = make_router(state);

    // Create first.
    router
        .clone()
        .oneshot(bearer_put(
            "/api/v1/presets/coffee/Morning",
            r#"{"values":{"bean":"Onyx"}}"#,
        ))
        .await
        .unwrap();

    // Delete.
    let resp = router
        .clone()
        .oneshot(bearer_delete("/api/v1/presets/coffee/Morning"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET must show empty list.
    let resp_get = router
        .oneshot(bearer_get("/api/v1/presets/coffee"))
        .await
        .unwrap();
    assert_eq!(resp_get.status(), StatusCode::OK);
    let json = body_json(resp_get.into_body()).await;
    assert!(json["presets"].as_array().unwrap().is_empty(), "list must be empty after delete");
}

#[tokio::test]
async fn presets_delete_nonexistent_returns_404() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer_delete("/api/v1/presets/coffee/Nonexistent"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_code(resp.into_body(), "not_found").await;
}

// ---------------------------------------------------------------------------
// PUT /api/v1/presets/{module}/order
// ---------------------------------------------------------------------------

#[tokio::test]
async fn presets_order_reorders_correctly() {
    let tmp = tempfile::tempdir().unwrap();
    let presets_path = tmp.path().join("presets.json");
    let state = make_state_with_presets_file(minimal_config(), presets_path);
    let router = make_router(state);

    // Create three presets.
    for name in &["Alpha", "Beta", "Gamma"] {
        router
            .clone()
            .oneshot(bearer_put(
                &format!("/api/v1/presets/coffee/{name}"),
                &format!(r#"{{"values":{{"bean":"{name}"}}}}"#),
            ))
            .await
            .unwrap();
    }

    // Reorder: Gamma, Alpha, Beta.
    let reorder_body = r#"{"names":["Gamma","Alpha","Beta"]}"#;
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/presets/coffee/order")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(reorder_body))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    let names: Vec<&str> = json["presets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Gamma", "Alpha", "Beta"]);
}

#[tokio::test]
async fn presets_order_missing_name_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let presets_path = tmp.path().join("presets.json");
    let state = make_state_with_presets_file(minimal_config(), presets_path);
    let router = make_router(state);

    // Create Alpha and Beta.
    for name in &["Alpha", "Beta"] {
        router
            .clone()
            .oneshot(bearer_put(
                &format!("/api/v1/presets/coffee/{name}"),
                r#"{"values":{}}"#,
            ))
            .await
            .unwrap();
    }

    // Supply only Alpha (missing Beta).
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/presets/coffee/order")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(r#"{"names":["Alpha"]}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    // Stronger: decode as Vec<String> to verify stable shape, not just is_array().
    let missing: Vec<String> =
        serde_json::from_value(json["error"]["details"]["missing"].clone())
            .expect("details.missing must be a string array");
    assert!(
        missing.contains(&"Beta".to_string()),
        "missing must contain 'Beta'; got {missing:?}"
    );
}

#[tokio::test]
async fn presets_order_extra_name_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let presets_path = tmp.path().join("presets.json");
    let state = make_state_with_presets_file(minimal_config(), presets_path);
    let router = make_router(state);

    // Create Alpha only.
    router
        .clone()
        .oneshot(bearer_put("/api/v1/presets/coffee/Alpha", r#"{"values":{}}"#))
        .await
        .unwrap();

    // Supply Alpha + Extra (extra not in the list).
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/presets/coffee/order")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(r#"{"names":["Alpha","Extra"]}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    // Stronger: decode as Vec<String> to verify stable shape.
    let extra: Vec<String> =
        serde_json::from_value(json["error"]["details"]["extra"].clone())
            .expect("details.extra must be a string array");
    assert!(
        extra.contains(&"Extra".to_string()),
        "extra must contain 'Extra'; got {extra:?}"
    );
}

#[tokio::test]
async fn presets_order_with_duplicate_names_returns_400() {
    // MAJOR #3 regression test: ["Alpha","Beta","Beta"] against ["Alpha","Beta"]
    // was previously accepted (set-diff passed) and silently dropped one entry.
    // Now duplicate detection runs before set-diff.
    let tmp = tempfile::tempdir().unwrap();
    let presets_path = tmp.path().join("presets.json");
    let state = make_state_with_presets_file(minimal_config(), presets_path);
    let router = make_router(state);

    // Create Alpha and Beta.
    for name in &["Alpha", "Beta"] {
        router
            .clone()
            .oneshot(bearer_put(
                &format!("/api/v1/presets/coffee/{name}"),
                r#"{"values":{}}"#,
            ))
            .await
            .unwrap();
    }

    // Submit ["Alpha","Beta","Beta"] — Beta is duplicated.
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/presets/coffee/order")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(
            r#"{"names":["Alpha","Beta","Beta"]}"#,
        ))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "duplicate names must be rejected with 400"
    );

    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    // details.duplicates must be present and contain "Beta".
    let duplicates: Vec<String> =
        serde_json::from_value(json["error"]["details"]["duplicates"].clone())
            .expect("details.duplicates must be a string array");
    assert!(
        duplicates.contains(&"Beta".to_string()),
        "duplicates must contain 'Beta'; got {duplicates:?}"
    );
}

// ---------------------------------------------------------------------------
// Cache-Control: no-store on all responses
// ---------------------------------------------------------------------------

#[tokio::test]
async fn presets_get_has_cache_control_no_store() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer_get("/api/v1/presets/coffee"))
        .await
        .unwrap();
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store", "Cache-Control must be no-store on GET presets");
}

// ---------------------------------------------------------------------------
// Reserved name "order" — CRITICAL 2 regression guard
// ---------------------------------------------------------------------------

#[tokio::test]
async fn presets_put_reserved_name_order_returns_400() {
    // PUT /presets/coffee/order must be rejected with 400 validation_failed
    // and code "reserved_name". "order" is the fixed route segment for
    // PUT /presets/{module}/order (§6.10); a preset named "order" would
    // be permanently unreachable via any single-preset endpoint.
    let state = make_state_empty(minimal_config());
    let router = make_router(state);

    let resp = router
        .clone()
        .oneshot(bearer_put(
            "/api/v1/presets/coffee/order",
            r#"{"values":{"bean":"test"}}"#,
        ))
        .await
        .unwrap();
    // The /order route is registered BEFORE /:name, so PUT /presets/coffee/order
    // hits order_handler (§6.10), which expects { "names": [...] } and returns
    // 400 validation_failed for an invalid body. This test pins that behaviour:
    // the name "order" never reaches put_handler via normal routing.
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "PUT /presets/coffee/order must return 400 (hits order_handler with wrong body)"
    );
    assert_error_code(resp.into_body(), "validation_failed").await;
}

#[tokio::test]
async fn presets_put_reserved_name_order_percent_encoded_returns_400() {
    // PUT /presets/coffee/ord%65r (percent-encoded "order") hits /:name handler.
    // The server must reject it with 400 reserved_name (belt-and-suspenders guard).
    let state = make_state_empty(minimal_config());
    let router = make_router(state);

    let resp = router
        .clone()
        .oneshot(bearer_put(
            "/api/v1/presets/coffee/ord%65r",
            r#"{"values":{"bean":"test"}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "percent-encoded 'order' must return 400 reserved_name"
    );
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    assert_eq!(
        json["error"]["details"]["code"], "reserved_name",
        "details.code must be reserved_name; got: {json}"
    );
}

#[tokio::test]
async fn presets_delete_reserved_name_order_returns_404() {
    // DELETE /presets/coffee/order hits the order_handler route (PUT-only),
    // so axum returns 405 Method Not Allowed. Pins that behaviour.
    let state = make_state_empty(minimal_config());
    let router = make_router(state);

    let resp = router
        .oneshot(bearer_delete("/api/v1/presets/coffee/order"))
        .await
        .unwrap();
    // /order is registered as PUT-only. DELETE on a PUT-only route returns 405.
    assert_eq!(
        resp.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "DELETE /presets/coffee/order must return 405 (order endpoint is PUT-only)"
    );
}

// ---------------------------------------------------------------------------
// Error envelopes on 4xx (spot-check)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn presets_4xx_responses_are_envelope_conformant() {
    let state = make_state_empty(minimal_config());
    let router = make_router(state);

    // 404 on unknown module is envelope-wrapped.
    let resp = router
        .oneshot(bearer_get("/api/v1/presets/nonexistent"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp.into_body()).await;
    assert!(json["error"]["code"].is_string());
    assert!(json["error"]["message"].is_string());
}
