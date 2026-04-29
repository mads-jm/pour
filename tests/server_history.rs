// Integration tests for GET /api/v1/history (§6.5).
//
// Tests use POUR_HOME env var to isolate history I/O. All tests that touch
// POUR_HOME are serialized via ENV_LOCK so they don't race each other.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use tokio::sync::Mutex;
use tower::ServiceExt as _;

use pour::server::{AppState, handlers, not_found_handler};
use pour::transport::TransportMode;

// ---------------------------------------------------------------------------
// Env serialization helpers
// ---------------------------------------------------------------------------

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct EnvGuard {
    key: &'static str,
    prior: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prior = std::env::var(key).ok();
        unsafe { std::env::set_var(key, value) };
        EnvGuard { key, prior }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.prior {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

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

fn make_state(config: pour::config::Config) -> AppState {
    use pour::data::presets::Presets;
    use pour::server::idempotency::IdempotencyCache;
    use pour::transport::{Transport, fs::FsWriter};
    AppState {
        transport_mode: TransportMode::FileSystem,
        token: "test-token".to_string(),
        config: Arc::new(config),
        transport: Arc::new(Transport::Fs(FsWriter::new(std::path::PathBuf::from(
            "/tmp",
        )))),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(tokio::sync::Mutex::new(Presets::empty())),
    }
}

fn make_router(state: AppState) -> axum::Router {
    use axum::{Router, middleware, routing::get};
    use pour::server::{auth, method_not_allowed_handler, no_store_middleware};

    let api = Router::new()
        .route("/api/v1/history", get(handlers::history::handler))
        .method_not_allowed_fallback(method_not_allowed_handler)
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(no_store_middleware));

    Router::new()
        .merge(api)
        .fallback(not_found_handler)
        .with_state(state)
}

fn bearer(uri: &str) -> Request<axum::body::Body> {
    Request::builder()
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
    assert!(
        json["error"].is_object(),
        "expected error envelope, got: {json}"
    );
    assert_eq!(
        json["error"]["code"], expected_code,
        "error code mismatch, got: {json}"
    );
}

/// Seed a POUR_HOME cache dir with a history JSONL file containing `entries`
/// serialised from the provided list.
fn seed_history(home: &std::path::Path, entries: &[pour::data::history::HistoryEntry]) {
    use std::io::Write as IoWrite;
    let cache_dir = home.join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();
    let path = cache_dir.join("history.jsonl");
    let mut file = std::fs::File::create(&path).unwrap();
    for entry in entries {
        let line = serde_json::to_string(entry).unwrap();
        writeln!(file, "{}", line).unwrap();
    }
    file.flush().unwrap();
}

fn make_entry(
    id: &str,
    module_key: &str,
    timestamp: chrono::DateTime<chrono::Utc>,
    first_field: Option<&str>,
) -> pour::data::history::HistoryEntry {
    pour::data::history::HistoryEntry {
        id: Some(id.to_string()),
        module_key: module_key.to_string(),
        timestamp,
        vault_path: format!("test/{id}.md"),
        first_field: first_field.map(str::to_string),
    }
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_auth_required() {
    let state = make_state(minimal_config());
    let router = make_router(state);

    let req = Request::builder()
        .uri("/api/v1/history")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_error_code(resp.into_body(), "unauthorized").await;
}

// ---------------------------------------------------------------------------
// Empty history — dashboard call (no since/until)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_empty_returns_200_with_summary() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router.oneshot(bearer("/api/v1/history")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    assert!(
        json["entries"].as_array().unwrap().is_empty(),
        "entries must be empty"
    );
    assert_eq!(json["has_more"], false);
    assert!(json["next_cursor"].is_null(), "next_cursor must be null");
    // Summary present on dashboard call.
    let s = &json["summary"];
    assert!(s.is_object(), "summary must be present on dashboard call");
    assert_eq!(s["today_count"], 0);
    assert_eq!(s["week_count"], 0);
    assert_eq!(s["streak_days"], 0);
    assert!(s["last_pour"].is_null());
    assert!(s["per_module_today"].is_object());
    assert_eq!(s["version"], 1);
}

// ---------------------------------------------------------------------------
// Cache-Control: no-store
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_has_cache_control_no_store() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router.oneshot(bearer("/api/v1/history")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(cc, "no-store", "Cache-Control must be no-store");
}

// ---------------------------------------------------------------------------
// Limit validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_limit_zero_is_400() {
    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer("/api/v1/history?limit=0"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_error_code(resp.into_body(), "validation_failed").await;
}

