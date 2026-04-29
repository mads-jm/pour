// Step F — Full-stack integration tests for `pour serve`.
//
// Unlike Steps A–E (which used `tower::ServiceExt::oneshot` / in-process calls),
// these tests spin a REAL `tokio::net::TcpListener` bound to port 0, serve the
// full axum app on a background task, and drive it with `reqwest`.
//
// This proves:
// - TCP bind/listen/accept lifecycle works
// - Hyper's HTTP/1.1 encode/decode is exercised
// - `Idempotency-Key` header propagates through the real TCP stack
// - Real request lifecycle (auth, body parsing, handler, response) works end-to-end
//
// Test isolation
// --------------
// Each scenario uses `tempfile::tempdir()` for:
//   1. The vault base path (FsWriter root) — controls where submit writes `.md` files.
//   2. POUR_HOME — controls where History::load() and Cache::load() read/write state.
//
// `POUR_HOME` tests are serialized via `ENV_LOCK` to prevent races between tests
// that must set the same env var.
//
// Scenario 3 (idempotency in-flight 409 via concurrent reqwest) is intentionally
// skipped here. The in-flight race window is microseconds — narrower than a real
// TCP round-trip + OS scheduling. The existing in-process test
// `tests/server_submit.rs::submit_idempotency_in_flight_returns_409` covers this
// case reliably using direct cache manipulation without network latency.
//
// Scenario 9 note (mobile_visible filter on submit):
// The submit handler (submit_inner) returns 404 for modules with
// `mobile_visible = false` (see src/server/handlers/submit.rs:133-139). This is
// an implementation decision: mobile_visible=false hides the module from both
// GET /config AND POST /submit. The contract §6.4 does not explicitly state this,
// but the implementation treats visibility consistently at both surfaces. This
// scenario tests the observed behaviour.

use std::net::SocketAddr;
use std::sync::Arc;

use reqwest::StatusCode;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;

use pour::config::Config;
use pour::data::presets::Presets;
use pour::server::{AppState, idempotency::IdempotencyCache, serve_on_listener};
use pour::transport::{Transport, TransportMode, fs::FsWriter};

// ---------------------------------------------------------------------------
// Env serialization — tests that touch POUR_HOME must hold this lock
// ---------------------------------------------------------------------------

static ENV_LOCK: TokioMutex<()> = TokioMutex::const_new(());

struct EnvGuard {
    key: &'static str,
    prior: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prior = std::env::var(key).ok();
        // SAFETY: required for test isolation; no threads read this env var
        // concurrently because all POUR_HOME tests serialize on ENV_LOCK.
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
// Test server helper
// ---------------------------------------------------------------------------

/// Holds the temp dirs alive for the duration of a test scenario.
struct TestServer {
    pub addr: SocketAddr,
    pub token: String,
    /// Vault temp dir (FsWriter root).
    pub _vault_dir: TempDir,
    /// POUR_HOME temp dir (history, cache).
    pub _home_dir: TempDir,
}

/// Spawn a real TCP listener on port 0, start the server, return the address
/// and cleanup handles.
///
/// `config_toml` — raw TOML string for the module config. The `[vault] base_path`
/// is patched to the temp vault dir at runtime.
async fn start_test_server(config_toml_template: &str) -> (TestServer, EnvGuard) {
    let vault_dir = tempfile::tempdir().expect("vault tempdir");
    let home_dir = tempfile::tempdir().expect("home tempdir");

    // Forward-slash the vault path for TOML safety on Windows.
    let vault_path_str = vault_dir.path().to_str().unwrap().replace('\\', "/");

    // Substitute {{VAULT}} placeholder in the template.
    let config_toml = config_toml_template.replace("{{VAULT}}", &vault_path_str);

    let config = Config::from_toml(&config_toml).expect("test config must parse");

    let transport = Transport::Fs(FsWriter::new(vault_dir.path().to_path_buf()));
    let transport_mode = TransportMode::FileSystem;

    let token = "integration-test-token".to_string();

    // Use a real presets path inside home_dir so api_set/api_remove/api_reorder
    // can persist. Presets::empty() has an empty path and save() fails.
    let presets_path = home_dir.path().join("presets.json");
    let presets = Presets::load_from(presets_path);

    let state = AppState {
        transport_mode,
        token: token.clone(),
        config: Arc::new(config),
        transport: Arc::new(transport),
        idempotency: Arc::new(IdempotencyCache::new()),
        presets: Arc::new(TokioMutex::new(presets)),
    };

    // Bind to port 0 — OS assigns an ephemeral port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind port 0");
    let addr = listener.local_addr().expect("local_addr");

    // Spawn server in background.
    tokio::spawn(async move {
        serve_on_listener(listener, state)
            .await
            .expect("server error");
    });

    // Set POUR_HOME to the isolated temp dir BEFORE the server receives any
    // requests. History::load() and Cache::load() read this env var at call
    // time, not at server start, so the guard must be held for the full test.
    //
    // Note: the EnvGuard is returned to the caller who must keep it alive.
    let env_guard = EnvGuard::set("POUR_HOME", home_dir.path().to_str().unwrap());

    let server = TestServer {
        addr,
        token: token.clone(),
        _vault_dir: vault_dir,
        _home_dir: home_dir,
    };

    (server, env_guard)
}

fn client(token: &str, _addr: SocketAddr) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {token}").parse().unwrap(),
            );
            h
        })
        .build()
        .unwrap()
}

