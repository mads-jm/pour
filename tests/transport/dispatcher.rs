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
        .append_under_heading("note.md", "## Log", "appended text")
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
