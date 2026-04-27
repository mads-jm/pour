// Integration tests for GET /api/v1/config.
//
// Uses axum's in-process test approach: build the Router, drive it with
// `tower::ServiceExt::oneshot` — no TCP socket needed.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use tower::ServiceExt as _;

use pour::server::{AppState, handlers, not_found_handler};
use pour::transport::TransportMode;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_config(toml: &str) -> pour::config::Config {
    pour::config::Config::from_toml(toml).expect("test config should parse")
}

fn make_state(config: pour::config::Config) -> AppState {
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

/// Mirrors the production router topology: route_layer (auth on known routes only),
/// method_not_allowed_fallback, no_store_middleware (outermost), and the global
/// not_found_handler fallback outside auth.
fn make_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get};
    use pour::server::{auth, method_not_allowed_handler, no_store_middleware};

    let api = Router::new()
        .route("/api/v1/config", get(handlers::config::handler))
        .route("/api/v1/health", get(handlers::health::handler))
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    Router::new()
        .merge(api)
        .fallback(not_found_handler)
        .with_state(state)
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

fn config_request(token: Option<&str>) -> Request<axum::body::Body> {
    let mut builder = Request::builder().uri("/api/v1/config");
    if let Some(t) = token {
        builder = builder.header("Authorization", format!("Bearer {t}"));
    }
    builder.body(axum::body::Body::empty()).unwrap()
}

