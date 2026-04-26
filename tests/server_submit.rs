// Integration tests for POST /api/v1/submit/{module} (§6.4).
//
// Uses axum's in-process test approach: build the Router, drive it with
// `tower::ServiceExt::oneshot` — no TCP socket needed.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt as _;

use chrono::Timelike as _;
use pour::server::{AppState, handlers, not_found_handler};
use pour::transport::TransportMode;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a platform path to forward-slash string safe for embedding in TOML.
fn fwd(p: &std::path::Path) -> String {
    p.to_str().unwrap().replace('\\', "/")
}

fn make_state(config: pour::config::Config, base: &std::path::Path) -> AppState {
    use pour::server::idempotency::IdempotencyCache;
    use pour::transport::{Transport, fs::FsWriter};
    AppState {
        transport_mode: TransportMode::FileSystem,
        token: "test-token".to_string(),
        config: Arc::new(config),
        transport: Arc::new(Transport::Fs(FsWriter::new(base.to_path_buf()))),
        idempotency: Arc::new(IdempotencyCache::new()),
    }
}

fn make_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::post};
    use pour::server::{auth, method_not_allowed_handler, no_store_middleware};

    let api = Router::new()
        .route(
            "/api/v1/submit/:module",
            post(handlers::submit::handler)
                .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)),
        )
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    Router::new()
        .merge(api)
        .fallback(not_found_handler)
        .with_state(state)
}

fn json_request(
    module: &str,
    token: Option<&str>,
    body: serde_json::Value,
) -> Request<axum::body::Body> {
    let uri = format!("/api/v1/submit/{module}");
    let mut b = Request::builder()
        .method("POST")
        .uri(uri)
        .header("Content-Type", "application/json");
    if let Some(t) = token {
        b = b.header("Authorization", format!("Bearer {t}"));
    }
    b.body(axum::body::Body::from(body.to_string())).unwrap()
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

/// Config with a create-mode module (base_path must already use forward slashes).
fn create_config(base_path: &str) -> pour::config::Config {
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

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating"
"#
    );
    pour::config::Config::from_toml(&toml).expect("create config")
}

/// Config with an append-mode module (base_path must already use forward slashes).
fn append_config(base_path: &str) -> pour::config::Config {
    // NOTE: uses escaped-string format! instead of r#"..."# because TOML values
    // like "## Log" contain the sequence `"#` which terminates the r#"..."# raw
    // string in Rust 2021+.
    let toml = format!(
        "config_version = \"0.3.0\"\n\
         [vault]\n\
         base_path = \"{base_path}\"\n\
         \n\
         [modules.log]\n\
         mode = \"append\"\n\
         path = \"Journal/daily.md\"\n\
         append_under_header = \"## Log\"\n\
         append_template = \"{{body}}\"\n\
         \n\
         [[modules.log.fields]]\n\
         name = \"body\"\n\
         field_type = \"text\"\n\
         prompt = \"Body\"\n\
         required = true\n\
         \n\
         [modules.coffee]\n\
         mode = \"create\"\n\
         path = \"Coffee/note.md\"\n\
         \n\
         [[modules.coffee.fields]]\n\
         name = \"bean\"\n\
         field_type = \"text\"\n\
         prompt = \"Bean\"\n\
         required = true\n"
    );
    pour::config::Config::from_toml(&toml).expect("append config")
}

/// Config with a show_when field to test visibility filtering.
fn visibility_config(base_path: &str) -> pour::config::Config {
    let toml = format!(
        r#"
config_version = "0.3.0"
[vault]
base_path = "{base_path}"

[modules.form]
mode = "create"
path = "Form/note.md"

[[modules.form.fields]]
name = "method"
field_type = "static_select"
prompt = "Method"
options = ["Espresso", "V60"]

[[modules.form.fields]]
name = "espresso_only"
field_type = "text"
prompt = "Espresso Only"
required = true
show_when = {{ field = "method", equals = "Espresso" }}

[[modules.form.fields]]
name = "always_present"
field_type = "text"
prompt = "Always"
required = true
"#
    );
    pour::config::Config::from_toml(&toml).expect("visibility config")
}