#[tokio::test]
async fn history_limit_1001_is_400() {
    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer("/api/v1/history?limit=1001"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_error_code(resp.into_body(), "validation_failed").await;
}

#[tokio::test]
async fn history_limit_non_integer_is_400() {
    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer("/api/v1/history?limit=abc"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_error_code(resp.into_body(), "validation_failed").await;
}

// ---------------------------------------------------------------------------
// Invalid since/until
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_invalid_since_is_400() {
    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer("/api/v1/history?since=not-a-date"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    assert_eq!(json["error"]["details"]["field"], "since");
}

#[tokio::test]
async fn history_invalid_until_is_400() {
    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer("/api/v1/history?until=2026-99-99"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    assert_eq!(json["error"]["details"]["field"], "until");
}

// ---------------------------------------------------------------------------
// Unknown module
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_unknown_module_is_400() {
    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer("/api/v1/history?module=nonexistent"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["error"]["code"], "validation_failed");
    assert_eq!(json["error"]["details"]["code"], "unknown_module");
}

// ---------------------------------------------------------------------------
// Summary absent when since/until provided
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_summary_absent_when_since_provided() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer("/api/v1/history?since=2026-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    // summary must not appear.
    assert!(
        json.get("summary").is_none() || json["summary"].is_null(),
        "summary must be absent when since is provided; got: {json}"
    );
}

#[tokio::test]
async fn history_summary_absent_when_until_provided() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    let state = make_state(minimal_config());
    let router = make_router(state);
    let resp = router
        .oneshot(bearer("/api/v1/history?until=2026-12-31T23:59:59Z"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    assert!(
        json.get("summary").is_none() || json["summary"].is_null(),
        "summary must be absent when until is provided; got: {json}"
    );
}

// ---------------------------------------------------------------------------
// Filtering and pagination
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_since_until_filter_range() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    use chrono::{TimeZone, Utc};

    let t1 = Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap();
    let t2 = Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap();
    let t3 = Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap();
    let t4 = Utc.with_ymd_and_hms(2026, 4, 23, 10, 0, 0).unwrap();

    seed_history(
        tmp.path(),
        &[
            make_entry("e1", "coffee", t1, Some("A")),
            make_entry("e2", "coffee", t2, Some("B")),
            make_entry("e3", "coffee", t3, Some("C")),
            make_entry("e4", "coffee", t4, Some("D")),
        ],
    );

    let state = make_state(minimal_config());
    let router = make_router(state);

    // since=t2, until=t4 → should return t2, t3 (t4 is exclusive)
    let uri = format!(
        "/api/v1/history?since={}&until={}",
        "2026-04-21T10:00:00Z", "2026-04-23T10:00:00Z"
    );
    let resp = router.oneshot(bearer(&uri)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    let entries = json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "expected 2 entries in range; got {json}");
    // Descending order — most recent first.
    let ids: Vec<&str> = entries.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids[0], "e3");
    assert_eq!(ids[1], "e2");
}

#[tokio::test]
async fn history_module_filter() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    use chrono::{TimeZone, Utc};
    let t1 = Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap();
    let t2 = Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap();
    let t3 = Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap();

    seed_history(
        tmp.path(),
        &[
            make_entry("e1", "coffee", t1, None),
            make_entry("e2", "me", t2, None),
            make_entry("e3", "coffee", t3, None),
        ],
    );

    let state = make_state(minimal_config());
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("/api/v1/history?module=coffee"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    let entries = json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "expected 2 coffee entries; got {json}");
    for e in entries {
        assert_eq!(e["module_key"], "coffee");
    }
}

#[tokio::test]
async fn history_limit_enforces_upper_bound() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    use chrono::{TimeZone, Utc};

    // Seed 5 entries.
    let entries: Vec<_> = (0..5)
        .map(|i| {
            make_entry(
                &format!("e{i}"),
                "coffee",
                Utc.with_ymd_and_hms(2026, 4, i + 1, 10, 0, 0).unwrap(),
                None,
            )
        })
        .collect();
    seed_history(tmp.path(), &entries);

    let state = make_state(minimal_config());
    let router = make_router(state);

    let resp = router
        .oneshot(bearer("/api/v1/history?limit=3"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_json(resp.into_body()).await;
    let arr = json["entries"].as_array().unwrap();
    assert_eq!(arr.len(), 3, "expected at most 3 entries; got {json}");
    assert_eq!(json["has_more"], true, "has_more must be true; got {json}");
    assert!(
        !json["next_cursor"].is_null(),
        "next_cursor must be set when has_more; got {json}"
    );
}

#[tokio::test]
async fn history_pagination_cursor_works() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    use chrono::{TimeZone, Utc};

    // 4 entries: e0 oldest … e3 newest.
    let entries: Vec<_> = (0..4)
        .map(|i| {
            make_entry(
                &format!("e{i}"),
                "coffee",
                Utc.with_ymd_and_hms(2026, 4, i + 1, 10, 0, 0).unwrap(),
                None,
            )
        })
        .collect();
    seed_history(tmp.path(), &entries);

    let state = make_state(minimal_config());
    let router = make_router(state);

    // Page 1: limit=2, expect e3, e2, has_more=true.
    let resp = router
        .clone()
        .oneshot(bearer("/api/v1/history?limit=2"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json1 = body_json(resp.into_body()).await;
    assert_eq!(json1["has_more"], true);
    let next_cursor = json1["next_cursor"].as_str().unwrap().to_string();
    let ids1: Vec<&str> = json1["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids1, vec!["e3", "e2"]);

    // Page 2: pass opaque cursor.
    let uri2 = format!("/api/v1/history?limit=2&cursor={}", next_cursor);
    let resp2 = router.oneshot(bearer(&uri2)).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let json2 = body_json(resp2.into_body()).await;
    let ids2: Vec<&str> = json2["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    // e2 is excluded by the cursor (id < next_cursor, where next_cursor == e2's id)
    assert!(!ids2.contains(&"e3"), "e3 must not appear on page 2");
    assert!(
        !ids2.contains(&"e2"),
        "e2 must not appear on page 2 (cursor)"
    );
    // e1 and e0 should appear.
    assert!(ids2.contains(&"e1"), "e1 must appear on page 2");
    assert!(ids2.contains(&"e0"), "e0 must appear on page 2");
}

// ---------------------------------------------------------------------------
// REGRESSION: same-millisecond entries must survive cursor pagination
// ---------------------------------------------------------------------------
//
// Scenario: 3 entries A, B, C share the same millisecond timestamp T.
// Paginating with limit=2 must return ALL THREE across both pages.
// A timestamp-only cursor (the old `next_until` scheme) would drop C because
// the next-page query `until=T` filters `timestamp < T`, excluding all @T entries.
// The id-based cursor (`next_cursor`) is immune because ids carry a monotonic
// counter suffix: A < B < C lexicographically.

#[tokio::test]
async fn history_same_ms_pagination_returns_all_entries() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let _env = EnvGuard::set("POUR_HOME", tmp.path().to_str().unwrap());

    // Craft three entries that share the exact same timestamp but have
    // lexicographically ordered ids (simulating the ms+counter format).
    use chrono::{TimeZone, Utc};
    let t = Utc.with_ymd_and_hms(2026, 4, 25, 12, 0, 0).unwrap();

    // ids must be in the format used by History::record so lexicographic sort
    // matches chronological+counter order.
    let entries = vec![
        pour::data::history::HistoryEntry {
            id: Some("20260425T120000000-0-coffee".to_string()),
            module_key: "coffee".to_string(),
            timestamp: t,
            vault_path: "test/a.md".to_string(),
            first_field: Some("A".to_string()),
        },
        pour::data::history::HistoryEntry {
            id: Some("20260425T120000000-1-coffee".to_string()),
            module_key: "coffee".to_string(),
            timestamp: t,
            vault_path: "test/b.md".to_string(),
            first_field: Some("B".to_string()),
        },
        pour::data::history::HistoryEntry {
            id: Some("20260425T120000000-2-coffee".to_string()),
            module_key: "coffee".to_string(),
            timestamp: t,
            vault_path: "test/c.md".to_string(),
            first_field: Some("C".to_string()),
        },
    ];
    seed_history(tmp.path(), &entries);

    let state = make_state(minimal_config());
    let router = make_router(state);

    // Page 1: limit=2
    let resp1 = router
        .clone()
        .oneshot(bearer("/api/v1/history?limit=2"))
        .await
        .unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);
    let json1 = body_json(resp1.into_body()).await;
    assert_eq!(
        json1["has_more"], true,
        "has_more must be true; got {json1}"
    );
    let cursor = json1["next_cursor"]
        .as_str()
        .expect("next_cursor must be present when has_more=true")
        .to_string();
    let page1_ids: Vec<&str> = json1["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    // Page 1 should have 2 of the 3 entries (most recent by id first).
    assert_eq!(
        page1_ids.len(),
        2,
        "page 1 should have 2 entries; got {json1}"
    );

    // Page 2: pass the cursor
    let uri2 = format!("/api/v1/history?limit=2&cursor={}", cursor);
    let resp2 = router.oneshot(bearer(&uri2)).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let json2 = body_json(resp2.into_body()).await;
    let page2_ids: Vec<&str> = json2["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        page2_ids.len(),
        1,
        "page 2 should have 1 remaining entry; got {json2}"
    );

    // All three ids must appear exactly once across both pages.
    let all_ids: Vec<&str> = page1_ids.iter().chain(page2_ids.iter()).copied().collect();
    assert!(
        all_ids.contains(&"20260425T120000000-0-coffee"),
        "entry A must appear; got page1={page1_ids:?}, page2={page2_ids:?}"
    );
    assert!(
        all_ids.contains(&"20260425T120000000-1-coffee"),
        "entry B must appear; got page1={page1_ids:?}, page2={page2_ids:?}"
    );
    assert!(
        all_ids.contains(&"20260425T120000000-2-coffee"),
        "entry C must appear; got page1={page1_ids:?}, page2={page2_ids:?}"
    );
}