async fn get_json(router: axum::Router, token: &str) -> serde_json::Value {
    let resp = router.oneshot(config_request(Some(token))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 65536).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_requires_auth_with_envelope() {
    let config = make_config(MINIMAL_TOML);
    let router = make_router(make_state(config));

    let resp = router.oneshot(config_request(None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_envelope(resp.into_body(), "unauthorized").await;
}

#[tokio::test]
async fn config_rejects_wrong_token_with_envelope() {
    let config = make_config(MINIMAL_TOML);
    let router = make_router(make_state(config));

    let resp = router
        .oneshot(config_request(Some("wrong")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_envelope(resp.into_body(), "unauthorized").await;
}

// ---------------------------------------------------------------------------
// Basic shape
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_returns_200_and_no_store_header() {
    let config = make_config(MINIMAL_TOML);
    let router = make_router(make_state(config));

    let resp = router
        .oneshot(config_request(Some("test-token")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store", "Cache-Control header must be no-store");
}

#[tokio::test]
async fn config_empty_modules_returns_empty_array() {
    // The config struct requires the modules field (HashMap). An empty [modules]
    // section is valid TOML and deserializes to an empty HashMap.
    let toml_with_empty = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"
[modules]
"#;
    // toml::from_str will deserialize an empty [modules] section as an empty HashMap.
    let config = make_config(toml_with_empty);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    assert!(json["modules"].is_array());
    assert_eq!(json["modules"].as_array().unwrap().len(), 0);
}

// ---------------------------------------------------------------------------
// Single module — full field shape
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_single_module_full_field_shape() {
    let config = make_config(FULL_FIELD_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;

    let modules = json["modules"].as_array().unwrap();
    assert_eq!(modules.len(), 1, "expected exactly one module");

    let m = &modules[0];
    assert_eq!(m["key"], "coffee");
    assert_eq!(m["mode"], "create");

    let fields = m["fields"].as_array().unwrap();
    assert!(!fields.is_empty());

    // Verify the dynamic_select field has all contract-required keys present
    let bean = fields.iter().find(|f| f["name"] == "bean").unwrap();
    assert_eq!(bean["field_type"], "dynamic_select");
    assert_eq!(bean["prompt"], "Bean");
    assert_eq!(bean["required"], true);
    // Optional fields present as null, not absent
    assert!(bean.get("default").is_some(), "default key must be present");
    assert_eq!(bean["default"], serde_json::Value::Null);
    assert!(bean.get("options").is_some(), "options key must be present");
    assert_eq!(bean["options"], serde_json::Value::Null);
    assert_eq!(bean["source"], "Coffee/Beans");
    assert!(bean.get("target").is_some(), "target key must be present");
    assert_eq!(bean["target"], serde_json::Value::Null);
    assert!(bean.get("callout").is_some(), "callout key must be present");
    assert_eq!(bean["callout"], serde_json::Value::Null);
    assert!(
        bean.get("callout_title").is_some(),
        "callout_title key must be present"
    );
    assert_eq!(bean["callout_title"], serde_json::Value::Null);
    assert_eq!(bean["allow_create"], true);
    assert_eq!(bean["wikilink"], true);
    assert_eq!(bean["create_template"], "bean");
    assert_eq!(bean["post_create_command"], "templater:run");
    assert!(
        bean.get("show_when").is_some(),
        "show_when key must be present"
    );
    assert_eq!(bean["show_when"], serde_json::Value::Null);
    assert!(bean.get("icon").is_some(), "icon key must be present");
    assert_eq!(bean["icon"], "🫘");
    assert_eq!(bean["preset_exclude"], false);
    assert_eq!(bean["list"], false);
    assert!(
        bean.get("sub_fields").is_some(),
        "sub_fields key must be present"
    );
    assert_eq!(bean["sub_fields"], serde_json::Value::Null);
}

// ---------------------------------------------------------------------------
// mobile_visible filtering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_module_with_mobile_visible_false_is_omitted() {
    let config = make_config(MOBILE_HIDDEN_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    let modules = json["modules"].as_array().unwrap();

    // "secret" module has mobile_visible = false — must not appear
    let secret_present = modules.iter().any(|m| m["key"] == "secret");
    assert!(!secret_present, "mobile_visible=false module must be omitted");

    // "visible" module must appear
    let visible_present = modules.iter().any(|m| m["key"] == "visible");
    assert!(visible_present, "mobile_visible=true module must be included");
}

#[tokio::test]
async fn config_module_with_mobile_visible_true_is_included() {
    let config = make_config(MOBILE_HIDDEN_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    let modules = json["modules"].as_array().unwrap();

    assert!(
        modules.iter().any(|m| m["key"] == "visible"),
        "module with mobile_visible=true must be included"
    );
}

#[tokio::test]
async fn config_module_with_mobile_visible_unset_is_included() {
    let config = make_config(MOBILE_HIDDEN_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    let modules = json["modules"].as_array().unwrap();

    assert!(
        modules.iter().any(|m| m["key"] == "default_vis"),
        "module with mobile_visible unset must be included (defaults to true)"
    );
}

// ---------------------------------------------------------------------------
// Module ordering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_module_order_is_honored() {
    let config = make_config(ORDER_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    let modules = json["modules"].as_array().unwrap();
    let keys: Vec<&str> = modules
        .iter()
        .map(|m| m["key"].as_str().unwrap())
        .collect();

    // module_order = ["bravo", "alpha"] → bravo first, alpha second, charlie last (alphabetical unlisted)
    assert_eq!(keys[0], "bravo");
    assert_eq!(keys[1], "alpha");
    assert_eq!(keys[2], "charlie");
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_templates_keyed_by_name() {
    let config = make_config(FULL_FIELD_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;

    assert!(json["templates"].is_object());
    let tmpl = &json["templates"]["bean"];
    assert!(tmpl.is_object(), "bean template must be present");
    assert!(tmpl["path"].is_string());
    assert!(tmpl["fields"].is_array());
    let tf = &tmpl["fields"][0];
    assert!(tf["name"].is_string());
    assert_eq!(tf["field_type"], "text");
    assert!(tf.get("options").is_some(), "options key must be present");
    assert!(tf.get("default").is_some(), "default key must be present");
    assert!(
        tf.get("allow_create").is_some(),
        "allow_create key must be present"
    );
}

// ---------------------------------------------------------------------------
// field_type strings
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_field_type_strings_are_snake_case() {
    let config = make_config(ALL_FIELD_TYPES_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    let fields = json["modules"][0]["fields"].as_array().unwrap();

    let expected_types = [
        "text",
        "textarea",
        "number",
        "static_select",
        "dynamic_select",
        "composite_array",
    ];
    for (i, expected) in expected_types.iter().enumerate() {
        assert_eq!(
            fields[i]["field_type"],
            *expected,
            "field[{i}] field_type mismatch"
        );
    }
}

// ---------------------------------------------------------------------------
// show_when serialization
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_show_when_equals_serializes_correctly() {
    let config = make_config(SHOW_WHEN_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    let fields = json["modules"][0]["fields"].as_array().unwrap();

    let conditional = fields.iter().find(|f| f["name"] == "conditional_field").unwrap();
    let sw = &conditional["show_when"];
    assert!(sw.is_object());
    assert_eq!(sw["field"], "controller");
    assert_eq!(sw["equals"], "yes");
    assert!(sw.get("one_of").is_some());
    assert_eq!(sw["one_of"], serde_json::Value::Null);
}

#[tokio::test]
async fn config_show_when_one_of_serializes_correctly() {
    let config = make_config(SHOW_WHEN_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    let fields = json["modules"][0]["fields"].as_array().unwrap();

    let one_of_field = fields.iter().find(|f| f["name"] == "one_of_field").unwrap();
    let sw = &one_of_field["show_when"];
    assert!(sw.is_object());
    assert_eq!(sw["field"], "controller");
    assert!(sw.get("equals").is_some());
    assert_eq!(sw["equals"], serde_json::Value::Null);
    let one_of = sw["one_of"].as_array().unwrap();
    assert_eq!(one_of.len(), 2);
    assert!(one_of.iter().any(|v| v == "a"));
    assert!(one_of.iter().any(|v| v == "b"));
}

// ---------------------------------------------------------------------------
// Vault sub-object
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_vault_defaults_date_format() {
    // Config without explicit date_format → defaults to "%Y%m%d"
    let config = make_config(MINIMAL_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    assert_eq!(json["vault"]["date_format"], "%Y%m%d");
    assert!(json["vault"]["transport_mode"].is_string());
}

#[tokio::test]
async fn config_vault_transport_mode_matches_state() {
    let config = make_config(MINIMAL_TOML);
    let mut state = make_state(config);
    state.transport_mode = TransportMode::Api;
    let router = make_router(state);

    let json = get_json(router, "test-token").await;
    assert_eq!(json["vault"]["transport_mode"], "API");
}

// ---------------------------------------------------------------------------
// config_version echoed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_version_is_echoed() {
    let config = make_config(MINIMAL_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    assert_eq!(json["config_version"], "0.3.0");
}

// ---------------------------------------------------------------------------
// module_order field echoed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_module_order_field_is_present() {
    let config = make_config(ORDER_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;
    assert!(json["module_order"].is_array());
    let order: Vec<&str> = json["module_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(order, vec!["bravo", "alpha"]);
}

/// CRITICAL #3: module_order echoes only keys that survive the mobile_visible filter.
/// module_order = ["a", "b", "c"] with b.mobile_visible = false
/// → response module_order = ["a", "c"], modules has only a and c.
#[tokio::test]
async fn config_module_order_filtered_when_module_hidden() {
    let config = make_config(ORDER_WITH_HIDDEN_TOML);
    let router = make_router(make_state(config));

    let json = get_json(router, "test-token").await;

    // module_order must not include the hidden key "b"
    let order: Vec<&str> = json["module_order"]
        .as_array()
        .expect("module_order must be an array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        vec!["a", "c"],
        "module_order must omit mobile_visible=false keys"
    );

    // modules array must also omit "b"
    let modules = json["modules"].as_array().unwrap();
    let keys: Vec<&str> = modules.iter().map(|m| m["key"].as_str().unwrap()).collect();
    assert!(!keys.contains(&"b"), "modules must not include mobile_visible=false module");
    assert!(keys.contains(&"a"), "module 'a' must be present");
    assert!(keys.contains(&"c"), "module 'c' must be present");
    assert_eq!(modules.len(), 2, "exactly 2 visible modules expected");
}

// ---------------------------------------------------------------------------
// TOML fixtures
// ---------------------------------------------------------------------------

const MINIMAL_TOML: &str = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.test]
mode = "create"
path = "Test/%Y%m%d.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"#;

const FULL_FIELD_TOML: &str = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.coffee]
mode = "create"
path = "Coffee/%Y%m%d.md"
display_name = "Coffee"
icon = "☕"
daily_link = true

[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
required = true
source = "Coffee/Beans"
icon = "🫘"
wikilink = true
allow_create = true
create_template = "bean"
post_create_command = "templater:run"

[templates.bean]
path = "Coffee/Beans/{{name}}.md"

[[templates.bean.fields]]
name = "roaster"
field_type = "text"
prompt = "Roaster"
"#;

const MOBILE_HIDDEN_TOML: &str = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.secret]
mode = "create"
path = "Secret/%Y%m%d.md"
mobile_visible = false

[[modules.secret.fields]]
name = "note"
field_type = "text"
prompt = "Note"

[modules.visible]
mode = "create"
path = "Visible/%Y%m%d.md"
mobile_visible = true

[[modules.visible.fields]]
name = "note"
field_type = "text"
prompt = "Note"

[modules.default_vis]
mode = "create"
path = "Default/%Y%m%d.md"

[[modules.default_vis.fields]]
name = "note"
field_type = "text"
prompt = "Note"
"#;

const ORDER_TOML: &str = r#"
config_version = "0.3.0"
module_order = ["bravo", "alpha"]

[vault]
base_path = "/vault"

[modules.alpha]
mode = "create"
path = "Alpha/%Y%m%d.md"

[[modules.alpha.fields]]
name = "x"
field_type = "text"
prompt = "X"

[modules.bravo]
mode = "create"
path = "Bravo/%Y%m%d.md"

[[modules.bravo.fields]]
name = "x"
field_type = "text"
prompt = "X"

[modules.charlie]
mode = "create"
path = "Charlie/%Y%m%d.md"

[[modules.charlie.fields]]
name = "x"
field_type = "text"
prompt = "X"
"#;

const ALL_FIELD_TYPES_TOML: &str = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.multi]
mode = "create"
path = "Multi/%Y%m%d.md"

[[modules.multi.fields]]
name = "f_text"
field_type = "text"
prompt = "Text"

[[modules.multi.fields]]
name = "f_textarea"
field_type = "textarea"
prompt = "Textarea"

[[modules.multi.fields]]
name = "f_number"
field_type = "number"
prompt = "Number"

[[modules.multi.fields]]
name = "f_static"
field_type = "static_select"
prompt = "Static"
options = ["a", "b"]

[[modules.multi.fields]]
name = "f_dynamic"
field_type = "dynamic_select"
prompt = "Dynamic"
source = "Multi/Source"

[[modules.multi.fields]]
name = "f_composite"
field_type = "composite_array"
prompt = "Composite"

[[modules.multi.fields.sub_fields]]
name = "col"
field_type = "text"
prompt = "Col"
"#;

const SHOW_WHEN_TOML: &str = r#"
config_version = "0.3.0"
[vault]
base_path = "/vault"

[modules.sw]
mode = "create"
path = "SW/%Y%m%d.md"

[[modules.sw.fields]]
name = "controller"
field_type = "static_select"
prompt = "Controller"
options = ["yes", "no", "a", "b"]

[[modules.sw.fields]]
name = "conditional_field"
field_type = "text"
prompt = "Conditional"
show_when = { field = "controller", equals = "yes" }

[[modules.sw.fields]]
name = "one_of_field"
field_type = "text"
prompt = "One Of"
show_when = { field = "controller", one_of = ["a", "b"] }
"#;

/// module_order = ["a", "b", "c"] with b.mobile_visible = false
/// → module_order echo must be ["a", "c"]
const ORDER_WITH_HIDDEN_TOML: &str = r#"
config_version = "0.3.0"
module_order = ["a", "b", "c"]

[vault]
base_path = "/vault"

[modules.a]
mode = "create"
path = "A/%Y%m%d.md"

[[modules.a.fields]]
name = "x"
field_type = "text"
prompt = "X"

[modules.b]
mode = "create"
path = "B/%Y%m%d.md"
mobile_visible = false

[[modules.b.fields]]
name = "x"
field_type = "text"
prompt = "X"

[modules.c]
mode = "create"
path = "C/%Y%m%d.md"

[[modules.c.fields]]
name = "x"
field_type = "text"
prompt = "X"
"#;
