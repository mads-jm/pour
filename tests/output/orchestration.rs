use pour::config::Config;
use pour::output::render_composite_table;
use pour::output::{CompositeData, apply_wikilink, write_append, write_create};
use pour::transport::Transport;
use std::collections::HashMap;
use tempfile::TempDir;

/// Build a Config with a create-mode module containing a wikilink field.
fn wikilink_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.brew]
mode = "create"
path = "Brew/note.md"
display_name = "Brew"

[[modules.brew.fields]]
name = "roaster"
field_type = "static_select"
prompt = "Roaster"
options = ["Onyx", "Stumptown"]
wikilink = true
list = true

[[modules.brew.fields]]
name = "origin"
field_type = "text"
prompt = "Origin"
wikilink = false

[[modules.brew.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
target = "body"
wikilink = true
"####
    );
    Config::from_toml(&toml).expect("test wikilink config should parse")
}

/// Build a Config with a create-mode module for testing pre-wrapped values.
fn wikilink_prewrapped_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.brew]
mode = "create"
path = "Brew/note.md"
display_name = "Brew"

[[modules.brew.fields]]
name = "roaster"
field_type = "text"
prompt = "Roaster"
wikilink = true
"####
    );
    Config::from_toml(&toml).expect("test pre-wrapped config should parse")
}

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
        r#####"
[vault]
base_path = "{base_path}"

[modules.me]
mode = "append"
path = "Journal/daily.md"
append_under_header = "## Log"
append_template = "#### {{{{time}}}}\n> [!note] {{{{title}}}}\n> {{{{body}}}}"
display_name = "Journal"

