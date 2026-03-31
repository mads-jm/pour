use pour::config::Config;
use pour::output::{write_append, write_create};
use pour::transport::Transport;
use std::collections::HashMap;
use tempfile::TempDir;

/// Build a Config with a create-mode coffee module pointing at a temp vault.
fn create_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.coffee]
mode = "create"
path = "Coffee/note.md"
display_name = "Coffee"

[[modules.coffee.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
options = ["V60", "AeroPress"]
target = "frontmatter"

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating"

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Tasting notes"
"####
    );
    Config::from_toml(&toml).expect("test config should parse")
}

/// Build a Config with an append-mode journal module.
fn append_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.me]
mode = "append"
path = "Journal/daily.md"
append_under_header = "## Log"
append_template = "> [!note] {{{{time}}}}\n> {{{{body}}}}"
display_name = "Journal"

[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "What's on your mind?"
required = true
target = "body"
"####
    );
    Config::from_toml(&toml).expect("test config should parse")
}

#[tokio::test]
async fn write_create_produces_file_with_frontmatter_and_body() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = create_config(&base);
    let module = &config.modules["coffee"];

    // Create the target directory so the filesystem writer can write.
    std::fs::create_dir_all(tmp.path().join("Coffee")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("brew_method".to_string(), "V60".to_string());
    fields.insert("rating".to_string(), "4".to_string());
    fields.insert("notes".to_string(), "Fruity and bright.".to_string());

    let path = write_create(&transport, module, &fields)
        .await
        .expect("write_create should succeed");

    assert_eq!(path, "Coffee/note.md");

    // Read the file back and verify structure.
    let content = std::fs::read_to_string(tmp.path().join("Coffee/note.md")).unwrap();

    assert!(
        content.starts_with("---\n"),
        "should start with frontmatter"
    );
    assert!(
        content.contains("brew_method: V60"),
        "should have brew_method in frontmatter"
    );
    assert!(
        content.contains("rating: 4"),
        "should have rating in frontmatter"
    );
    assert!(content.contains("date:"), "should have auto-injected date");

    // Notes is textarea -> body, should NOT be in frontmatter.
    // It should appear after the closing ---.
    let parts: Vec<&str> = content.splitn(3, "---\n").collect();
    assert!(
        parts.len() >= 3,
        "should have opening ---, frontmatter, closing ---"
    );
    let body = parts[2];
    assert!(
        body.contains("Fruity and bright."),
        "body should contain notes text, got: {body}"
    );
    // Notes should NOT be in the frontmatter section.
    let frontmatter_section = parts[1];
    assert!(
        !frontmatter_section.contains("notes"),
        "notes should not appear in frontmatter section"
    );
}

#[tokio::test]
async fn write_create_rejects_append_module() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = append_config(&base);
    let module = &config.modules["me"];

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));
    let fields = HashMap::new();

    let result = write_create(&transport, module, &fields).await;
    assert!(result.is_err(), "write_create on append module should fail");
    assert!(
        result.unwrap_err().to_string().contains("non-create"),
        "error should mention non-create"
    );
}

#[tokio::test]
async fn write_append_inserts_rendered_template() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = append_config(&base);
    let module = &config.modules["me"];

    // Create the daily note file that append will write into.
    let journal_dir = tmp.path().join("Journal");
    std::fs::create_dir_all(&journal_dir).unwrap();
    std::fs::write(journal_dir.join("daily.md"), "# Daily Note\n\n## Log\n\n").unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Had a great morning.".to_string());

    let path = write_append(&transport, module, &fields)
        .await
        .expect("write_append should succeed");

    assert_eq!(path, "Journal/daily.md");

    let content = std::fs::read_to_string(journal_dir.join("daily.md")).unwrap();
    assert!(
        content.contains("Had a great morning."),
        "appended content should contain body text, got: {content}"
    );
}

#[tokio::test]
async fn write_append_rejects_create_module() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = create_config(&base);
    let module = &config.modules["coffee"];

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));
    let fields = HashMap::new();

    let result = write_append(&transport, module, &fields).await;
    assert!(result.is_err(), "write_append on create module should fail");
    assert!(
        result.unwrap_err().to_string().contains("non-append"),
        "error should mention non-append"
    );
}

#[tokio::test]
async fn write_create_skips_empty_body_section() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = create_config(&base);
    let module = &config.modules["coffee"];

    std::fs::create_dir_all(tmp.path().join("Coffee")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    // Only provide frontmatter fields, no body (notes is empty).
    let mut fields = HashMap::new();
    fields.insert("brew_method".to_string(), "AeroPress".to_string());
    fields.insert("rating".to_string(), "3".to_string());

    let _path = write_create(&transport, module, &fields)
        .await
        .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Coffee/note.md")).unwrap();

    // Content should end with closing --- and a newline, no extra body.
    assert!(
        content.ends_with("---\n"),
        "with no body, content should end with ---\\n, got: {content}"
    );
}