/// Config with a dynamic_select field with allow_create.
fn autocreate_config(base_path: &str) -> pour::config::Config {
    let toml = format!(
        r#"
config_version = "0.3.0"
[vault]
base_path = "{base_path}"

[modules.brew]
mode = "create"
path = "Brew/note.md"

[[modules.brew.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Brew/Beans"
allow_create = true
required = true
"#
    );
    pour::config::Config::from_toml(&toml).expect("autocreate config")
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_requires_auth() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "coffee",
        None,
        json!({ "field_values": { "bean": "Ethiopia" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_code(resp.into_body(), "unauthorized").await;
}

// ---------------------------------------------------------------------------
// Content-Type
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_wrong_content_type_returns_415() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "text/plain")
        .body(axum::body::Body::from(r#"{"field_values":{"bean":"x"}}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_error_code(resp.into_body(), "unsupported_media_type").await;
}

#[tokio::test]
async fn submit_missing_content_type_returns_415() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .body(axum::body::Body::from(r#"{"field_values":{"bean":"x"}}"#))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_error_code(resp.into_body(), "unsupported_media_type").await;
}

// ---------------------------------------------------------------------------
// Module lookup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_unknown_module_returns_404() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "nonexistent",
        Some("test-token"),
        json!({ "field_values": {} }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_error_code(resp.into_body(), "not_found").await;
}

// ---------------------------------------------------------------------------
// Field validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_empty_required_field_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({ "field_values": { "bean": "" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    let fields = &json["error"]["details"]["fields"];
    assert!(fields.is_array(), "details.fields must be array: {json}");
    let has_bean = fields
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["field"] == "bean");
    assert!(has_bean, "bean field error must be present: {json}");
}

#[tokio::test]
async fn submit_hidden_required_field_does_not_block() {
    // `espresso_only` is required but show_when=Espresso. With method=V60 it's hidden.
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(visibility_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "form",
        Some("test-token"),
        json!({ "field_values": { "method": "V60", "always_present": "something" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "hidden required field should not block submit"
    );
}

#[tokio::test]
async fn submit_missing_field_values_key_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    // No field_values key at all — should fail deserialization
    let req = json_request("coffee", Some("test-token"), json!({}));
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Happy path create mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_create_mode_returns_201_with_expected_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({ "field_values": { "bean": "Ethiopia Guji" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let json = body_json(resp.into_body()).await;
    assert!(
        json["vault_path"].is_string(),
        "vault_path must be string: {json}"
    );
    assert!(
        json["transport_mode"].is_string(),
        "transport_mode must be string: {json}"
    );
    assert!(
        json["history_id"].is_string(),
        "history_id must be string: {json}"
    );
    assert!(
        json["auto_created"].is_array(),
        "auto_created must be array: {json}"
    );
    assert!(
        json["post_create_commands"].is_array(),
        "post_create_commands must be array: {json}"
    );
    assert!(
        json["captured_at"].is_string(),
        "captured_at must be string: {json}"
    );
}

#[tokio::test]
async fn submit_create_mode_location_header_points_to_history() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({ "field_values": { "bean": "Ethiopia" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location.starts_with("/api/v1/history/"),
        "Location header must start with /api/v1/history/: {location:?}"
    );
}

#[tokio::test]
async fn submit_creates_file_on_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({ "field_values": { "bean": "Colombia" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let json = body_json(resp.into_body()).await;
    let vault_path = json["vault_path"].as_str().unwrap();
    let full_path = tmp.path().join(vault_path);
    assert!(
        full_path.exists(),
        "file should exist at {}",
        full_path.display()
    );
    let content = std::fs::read_to_string(&full_path).unwrap();
    assert!(
        content.contains("Colombia"),
        "file should contain field value"
    );
}

// ---------------------------------------------------------------------------
// Happy path append mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_append_mode_returns_201() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());

    // Pre-create the daily note so append can find the header
    let journal_dir = tmp.path().join("Journal");
    std::fs::create_dir_all(&journal_dir).unwrap();
    std::fs::write(journal_dir.join("daily.md"), "# Journal\n\n## Log\n\n").unwrap();

    let state = make_state(append_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "log",
        Some("test-token"),
        json!({ "field_values": { "body": "Had a great session." } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "append mode should return 201"
    );
}

// ---------------------------------------------------------------------------
// captured_at semantics (§10)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_captured_at_valid_is_echoed_in_response() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    // Use a relative timestamp (1 day ago) so the test stays green regardless
    // of when it runs. The 30-day window check would make a hardcoded date fail
    // ~30 days after the date was written.
    let captured_at = chrono::Utc::now() - chrono::Duration::days(1);
    let captured_at_str = captured_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({
            "field_values": { "bean": "Kenya" },
            "captured_at": captured_at_str
        }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp.into_body()).await;

    // The response echoes the parsed/re-formatted value. Compare via DateTime
    // parse to tolerate sub-second precision differences in formatting.
    let echoed_str = json["captured_at"].as_str().expect("captured_at must be string");
    let echoed: chrono::DateTime<chrono::Utc> = echoed_str.parse()
        .expect("captured_at in response must parse as RFC3339");
    let diff = (echoed - captured_at).num_seconds().abs();
    assert!(
        diff <= 1,
        "echoed captured_at ({echoed_str}) should be within 1 second of sent ({captured_at_str})"
    );
}

#[tokio::test]
async fn submit_captured_at_too_old_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    // 31 days ago — outside the 30-day window
    let old = (chrono::Utc::now() - chrono::Duration::days(31))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({
            "field_values": { "bean": "Kenya" },
            "captured_at": old
        }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    let details = &json["error"]["details"];
    assert_eq!(
        details["code"],
        "captured_at_out_of_range",
        "details.code must be captured_at_out_of_range: {json}"
    );
}

#[tokio::test]
async fn submit_captured_at_too_far_future_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    // 6 minutes in the future — outside the 5-minute tolerance
    let future = (chrono::Utc::now() + chrono::Duration::minutes(6))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({
            "field_values": { "bean": "Kenya" },
            "captured_at": future
        }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    let details = &json["error"]["details"];
    assert_eq!(
        details["code"],
        "captured_at_out_of_range",
        "details.code: {json}"
    );
}

#[tokio::test]
async fn submit_null_captured_at_uses_server_time() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    // Truncate before/after to seconds — the server formats captured_at without
    // sub-second precision, so comparing against full-precision Instant would
    // flake when the sub-second part of `before` exceeds the truncated response.
    let before = chrono::Utc::now().with_nanosecond(0).unwrap();
    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({
            "field_values": { "bean": "Brazil" },
            "captured_at": null
        }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let after = chrono::Utc::now()
        + chrono::Duration::seconds(1); // +1s headroom for clock skew

    let json = body_json(resp.into_body()).await;
    let ca_str = json["captured_at"].as_str().unwrap();
    let ca: chrono::DateTime<chrono::Utc> = ca_str.parse().unwrap();
    assert!(
        ca >= before && ca <= after,
        "captured_at ({ca_str}) should be between before and after"
    );
}

// ---------------------------------------------------------------------------
// Idempotency (§9)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_idempotency_key_invalid_format_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    // Key > 256 chars
    let long_key = "a".repeat(257);
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", long_key)
        .body(axum::body::Body::from(
            json!({ "field_values": { "bean": "Kenya" } }).to_string(),
        ))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_error_code(resp.into_body(), "validation_failed").await;
}

#[tokio::test]
async fn submit_idempotency_key_replay_returns_same_body_with_header() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let key = "test-idempotency-key-123";
    let body = json!({ "field_values": { "bean": "Ethiopia" } }).to_string();

    // First request
    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", key)
        .body(axum::body::Body::from(body.clone()))
        .unwrap();
    let resp1 = router.clone().oneshot(req1).await.unwrap();
    assert_eq!(
        resp1.status(),
        StatusCode::CREATED,
        "first request should be 201"
    );
    let body1 = to_bytes(resp1.into_body(), 65536).await.unwrap();

    // Second request with same key — must be a replay
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", key)
        .body(axum::body::Body::from(body.clone()))
        .unwrap();
    let resp2 = router.oneshot(req2).await.unwrap();
    assert_eq!(
        resp2.status(),
        StatusCode::CREATED,
        "replay should return same status"
    );

    let replay_header = resp2
        .headers()
        .get("idempotency-replay")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        replay_header,
        "true",
        "Idempotency-Replay header must be 'true'"
    );

    let body2 = to_bytes(resp2.into_body(), 65536).await.unwrap();
    assert_eq!(body1, body2, "replay body must match original body");
}

// ---------------------------------------------------------------------------
// Auto-create (bare stub)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_auto_create_bare_stub_novel_value_creates_note() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());

    // Isolate the pour state cache from other parallel tests by pointing
    // POUR_HOME at the temp dir. Without this, Cache::load/save uses the
    // global ~/.pour/cache/state.json which races with other autocreate tests.
    // SAFETY: set_var is process-global. Tests in this binary may run in
    // parallel and could observe a partial update of POUR_HOME. The races are
    // benign here because every test points POUR_HOME at its own tempdir.
    unsafe { std::env::set_var("POUR_HOME", tmp.path()) };

    // Pre-create the beans directory so list_directory can find it
    let beans_dir = tmp.path().join("Brew").join("Beans");
    std::fs::create_dir_all(&beans_dir).unwrap();

    let state = make_state(autocreate_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "brew",
        Some("test-token"),
        json!({ "field_values": { "bean": "Novel Bean" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let json = body_json(resp.into_body()).await;
    let auto_created = json["auto_created"].as_array().unwrap();
    assert_eq!(
        auto_created.len(),
        1,
        "one auto-created note expected: {json}"
    );
    assert_eq!(auto_created[0]["field"], "bean");
    assert_eq!(auto_created[0]["value"], "Novel Bean");
    assert_eq!(auto_created[0]["templated"], false);
}

#[tokio::test]
async fn submit_auto_create_existing_value_is_not_auto_created() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    // SAFETY: set_var is process-global. Tests in this binary may run in
    // parallel and could observe a partial update of POUR_HOME. The races are
    // benign here because every test points POUR_HOME at its own tempdir.
    unsafe { std::env::set_var("POUR_HOME", tmp.path()) };

    // Pre-create an existing bean note
    let beans_dir = tmp.path().join("Brew").join("Beans");
    std::fs::create_dir_all(&beans_dir).unwrap();
    std::fs::write(beans_dir.join("Ethiopia.md"), "---\n---\n").unwrap();

    let state = make_state(autocreate_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "brew",
        Some("test-token"),
        json!({ "field_values": { "bean": "Ethiopia" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp.into_body()).await;
    let auto_created = json["auto_created"].as_array().unwrap();
    assert_eq!(
        auto_created.len(),
        0,
        "no auto-create for existing value: {json}"
    );
}

// ---------------------------------------------------------------------------
// Cache-Control: no-store (§12)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_response_has_no_store_cache_control() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "coffee",
        Some("test-token"),
        json!({ "field_values": { "bean": "Kenya" } }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store");
}

// ---------------------------------------------------------------------------
// MAJOR #5 — additional test coverage
// ---------------------------------------------------------------------------

/// InFlight TTL: a wedged in-flight entry (dropped future) must unblock after
/// the 60-second TTL expires. We use the test helper `insert_stale_in_flight`
/// to inject an already-expired InFlight entry without sleeping.
#[test]
fn idempotency_stale_in_flight_is_treated_as_fresh() {
    use pour::server::idempotency::{IdempotencyCache, IdempotencyOutcome};
    use std::time::Duration;

    let cache = IdempotencyCache::new();
    let key = "stale-in-flight-key";

    // Insert an InFlight entry that is 61 seconds old (past the 60s TTL).
    cache.insert_stale_in_flight(key, Duration::from_secs(61));

    // A subsequent lookup must treat the expired entry as fresh, not InFlight.
    let outcome = cache.get_or_insert_in_flight(key);
    assert!(
        matches!(outcome, IdempotencyOutcome::Fresh),
        "expired InFlight entry must be treated as Fresh, not InFlight or Replay"
    );
}

/// §13 body size limit: payloads over 1 MiB must return 413.
#[tokio::test]
async fn submit_body_too_large_returns_413() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    // Build a raw body that exceeds 1 MiB (1_048_576 bytes). Use raw bytes
    // rather than constructing valid JSON — the body limit check fires before
    // JSON deserialization, so the content doesn't need to be valid JSON.
    const LIMIT: usize = 1024 * 1024; // 1 MiB
    let raw_body = vec![b'x'; LIMIT + 1];

    let body_len = raw_body.len();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        // Provide Content-Length so axum's DefaultBodyLimit can reject based on
        // the declared size before reading the body stream.
        .header("Content-Length", body_len.to_string())
        .body(axum::body::Body::from(raw_body))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "body exceeding 1 MiB must return 413"
    );
}

/// Idempotency in-flight 409: two concurrent requests with the same key.
///
/// We use `tokio::join!` to fire both concurrently. One gets 201 and one
/// gets 409 `idempotency_replay_in_flight`. The ordering is non-deterministic,
/// so we assert the pair sums to exactly one 201 and one 409.
#[tokio::test]
async fn submit_idempotency_in_flight_returns_409() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    // Use the shared router (NOT cloned per request) so both requests see the
    // same idempotency cache.
    let router = make_router(state);

    let key = "concurrent-in-flight-key-xyz";
    let body = json!({ "field_values": { "bean": "In-Flight Bean" } }).to_string();

    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", key)
        .body(axum::body::Body::from(body.clone()))
        .unwrap();

    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", key)
        .body(axum::body::Body::from(body.clone()))
        .unwrap();

    let (resp1, resp2) = tokio::join!(
        router.clone().oneshot(req1),
        router.oneshot(req2),
    );
    let resp1 = resp1.unwrap();
    let resp2 = resp2.unwrap();

    let status1 = resp1.status();
    let status2 = resp2.status();

    // One must succeed, one must be rejected as in-flight or replayed.
    // Acceptable outcomes: (201, 409) or (201, 201-replay) or (409, 201).
    // In practice with no artificial delay, the second will see InFlight.
    let statuses = [status1, status2];
    let has_created = statuses.contains(&StatusCode::CREATED);
    let has_conflict_or_created = statuses
        .iter()
        .all(|&s| s == StatusCode::CREATED || s == StatusCode::CONFLICT);

    assert!(
        has_created,
        "at least one request must succeed with 201: {status1} / {status2}"
    );
    assert!(
        has_conflict_or_created,
        "both responses must be 201 or 409: {status1} / {status2}"
    );
}

/// Config with a dynamic_select field backed by a create_template.
fn autocreate_templated_config(base_path: &str) -> pour::config::Config {
    let toml = format!(
        r#"
config_version = "0.3.0"
[vault]
base_path = "{base_path}"

[templates.bean_template]
path = "Brew/Beans/{{{{name}}}}.md"

[[templates.bean_template.fields]]
name = "origin"
field_type = "text"
prompt = "Origin"

[modules.brew]
mode = "create"
path = "Brew/note.md"

[[modules.brew.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Brew/Beans"
allow_create = true
create_template = "bean_template"
required = true
"#
    );
    pour::config::Config::from_toml(&toml).expect("autocreate templated config")
}

/// Templated auto-create: novel dynamic_select value with create_template + auto_create_inputs.
#[tokio::test]
async fn submit_auto_create_templated_note_has_full_frontmatter() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    // SAFETY: set_var is process-global. Tests in this binary may run in
    // parallel and could observe a partial update of POUR_HOME. The races are
    // benign here because every test points POUR_HOME at its own tempdir.
    unsafe { std::env::set_var("POUR_HOME", tmp.path()) };

    let beans_dir = tmp.path().join("Brew").join("Beans");
    std::fs::create_dir_all(&beans_dir).unwrap();

    let state = make_state(autocreate_templated_config(&base), tmp.path());
    let router = make_router(state);

    let req = json_request(
        "brew",
        Some("test-token"),
        json!({
            "field_values": { "bean": "Templated Bean" },
            "auto_create_inputs": {
                "bean": { "origin": "Ethiopia" }
            }
        }),
    );
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let json = body_json(resp.into_body()).await;
    let auto_created = json["auto_created"].as_array().unwrap();
    assert_eq!(
        auto_created.len(),
        1,
        "one auto-created note expected: {json}"
    );
    assert_eq!(auto_created[0]["field"], "bean");
    assert_eq!(auto_created[0]["templated"], true, "must be templated: {json}");

    // Verify the created note has full frontmatter (origin field).
    let vault_path = auto_created[0]["vault_path"].as_str().unwrap();
    let full_path = tmp.path().join(vault_path);
    assert!(full_path.exists(), "created note must exist at {vault_path}");
    let content = std::fs::read_to_string(&full_path).unwrap();
    assert!(
        content.contains("origin:"),
        "templated note must have frontmatter from template fields, got:\n{content}"
    );
}

/// Auto-create failure: transport write fails → still returns 201 with warning.
#[tokio::test]
async fn submit_auto_create_failure_populates_warnings_and_still_returns_201() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());

    // Make "Brew/Beans" a FILE rather than a directory. This causes two things:
    // 1. list_directory("Brew/Beans") returns an error → the existing-options
    //    list falls back to an empty cache, so "Collision Bean" looks novel.
    // 2. When create_file tries to create "Brew/Beans/Collision Bean.md", the
    //    create_dir_all call on the parent "Brew/Beans" fails because a file
    //    already exists at that path → transport error → warning is emitted.
    let brew_dir = tmp.path().join("Brew");
    std::fs::create_dir_all(&brew_dir).unwrap();
    std::fs::write(brew_dir.join("Beans"), "not a directory").unwrap();

    let state = make_state(autocreate_config(&base), tmp.path());
    let router = make_router(state);

    // "Collision Bean" is novel (empty options list) but create_file will fail
    // because the parent path is a file, not a directory.
    let req = json_request(
        "brew",
        Some("test-token"),
        json!({ "field_values": { "bean": "Collision Bean" } }),
    );
    let resp = router.oneshot(req).await.unwrap();

    // Must still be 201 — autocreate failure is non-fatal.
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "autocreate failure should not block 201"
    );

    let json = body_json(resp.into_body()).await;

    // Warnings must be present and non-empty — the autocreate failure must
    // surface as a warning rather than silently disappearing.
    let warnings = json["warnings"]
        .as_array()
        .expect("response should have a warnings array on autocreate failure");
    assert!(
        !warnings.is_empty(),
        "expected autocreate failure to populate warnings, got empty array: {json}"
    );
    let first_warning = &warnings[0];
    assert_eq!(
        first_warning["code"].as_str(),
        Some("autocreate_failed"),
        "warning code should be autocreate_failed: {json}"
    );
    assert_eq!(
        first_warning["field"].as_str(),
        Some("bean"),
        "warning should name the offending field: {json}"
    );
    assert!(
        first_warning["message"].as_str().is_some(),
        "warning must include a message: {json}"
    );

    // auto_created must be empty (the note was NOT successfully created).
    let auto_created = json["auto_created"].as_array().unwrap();
    assert_eq!(
        auto_created.len(),
        0,
        "failed autocreate should not appear in auto_created: {json}"
    );
}

/// Idempotency-Key with an ASCII control character must return 400.
///
/// HTTP allows horizontal tab (0x09) in header values, but our validation
/// requires all characters to be ASCII printable (not control characters).
/// Tab is `is_ascii_control()` in Rust, so "abc\txyz" triggers the 400 path
/// in our handler while still passing axum's HTTP-level header parsing.
#[tokio::test]
async fn submit_idempotency_key_non_printable_returns_400() {
    let tmp = tempfile::tempdir().unwrap();
    let base = fwd(tmp.path());
    let state = make_state(create_config(&base), tmp.path());
    let router = make_router(state);

    // Use a horizontal tab (0x09) — allowed by HTTP but rejected by our
    // is_ascii_control() check. Construct via HeaderValue::from_bytes to
    // avoid compile-time rejection.
    let key_with_tab = axum::http::HeaderValue::from_bytes(b"abc\x09xyz")
        .expect("tab is a valid HTTP header byte");
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/submit/coffee")
        .header("Authorization", "Bearer test-token")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", key_with_tab)
        .body(axum::body::Body::from(
            json!({ "field_values": { "bean": "Kenya" } }).to_string(),
        ))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Idempotency-Key containing ASCII control character (tab) must return 400"
    );
    assert_error_code(resp.into_body(), "validation_failed").await;
}