fn base(addr: SocketAddr) -> String {
    format!("http://{}", addr)
}

// ---------------------------------------------------------------------------
// Config TOML fixtures
// ---------------------------------------------------------------------------

const COFFEE_CONFIG: &str = r#"
config_version = "0.3.0"
module_order = ["coffee"]

[vault]
base_path = "{{VAULT}}"

[modules.coffee]
mode = "create"
path = "Coffee/{{bean}}-%H%M%S%6f.md"
display_name = "Coffee"
icon = "☕"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
required = true

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
"#;

const TWO_MODULE_CONFIG: &str = r#"
config_version = "0.3.0"
module_order = ["coffee"]

[vault]
base_path = "{{VAULT}}"

[modules.coffee]
mode = "create"
path = "Coffee/%Y/%Y%m%d-%H%M%S.md"
display_name = "Coffee"
icon = "☕"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
required = true

[modules.desktop_only]
mode = "create"
path = "Desktop/%Y%m%d.md"
mobile_visible = false

[[modules.desktop_only.fields]]
name = "note"
field_type = "text"
prompt = "Note"
"#;

const BEAN_TEMPLATE_CONFIG: &str = r#"
config_version = "0.3.0"
module_order = ["coffee"]

[vault]
base_path = "{{VAULT}}"

[modules.coffee]
mode = "create"
path = "Coffee/%Y/%Y%m%d-%H%M%S.md"
display_name = "Coffee"
icon = "☕"