[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "What's on your mind?"
required = true
target = "body"
"#####
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

    let path = write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
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
        content.contains("rating: \"4\""),
        "should have rating in frontmatter (quoted numeric)"
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

    let result = write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await;
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

    let path = write_append(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
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

    let result = write_append(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await;
    assert!(result.is_err(), "write_append on create module should fail");
    assert!(
        result.unwrap_err().to_string().contains("non-append"),
        "error should mention non-append"
    );
}

#[tokio::test]
async fn wikilink_true_wraps_frontmatter_value() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = wikilink_config(&base);
    let module = &config.modules["brew"];

    std::fs::create_dir_all(tmp.path().join("Brew")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("roaster".to_string(), "Onyx".to_string());
    fields.insert("origin".to_string(), "Ethiopia".to_string());

    write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Brew/note.md")).unwrap();

    assert!(
        content.contains(r#"roaster: "[[Onyx]]""#),
        "wikilink=true field should be wrapped and quoted, got: {content}"
    );
    assert!(
        content.contains("origin: Ethiopia"),
        "wikilink=false field should be plain, got: {content}"
    );
}

#[tokio::test]
async fn wikilink_true_wraps_body_value() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = wikilink_config(&base);
    let module = &config.modules["brew"];

    std::fs::create_dir_all(tmp.path().join("Brew")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("roaster".to_string(), "Onyx".to_string());
    fields.insert("notes".to_string(), "Very bright.".to_string());

    write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Brew/note.md")).unwrap();

    let parts: Vec<&str> = content.splitn(3, "---\n").collect();
    let body = parts.get(2).copied().unwrap_or("");
    assert!(
        body.contains("[[Very bright.]]"),
        "wikilink=true textarea body should be wrapped, got body: {body}"
    );
}

#[tokio::test]
async fn wikilink_no_double_wrap() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = wikilink_prewrapped_config(&base);
    let module = &config.modules["brew"];

    std::fs::create_dir_all(tmp.path().join("Brew")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    // Value is already wrapped
    fields.insert("roaster".to_string(), "[[Onyx]]".to_string());

    write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Brew/note.md")).unwrap();

    // Should appear exactly once, not as [[[[Onyx]]]]
    assert!(
        content.contains(r#"roaster: "[[Onyx]]""#),
        "pre-wrapped value should not be double-wrapped, got: {content}"
    );
    assert!(
        !content.contains("[[[["),
        "should not double-wrap, got: {content}"
    );
}

#[tokio::test]
async fn wikilink_default_false_no_behavior_change() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    // The existing create_config has no wikilink field — default behavior
    let config = create_config(&base);
    let module = &config.modules["coffee"];

    std::fs::create_dir_all(tmp.path().join("Coffee")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("brew_method".to_string(), "V60".to_string());
    fields.insert("rating".to_string(), "4".to_string());

    write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Coffee/note.md")).unwrap();

    assert!(
        content.contains("brew_method: V60"),
        "without wikilink, value should be plain, got: {content}"
    );
    assert!(
        !content.contains("[["),
        "without wikilink, no brackets should appear, got: {content}"
    );
}

#[tokio::test]
async fn wikilink_wraps_each_comma_separated_item() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = wikilink_config(&base);
    let module = &config.modules["brew"];

    std::fs::create_dir_all(tmp.path().join("Brew")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("roaster".to_string(), "Onyx, Stumptown".to_string());

    write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Brew/note.md")).unwrap();

    // Each item should be individually wrapped, producing a YAML list
    assert!(
        content.contains("[[Onyx]]") && content.contains("[[Stumptown]]"),
        "comma-separated values should each be wrapped individually, got: {content}"
    );
    assert!(
        !content.contains("[[Onyx, Stumptown]]"),
        "should not wrap the entire comma-separated string as one wikilink, got: {content}"
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

    let _path = write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Coffee/note.md")).unwrap();

    // Content should end with closing --- and a newline, no extra body.
    assert!(
        content.ends_with("---\n"),
        "with no body, content should end with ---\\n, got: {content}"
    );
}

/// Config with a field-level callout on a textarea field.
fn callout_field_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.test]
mode = "create"
path = "Test/note.md"

[[modules.test.fields]]
name = "title"
field_type = "text"
prompt = "Title"
target = "frontmatter"

[[modules.test.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
callout = "tip"
"####
    );
    Config::from_toml(&toml).expect("test config should parse")
}

#[tokio::test]
async fn write_create_wraps_body_in_callout() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = callout_field_config(&base);
    let module = &config.modules["test"];

    std::fs::create_dir_all(tmp.path().join("Test")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "My Title".to_string());
    fields.insert("notes".to_string(), "Line one\nLine two".to_string());

    let _path = write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Test/note.md")).unwrap();

    assert!(
        content.contains("> [!tip]"),
        "body should contain callout opener, got: {content}"
    );
    assert!(
        content.contains("> Line one"),
        "first line should be blockquoted, got: {content}"
    );
    assert!(
        content.contains("> Line two"),
        "second line should be blockquoted, got: {content}"
    );
}

// ── Visibility filtering tests ────────────────────────────────────────────────

/// Module with a conditional `extra` field gated on `kind = "special"`.
fn visibility_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.test]
mode = "create"
path = "Test/note.md"
display_name = "Test"

[[modules.test.fields]]
name = "kind"
field_type = "static_select"
prompt = "Kind"
options = ["normal", "special"]

[[modules.test.fields]]
name = "extra"
field_type = "text"
prompt = "Extra"
[modules.test.fields.show_when]
field = "kind"
equals = "special"

[[modules.test.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
[modules.test.fields.show_when]
field = "kind"
equals = "special"
"####
    );
    Config::from_toml(&toml).expect("visibility test config should parse")
}

#[tokio::test]
async fn hidden_field_excluded_from_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = visibility_config(&base);
    let module = &config.modules["test"];

    std::fs::create_dir_all(tmp.path().join("Test")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    // kind = "normal" so `extra` (show_when kind=special) is hidden
    let mut fields = HashMap::new();
    fields.insert("kind".to_string(), "normal".to_string());
    fields.insert("extra".to_string(), "should-not-appear".to_string());

    write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Test/note.md")).unwrap();
    let parts: Vec<&str> = content.splitn(3, "---\n").collect();
    let frontmatter = parts.get(1).copied().unwrap_or("");

    assert!(
        !frontmatter.contains("extra"),
        "hidden field 'extra' should not appear in frontmatter, got: {frontmatter}"
    );
    assert!(
        !content.contains("should-not-appear"),
        "stale hidden value should not appear anywhere in output, got: {content}"
    );
}

#[tokio::test]
async fn visible_conditional_field_included_in_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = visibility_config(&base);
    let module = &config.modules["test"];

    std::fs::create_dir_all(tmp.path().join("Test")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    // kind = "special" so `extra` is visible
    let mut fields = HashMap::new();
    fields.insert("kind".to_string(), "special".to_string());
    fields.insert("extra".to_string(), "rare-value".to_string());

    write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Test/note.md")).unwrap();
    let parts: Vec<&str> = content.splitn(3, "---\n").collect();
    let frontmatter = parts.get(1).copied().unwrap_or("");

    assert!(
        frontmatter.contains("extra: rare-value"),
        "visible conditional field 'extra' should appear in frontmatter, got: {frontmatter}"
    );
}

#[tokio::test]
async fn hidden_field_excluded_from_body() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = visibility_config(&base);
    let module = &config.modules["test"];

    std::fs::create_dir_all(tmp.path().join("Test")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    // kind = "normal" so `notes` textarea (show_when kind=special) is hidden
    let mut fields = HashMap::new();
    fields.insert("kind".to_string(), "normal".to_string());
    fields.insert("notes".to_string(), "ghost-body-text".to_string());

    write_create(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Test/note.md")).unwrap();
    let parts: Vec<&str> = content.splitn(3, "---\n").collect();
    let body = parts.get(2).copied().unwrap_or("");

    assert!(
        !body.contains("ghost-body-text"),
        "hidden textarea field should not appear in body, got: {body}"
    );
}

// ── Append mode: field-level callout wrapping ───────────────────────────────

fn append_callout_config(base_path: &str) -> Config {
    let toml = format!(
        r#####"
[vault]
base_path = "{base_path}"

[modules.test]
mode = "append"
path = "Journal/daily.md"
append_under_header = "## Log"
append_template = "#### Entry\n{{{{notes}}}}"

[[modules.test.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
callout = "tip"
"#####
    );
    Config::from_toml(&toml).expect("test config should parse")
}

#[tokio::test]
async fn write_append_wraps_body_in_field_callout() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = append_callout_config(&base);
    let module = &config.modules["test"];

    // Create target file with the expected heading
    let journal_dir = tmp.path().join("Journal");
    std::fs::create_dir_all(&journal_dir).unwrap();
    std::fs::write(journal_dir.join("daily.md"), "## Log\n").unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("notes".to_string(), "Line one\nLine two".to_string());

    write_append(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_append should succeed");

    let content = std::fs::read_to_string(journal_dir.join("daily.md")).unwrap();

    assert!(
        content.contains("> [!tip]"),
        "appended content should contain callout opener, got: {content}"
    );
    assert!(
        content.contains("> Line one"),
        "first line should be blockquoted, got: {content}"
    );
    assert!(
        content.contains("> Line two"),
        "second line should be blockquoted, got: {content}"
    );
}

#[tokio::test]
async fn write_append_callout_override_in_template() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = append_callout_config(&base);
    let module = &config.modules["test"];

    let journal_dir = tmp.path().join("Journal");
    std::fs::create_dir_all(&journal_dir).unwrap();
    std::fs::write(journal_dir.join("daily.md"), "## Log\n").unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut fields = HashMap::new();
    fields.insert("notes".to_string(), "Important".to_string());

    let mut overrides = HashMap::new();
    overrides.insert("notes".to_string(), "warning".to_string());

    write_append(
        &transport,
        module,
        &fields,
        &CompositeData::new(),
        None,
        &overrides,
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_append should succeed");

    let content = std::fs::read_to_string(journal_dir.join("daily.md")).unwrap();

    assert!(
        content.contains("> [!warning]"),
        "override should produce warning callout, got: {content}"
    );
    assert!(
        !content.contains("> [!tip]"),
        "config callout should not appear when overridden, got: {content}"
    );
}

// --- Icon frontmatter tests ---

fn icon_create_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.coffee]
mode = "create"
path = "Coffee/note.md"
icon = "☕"

[[modules.coffee.fields]]
name = "method"
field_type = "text"
prompt = "Method"
"####
    );
    Config::from_toml(&toml).expect("icon config should parse")
}

#[tokio::test]
async fn create_mode_includes_module_icon_in_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = icon_create_config(&base);
    let module = &config.modules["coffee"];

    std::fs::create_dir_all(tmp.path().join("Coffee")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut values = HashMap::new();
    values.insert("method".to_string(), "V60".to_string());

    let path = write_create(
        &transport,
        module,
        &values,
        &CompositeData::new(),
        None,
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let full_path = tmp.path().join(&path);
    let content = std::fs::read_to_string(full_path).unwrap();

    assert!(
        content.contains("icon: ☕"),
        "frontmatter should contain icon field, got:\n{content}"
    );
    // icon should come after date
    let date_pos = content.find("date:").expect("date field should exist");
    let icon_pos = content.find("icon:").expect("icon field should exist");
    assert!(
        icon_pos > date_pos,
        "icon should appear after date in frontmatter"
    );
}

// --- daily_link frontmatter tests ---

fn daily_link_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.note]
mode = "create"
path = "Notes/note.md"
daily_link = true

[[modules.note.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####
    );
    Config::from_toml(&toml).expect("daily_link config should parse")
}

fn daily_link_disabled_config(base_path: &str) -> Config {
    let toml = format!(
        r####"
[vault]
base_path = "{base_path}"

[modules.note]
mode = "create"
path = "Notes/note.md"

[[modules.note.fields]]
name = "title"
field_type = "text"
prompt = "Title"
"####
    );
    Config::from_toml(&toml).expect("daily_link disabled config should parse")
}

#[tokio::test]
async fn daily_link_injects_daily_frontmatter_key() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = daily_link_config(&base);
    let module = &config.modules["note"];

    std::fs::create_dir_all(tmp.path().join("Notes")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut values = HashMap::new();
    values.insert("title".to_string(), "My Note".to_string());

    write_create(
        &transport,
        module,
        &values,
        &CompositeData::new(),
        Some("%Y%m%d"),
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Notes/note.md")).unwrap();

    // daily key should exist and be a wikilink
    assert!(
        content.contains("daily: \"[["),
        "frontmatter should contain daily wikilink, got:\n{content}"
    );
    assert!(
        content.contains("]]\""),
        "daily wikilink should be closed, got:\n{content}"
    );
}

#[tokio::test]
async fn daily_link_uses_date_format_from_caller() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = daily_link_config(&base);
    let module = &config.modules["note"];

    std::fs::create_dir_all(tmp.path().join("Notes")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut values = HashMap::new();
    values.insert("title".to_string(), "Formatted".to_string());

    write_create(
        &transport,
        module,
        &values,
        &CompositeData::new(),
        Some("%Y-%m-%d"),
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Notes/note.md")).unwrap();

    // The date format %Y-%m-%d produces e.g. 2026-04-05 — confirm dashes are present
    let daily_line = content
        .lines()
        .find(|l| l.starts_with("daily:"))
        .expect("daily key should be present");
    assert!(
        daily_line.contains('-'),
        "date format with dashes should produce dashes in daily link, got: {daily_line}"
    );
}

#[tokio::test]
async fn daily_link_false_does_not_inject_daily_key() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");
    let config = daily_link_disabled_config(&base);
    let module = &config.modules["note"];

    std::fs::create_dir_all(tmp.path().join("Notes")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut values = HashMap::new();
    values.insert("title".to_string(), "No Daily".to_string());

    write_create(
        &transport,
        module,
        &values,
        &CompositeData::new(),
        Some("%Y%m%d"),
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Notes/note.md")).unwrap();

    assert!(
        !content.contains("daily:"),
        "module without daily_link should not have daily key, got:\n{content}"
    );
}

#[tokio::test]
async fn daily_link_does_not_override_user_daily_field() {
    // If the user has a field named "daily", daily_link should not inject a duplicate.
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_str().unwrap().replace('\\', "/");

    let toml = format!(
        r####"
[vault]
base_path = "{base}"

[modules.note]
mode = "create"
path = "Notes/note.md"
daily_link = true

[[modules.note.fields]]
name = "daily"
field_type = "text"
prompt = "Daily"
"####
    );
    let config = Config::from_toml(&toml).expect("config with user daily field should parse");
    let module = &config.modules["note"];

    std::fs::create_dir_all(tmp.path().join("Notes")).unwrap();

    let transport = Transport::Fs(pour::transport::fs::FsWriter::new(tmp.path().to_path_buf()));

    let mut values = HashMap::new();
    values.insert("daily".to_string(), "[[custom]]".to_string());

    write_create(
        &transport,
        module,
        &values,
        &CompositeData::new(),
        Some("%Y%m%d"),
        &HashMap::new(),
        &::std::collections::HashMap::new(),
    )
    .await
    .expect("write_create should succeed");

    let content = std::fs::read_to_string(tmp.path().join("Notes/note.md")).unwrap();

    let daily_count = content.matches("daily:").count();
    assert_eq!(
        daily_count, 1,
        "should have exactly one daily key, got:\n{content}"
    );
    assert!(
        content.contains("[[custom]]"),
        "user-provided daily value should be preserved, got:\n{content}"
    );
}

// --- list flag + wikilink tests ---

#[test]
fn wikilink_comma_value_is_single_link_by_default() {
    // wikilink = true, list = false (default): the whole value wraps as one link.
    let result = apply_wikilink("tag1, tag2".to_string(), false);
    assert_eq!(
        result, "[[tag1, tag2]]",
        "list=false should wrap the whole value as one wikilink, got: {result}"
    );
}

#[test]
fn wikilink_comma_value_splits_when_list_true() {
    // wikilink = true, list = true: each comma-separated item gets its own link.
    let result = apply_wikilink("tag1, tag2".to_string(), true);
    assert_eq!(
        result, "[[tag1]], [[tag2]]",
        "list=true should split into individual wikilinks, got: {result}"
    );
}

// --- composite table unicode-width test ---

fn cjk_sub_fields() -> Vec<pour::config::SubFieldConfig> {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.c]
mode = "create"
path = "c.md"

[[modules.c.fields]]
name = "items"
field_type = "composite_array"
prompt = "Items"

[[modules.c.fields.sub_fields]]
name = "label"
field_type = "text"
prompt = "Label"

[[modules.c.fields.sub_fields]]
name = "count"
field_type = "number"
prompt = "Count"
"####;
    let config = Config::from_toml(toml).unwrap();
    config.modules["c"].fields[0].sub_fields.clone().unwrap()
}

#[test]
fn composite_table_width_accounts_for_cjk() {
    let subs = cjk_sub_fields();
    // CJK characters each occupy 2 display columns.
    let rows = vec![
        vec!["日本語".to_string(), "3".to_string()],
        vec!["ABC".to_string(), "1".to_string()],
    ];
    let table = render_composite_table(&subs, &rows);

    // The separator row width for the first column must be at least 6
    // (3 CJK chars × 2 display cols each), not 3 (byte/char count).
    // The separator line looks like: |---...-|---...-|
    // Extract the first column separator length by finding between the first two |.
    let sep_line = table
        .lines()
        .nth(1)
        .expect("table should have a separator row");
    // sep_line: |---------|-----|
    // First segment after leading | is the first column separator dashes.
    let first_col_sep = sep_line
        .trim_start_matches('|')
        .split('|')
        .next()
        .expect("separator should have column segment");
    // The dashes count (excluding the surrounding -) should be >= 6 for CJK content.
    let dash_count = first_col_sep.chars().filter(|&c| c == '-').count();
    assert!(
        dash_count >= 6,
        "CJK cell '日本語' (display width 6) should drive column width to at least 6 dashes, got {dash_count} in separator: {sep_line}"
    );

    // Also verify the CJK row itself is present and the cell text appears in the table.
    assert!(
        table.contains("日本語"),
        "CJK cell content should appear in table, got:\n{table}"
    );
}
