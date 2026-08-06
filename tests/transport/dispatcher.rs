use pour::config::Config;
use pour::transport::{Transport, TransportMode};

/// Helper: minimal valid TOML config with no API key (forces FS fallback).
const FS_ONLY_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####;

/// Helper: config with API key but unreachable port (forces FS fallback).
const UNREACHABLE_API_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"
api_port = 19876
api_key = "test-key"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####;

#[tokio::test]
async fn connect_falls_back_to_fs_when_no_api_key() {
    let config = Config::from_toml(FS_ONLY_TOML).expect("should parse");
    let transport = Transport::connect(&config).await;
    assert_eq!(transport.mode(), TransportMode::FileSystem);
}

#[tokio::test]
async fn connect_falls_back_to_fs_when_api_unreachable() {
    let config = Config::from_toml(UNREACHABLE_API_TOML).expect("should parse");
    let transport = Transport::connect(&config).await;
    assert_eq!(transport.mode(), TransportMode::FileSystem);
}

#[tokio::test]
async fn create_file_delegates_to_fs_backend() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let toml_str = format!(
        r####"
[vault]
base_path = "{}"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####,
        dir.path().display().to_string().replace('\\', "/")
    );
    let config = Config::from_toml(&toml_str).expect("should parse");
    let transport = Transport::connect(&config).await;

    assert_eq!(transport.mode(), TransportMode::FileSystem);

    transport
        .create_file("hello.md", "# Hello\n")
        .await
        .expect("create_file should succeed");

    let content = std::fs::read_to_string(dir.path().join("hello.md")).unwrap();
    assert_eq!(content, "# Hello\n");
}

#[tokio::test]
async fn append_under_heading_delegates_to_fs_backend() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    std::fs::write(
        dir.path().join("note.md"),
        "# Daily Note\n\n## Log\n\nExisting entry\n",
    )
    .unwrap();

    let toml_str = format!(
        r####"
[vault]
base_path = "{}"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####,
        dir.path().display().to_string().replace('\\', "/")
    );
    let config = Config::from_toml(&toml_str).expect("should parse");
    let transport = Transport::connect(&config).await;

    transport
        .append_under_heading("note.md", "## Log", "appended text", false)
        .await
        .expect("append should succeed");

    let content = std::fs::read_to_string(dir.path().join("note.md")).unwrap();
    assert!(content.contains("appended text"));
    assert!(content.contains("Existing entry"));
}

#[tokio::test]
async fn list_directory_delegates_to_fs_backend() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let sub = dir.path().join("Beans");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("arabica.md"), "").unwrap();
    std::fs::write(sub.join("robusta.md"), "").unwrap();

    let toml_str = format!(
        r####"
[vault]
base_path = "{}"

[modules.test]
mode = "create"
path = "test.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####,
        dir.path().display().to_string().replace('\\', "/")
    );
    let config = Config::from_toml(&toml_str).expect("should parse");
    let transport = Transport::connect(&config).await;

    let files = transport
        .list_directory("Beans")
        .await
        .expect("list should succeed");

    assert_eq!(files, vec!["arabica", "robusta"]);
}

#[test]
fn transport_mode_display() {
    assert_eq!(TransportMode::Api.to_string(), "API");
    assert_eq!(TransportMode::FileSystem.to_string(), "File System");
}

// ── Per-module root override (`Transport::for_module`) ───────────────────────

/// Config with a vault plus a module `lyra` rooted somewhere else entirely.
fn config_with_module_root(module_body: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "/tmp/vault"

[modules.lyra]
mode = "create"
path = "inbox/note.md"
{module_body}

[[modules.lyra.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####
    );
    Config::from_toml(&toml).expect("test config should parse")
}

#[test]
fn for_module_is_none_when_the_module_uses_the_vault() {
    // None means "use the app transport" — every existing module is unaffected.
    let config = config_with_module_root("");
    assert!(Transport::for_module(&config.modules["lyra"]).is_none());
}

#[test]
fn for_module_is_a_filesystem_transport_rooted_at_the_override() {
    let config = config_with_module_root(r#"base_path = "/srv/inbox""#);

    let transport = Transport::for_module(&config.modules["lyra"]).expect("override → transport");

    assert_eq!(
        transport.mode(),
        TransportMode::FileSystem,
        "a root override is always filesystem — the Obsidian API cannot reach outside its vault"
    );
    match transport {
        Transport::Fs(writer) => {
            assert_eq!(writer.base_path().to_str(), Some("/srv/inbox"));
        }
        Transport::Api(_) => panic!("must never select the API transport for a root override"),
    }
}

#[test]
fn for_module_prefers_the_per_os_override() {
    let body = format!(
        "base_path = \"/srv/inbox\"\n\n[modules.lyra.platform]\n{} = \"/srv/os-specific\"",
        std::env::consts::OS
    );
    let config = config_with_module_root(&body);

    let transport = Transport::for_module(&config.modules["lyra"]).expect("override → transport");
    match transport {
        Transport::Fs(writer) => assert_eq!(writer.base_path().to_str(), Some("/srv/os-specific")),
        Transport::Api(_) => panic!("must be filesystem"),
    }
}