[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
required = true
source = "Coffee/Beans"
allow_create = true
create_template = "bean"

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"

[templates.bean]
path = "Coffee/Beans/{{name}}.md"

[[templates.bean.fields]]
name = "roaster"
field_type = "text"
prompt = "Roaster"

[[templates.bean.fields]]
name = "origin"
field_type = "text"
prompt = "Origin"
"#;

// ---------------------------------------------------------------------------
// Scenario 1: Capture round-trip (keystone)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scenario1_capture_round_trip() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(COFFEE_CONFIG).await;
    let c = client(&srv.token, srv.addr);
    let base = base(srv.addr);

    // Step 1: health
    let resp = c.get(format!("{base}/api/v1/health")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["ok"], true);
    assert_eq!(body["transport_mode"], "FileSystem");

    // Step 2: config — modules array contains coffee
    let resp = c.get(format!("{base}/api/v1/config")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cfg: Value = resp.json().await.unwrap();
    let modules = cfg["modules"].as_array().unwrap();
    assert!(
        modules.iter().any(|m| m["key"] == "coffee"),
        "config must contain 'coffee' module"
    );

    // Step 3: submit
    let submit_body = json!({
        "field_values": { "bean": "Ethiopia Guji", "notes": "Bright and fruity." }
    });
    let resp = c
        .post(format!("{base}/api/v1/submit/coffee"))
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", "s1-round-trip-001")
        .json(&submit_body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "submit must return 201");
    let submit_resp: Value = resp.json().await.unwrap();
    let history_id = submit_resp["history_id"].as_str().unwrap().to_string();
    let vault_path = submit_resp["vault_path"].as_str().unwrap().to_string();
    assert!(!history_id.is_empty(), "history_id must be non-empty");
    assert!(!vault_path.is_empty(), "vault_path must be non-empty");
    assert_eq!(submit_resp["transport_mode"], "FileSystem");

    // Step 4: history — first entry matches submit response
    let resp = c
        .get(format!("{base}/api/v1/history"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let hist: Value = resp.json().await.unwrap();
    let entries = hist["entries"].as_array().unwrap();
    assert!(!entries.is_empty(), "history must have at least one entry");
    let entry = entries.iter().find(|e| e["id"] == history_id);
    assert!(
        entry.is_some(),
        "history must contain entry with id={history_id}"
    );
    let entry = entry.unwrap();
    assert_eq!(entry["vault_path"], vault_path);
    assert_eq!(entry["module_key"], "coffee");

    // Step 5: captures — content includes the submitted bean value
    let resp = c
        .get(format!("{base}/api/v1/captures/{history_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cap: Value = resp.json().await.unwrap();
    let content = cap["content"].as_str().unwrap();
    assert!(
        content.contains("Ethiopia Guji"),
        "capture content must include submitted bean value; content: {content}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: Idempotency replay round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scenario2_idempotency_replay() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(COFFEE_CONFIG).await;
    let c = client(&srv.token, srv.addr);
    let base = base(srv.addr);

    let submit_body = json!({
        "field_values": { "bean": "Yirgacheffe" }
    });
    let key = "s2-idempotency-replay-key";

    // First submit → 201
    let resp1 = c
        .post(format!("{base}/api/v1/submit/coffee"))
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", key)
        .json(&submit_body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), StatusCode::CREATED);
    let b1: Value = resp1.json().await.unwrap();
    let history_id1 = b1["history_id"].as_str().unwrap().to_string();

    // Second submit with same key → replay (200 or 201, with Idempotency-Replay: true)
    let resp2 = c
        .post(format!("{base}/api/v1/submit/coffee"))
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", key)
        .json(&submit_body)
        .send()
        .await
        .unwrap();
    // §9: replayed response carries the original status (201 for the first submit).
    assert_eq!(
        resp2.status(),
        StatusCode::CREATED,
        "replay must echo original status"
    );
    let replay_header = resp2
        .headers()
        .get("idempotency-replay")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        replay_header, "true",
        "replay response must carry Idempotency-Replay: true header"
    );
    let b2: Value = resp2.json().await.unwrap();
    assert_eq!(
        b2["history_id"], history_id1,
        "replayed response body must be identical (same history_id)"
    );

    // History must have exactly ONE entry for this submission
    let resp = c
        .get(format!("{base}/api/v1/history"))
        .send()
        .await
        .unwrap();
    let hist: Value = resp.json().await.unwrap();
    let matching: Vec<_> = hist["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["id"] == history_id1)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "idempotency replay must not create a duplicate history entry"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: Idempotency in-flight 409 — SKIPPED in integration tests
// ---------------------------------------------------------------------------
//
// Genuine concurrent reqwest requests cannot reliably produce the in-flight
// race because the first request completes (and the idempotency cache entry
// transitions to Done) faster than the second request's TCP handshake + server
// scheduling. The in-process test
// `tests/server_submit.rs::submit_idempotency_in_flight_returns_409`
// covers this path with direct cache manipulation (insert_stale_in_flight)
// without TCP latency.
//
// We keep a marker here so the test count is traceable.

#[tokio::test]
async fn scenario3_in_flight_409_covered_by_unit_tests() {
    // Intentional no-op. See comment above.
}

// ---------------------------------------------------------------------------
// Scenario 4: Presets CRUD round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scenario4_presets_crud_round_trip() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(COFFEE_CONFIG).await;
    let c = client(&srv.token, srv.addr);
    let base = base(srv.addr);

    // Step 1: GET → empty list
    let resp = c
        .get(format!("{base}/api/v1/presets/coffee"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["presets"].as_array().unwrap().len(),
        0,
        "initial presets list must be empty"
    );

    // Step 2: PUT (create)
    let preset_body = json!({
        "description": "weekday espresso",
        "values": { "bean": "Ethiopia Guji" }
    });
    let resp = c
        .put(format!("{base}/api/v1/presets/coffee/Morning%20Onyx"))
        .header("Content-Type", "application/json")
        .json(&preset_body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "first PUT must return 201"
    );
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(!location.is_empty(), "201 must carry Location header");

    // Step 3: GET → contains new preset
    let resp = c
        .get(format!("{base}/api/v1/presets/coffee"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let presets = body["presets"].as_array().unwrap();
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0]["name"], "Morning Onyx");
    assert_eq!(presets[0]["values"]["bean"], "Ethiopia Guji");

    // Add a second preset
    let resp = c
        .put(format!("{base}/api/v1/presets/coffee/Aeropress%20Quick"))
        .header("Content-Type", "application/json")
        .json(&json!({ "values": { "bean": "Sumatra" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Step 4: PUT (update existing)
    let resp = c
        .put(format!("{base}/api/v1/presets/coffee/Morning%20Onyx"))
        .header("Content-Type", "application/json")
        .json(&json!({
            "description": "updated",
            "values": { "bean": "Yirgacheffe" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "second PUT must return 200");
    // 200 (update) must NOT carry Location header — check before consuming body
    let has_location = resp.headers().contains_key("location");
    let update_resp: Value = resp.json().await.unwrap();
    assert_eq!(update_resp["preset"]["values"]["bean"], "Yirgacheffe");
    assert!(!has_location, "update (200) must not carry Location header");

    // Step 5: PUT order — reorder ["Aeropress Quick", "Morning Onyx"]
    let resp = c
        .put(format!("{base}/api/v1/presets/coffee/order"))
        .header("Content-Type", "application/json")
        .json(&json!({ "names": ["Aeropress Quick", "Morning Onyx"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let order_resp: Value = resp.json().await.unwrap();
    let names: Vec<_> = order_resp["presets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Aeropress Quick", "Morning Onyx"]);

    // Step 6: DELETE
    let resp = c
        .delete(format!("{base}/api/v1/presets/coffee/Morning%20Onyx"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Step 7: GET → list without deleted preset
    let resp = c
        .get(format!("{base}/api/v1/presets/coffee"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let presets = body["presets"].as_array().unwrap();
    assert_eq!(presets.len(), 1, "only one preset remains after delete");
    assert_eq!(presets[0]["name"], "Aeropress Quick");
}

// ---------------------------------------------------------------------------
// Scenario 5: Auto-create with templated note
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scenario5_autocreate_templated_round_trip() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(BEAN_TEMPLATE_CONFIG).await;
    let c = client(&srv.token, srv.addr);
    let base = base(srv.addr);

    let submit_body = json!({
        "field_values": { "bean": "Novel Bean", "notes": "First attempt." },
        "auto_create_inputs": {
            "bean": { "roaster": "Test Roaster", "origin": "Kenya" }
        }
    });

    let resp = c
        .post(format!("{base}/api/v1/submit/coffee"))
        .header("Content-Type", "application/json")
        .json(&submit_body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let resp_body: Value = resp.json().await.unwrap();

    // auto_created array must contain the bean entry with templated = true
    let auto_created = resp_body["auto_created"].as_array().unwrap();
    assert!(!auto_created.is_empty(), "auto_created must not be empty");
    let bean_entry = auto_created
        .iter()
        .find(|a| a["field"] == "bean")
        .expect("auto_created must contain bean entry");
    assert_eq!(bean_entry["templated"], true);
    let bean_vault_path = bean_entry["vault_path"].as_str().unwrap();
    assert!(!bean_vault_path.is_empty());

    // Verify the bean's vault file exists on disk with templated content
    let full_bean_path = srv
        ._vault_dir
        .path()
        .join(bean_vault_path.replace('/', std::path::MAIN_SEPARATOR_STR));
    assert!(
        full_bean_path.exists(),
        "auto-created bean file must exist on disk at {full_bean_path:?}"
    );
    let bean_content =
        std::fs::read_to_string(&full_bean_path).expect("bean file must be readable");
    assert!(
        bean_content.contains("Test Roaster") || bean_content.contains("Novel Bean"),
        "bean file must contain template values; content: {bean_content}"
    );

    // Verify the parent submit's captured file also exists
    let history_id = resp_body["history_id"].as_str().unwrap();
    let captures_resp = c
        .get(format!("{base}/api/v1/captures/{history_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(captures_resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Scenario 6: Static + API coexistence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scenario6_static_and_api_coexist() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(COFFEE_CONFIG).await;
    let base = base(srv.addr);

    // Build an unauthenticated client for static asset checks.
    let anon = reqwest::Client::new();

    // Step 1: GET / (no auth) → 200, HTML body
    let resp = anon.get(format!("{base}/")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("text/html"),
        "/ must return text/html; got {ct}"
    );
    // Cache-Control for shell HTML must be no-cache, max-age=0, must-revalidate (§12)
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "no-cache, max-age=0, must-revalidate",
        "/ Cache-Control must be no-cache, max-age=0, must-revalidate; got {cc}"
    );

    // Step 2: GET /api/v1/health (no auth) → 401
    let resp = anon
        .get(format!("{base}/api/v1/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "unauthorized");

    // Step 3: GET /api/v1/health (with token) → 200
    let authed = client(&srv.token, srv.addr);
    let resp = authed
        .get(format!("{base}/api/v1/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // Cache-Control: no-store on API responses (§12)
    let cc = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        cc, "no-store",
        "/api/v1/health Cache-Control must be no-store; got {cc}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 7: Full PWA bootstrap simulation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scenario7_pwa_bootstrap_simulation() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(COFFEE_CONFIG).await;
    let base = base(srv.addr);
    let anon = reqwest::Client::new();

    // Step 1: Phone scans QR → GET /?token=<T> (no auth header)
    // The static index.html is served without auth; the JS extracts the token.
    let resp = anon
        .get(format!("{base}/?token={}", srv.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "GET /?token=<T> must return 200 (static asset, no auth required)"
    );

    // Steps 2-5: JS reads token, switches to header auth
    let authed = client(&srv.token, srv.addr);

    // Step 3: GET /api/v1/health with Bearer header
    let resp = authed
        .get(format!("{base}/api/v1/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Step 4: GET /api/v1/config
    let resp = authed
        .get(format!("{base}/api/v1/config"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cfg: Value = resp.json().await.unwrap();
    assert!(
        cfg["modules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["key"] == "coffee")
    );

    // Step 5: POST /api/v1/submit with Bearer header + Idempotency-Key
    let resp = authed
        .post(format!("{base}/api/v1/submit/coffee"))
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", "s7-pwa-bootstrap-001")
        .json(&json!({ "field_values": { "bean": "Sumatra Mandheling" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let submit_resp: Value = resp.json().await.unwrap();
    let history_id = submit_resp["history_id"].as_str().unwrap();

    // Step 6: GET /api/v1/history → includes new entry
    let resp = authed
        .get(format!("{base}/api/v1/history"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let hist: Value = resp.json().await.unwrap();
    let found = hist["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == history_id);
    assert!(found, "history must include the submitted entry");
}

// ---------------------------------------------------------------------------
// Scenario 8: captured_at offline-replay
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scenario8_captured_at_offline_replay() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(COFFEE_CONFIG).await;
    let c = client(&srv.token, srv.addr);
    let base = base(srv.addr);

    // Use a captured_at 7 days in the past (well within the 30-day window).
    let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
    let captured_at_str = seven_days_ago.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let submit_body = json!({
        "field_values": { "bean": "Offline Replay Bean" },
        "captured_at": captured_at_str
    });

    let resp = c
        .post(format!("{base}/api/v1/submit/coffee"))
        .header("Content-Type", "application/json")
        .json(&submit_body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let submit_resp: Value = resp.json().await.unwrap();
    let history_id = submit_resp["history_id"].as_str().unwrap().to_string();

    // Echoed captured_at in response must match what we sent (±1s tolerance for format)
    let echoed = submit_resp["captured_at"].as_str().unwrap();
    // Parse both and compare at second resolution
    let echoed_dt: chrono::DateTime<chrono::Utc> = echoed.parse().unwrap();
    let diff = (echoed_dt - seven_days_ago).num_seconds().abs();
    assert!(
        diff <= 1,
        "echoed captured_at must match submitted captured_at; diff={diff}s"
    );

    // History timestamp must match captured_at (not server now)
    let resp = c
        .get(format!("{base}/api/v1/history"))
        .send()
        .await
        .unwrap();
    let hist: Value = resp.json().await.unwrap();
    let entry = hist["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == history_id)
        .expect("entry must be in history");

    let hist_ts: chrono::DateTime<chrono::Utc> =
        entry["timestamp"].as_str().unwrap().parse().unwrap();
    let diff = (hist_ts - seven_days_ago).num_seconds().abs();
    assert!(
        diff <= 1,
        "history timestamp must equal captured_at (not server now); diff={diff}s"
    );

    // Capture file's date frontmatter must use captured_at date
    let cap_resp = c
        .get(format!("{base}/api/v1/captures/{history_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(cap_resp.status(), StatusCode::OK);
    let cap: Value = cap_resp.json().await.unwrap();
    let content = cap["content"].as_str().unwrap();
    // The server renders `date:` frontmatter in LOCAL timezone per contract §10,
    // not UTC. Compute expected the same way the server does — otherwise the
    // assertion is flaky and only passes during the half of the day when UTC
    // and Local fall on the same calendar date.
    let expected_date = seven_days_ago
        .with_timezone(&chrono::Local)
        .format("%Y-%m-%d")
        .to_string();
    assert!(
        content.contains(&expected_date),
        "capture content must contain date {expected_date} matching captured_at; content: {content}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 9: mobile_visible filter end-to-end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scenario9_mobile_visible_filter_e2e() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(TWO_MODULE_CONFIG).await;
    let c = client(&srv.token, srv.addr);
    let base = base(srv.addr);

    // GET /api/v1/config → only coffee, not desktop_only
    let resp = c.get(format!("{base}/api/v1/config")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cfg: Value = resp.json().await.unwrap();
    let modules = cfg["modules"].as_array().unwrap();
    assert!(
        modules.iter().any(|m| m["key"] == "coffee"),
        "coffee must appear in config"
    );
    assert!(
        !modules.iter().any(|m| m["key"] == "desktop_only"),
        "desktop_only (mobile_visible=false) must not appear in config"
    );
    let order = cfg["module_order"].as_array().unwrap();
    assert!(
        !order.iter().any(|k| k == "desktop_only"),
        "module_order must not include desktop_only"
    );

    // POST /api/v1/submit/desktop_only → 404 (server treats mobile_visible=false
    // as not-found at the submit surface, consistent with the config filter).
    // This is the implementation's chosen behavior (belt-and-suspenders visibility
    // enforcement) — not explicitly stated in §6.4 but consistent with the contract's
    // intent: the PWA cannot see the module, so the server is permissive to return 404.
    let resp = c
        .post(format!("{base}/api/v1/submit/desktop_only"))
        .header("Content-Type", "application/json")
        .json(&json!({ "field_values": { "note": "test" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "submit to mobile_visible=false module must return 404"
    );
}

// ---------------------------------------------------------------------------
// Scenario 10: 50-parallel submits stress test
// ---------------------------------------------------------------------------
//
// Fires 50 parallel submits (distinct Idempotency-Keys) to prove:
// - All 50 return 201
// - All 50 complete under 5 seconds (wall clock)
// - History has 50 entries with distinct IDs (atomic counter works under concurrency)

#[tokio::test]
async fn scenario10_parallel_submits_stress() {
    let _lock = ENV_LOCK.lock().await;
    let (srv, _env) = start_test_server(COFFEE_CONFIG).await;
    let c = Arc::new(client(&srv.token, srv.addr));
    let base = Arc::new(base(srv.addr));

    let n = 50usize;

    let start = std::time::Instant::now();

    let tasks: Vec<_> = (0..n)
        .map(|i| {
            let c = Arc::clone(&c);
            let base = Arc::clone(&base);
            tokio::spawn(async move {
                c.post(format!("{base}/api/v1/submit/coffee"))
                    .header("Content-Type", "application/json")
                    .header("Idempotency-Key", format!("stress-{i:04}"))
                    .json(&json!({
                        "field_values": { "bean": format!("Bean {i}") }
                    }))
                    .send()
                    .await
                    .expect("request must not fail")
            })
        })
        .collect();

    let mut history_ids = Vec::with_capacity(n);
    for task in tasks {
        let resp = task.await.expect("task must not panic");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap();
        assert_eq!(
            status,
            StatusCode::CREATED,
            "each parallel submit must return 201; body: {body}"
        );
        history_ids.push(body["history_id"].as_str().unwrap().to_string());
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "50 parallel submits must complete in under 5 seconds; took {elapsed:.2?}"
    );

    // All 50 history IDs must be distinct.
    let unique_ids: std::collections::HashSet<_> = history_ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        n,
        "all {n} parallel submits must have distinct history IDs; got {} unique",
        unique_ids.len()
    );

    // History must have at least 50 entries (may have more from prior scenarios
    // if env isolation failed, but we verify our n are present by id).
    let resp = c
        .get(format!("{base}/api/v1/history?limit=1000"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let hist: Value = resp.json().await.unwrap();
    let all_ids: std::collections::HashSet<_> = hist["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap_or(""))
        .collect();

    let missing: Vec<_> = history_ids
        .iter()
        .filter(|id| !all_ids.contains(id.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "all {n} stress-test history IDs must appear in history; missing: {missing:?}"
    );
}
