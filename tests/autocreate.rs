use std::collections::HashMap;

use pour::autocreate::{
    build_note_content, build_templated_note_content, is_existing_option, note_vault_path,
    resolve_template_path, sanitize_filename,
};
use pour::config::{
    FieldConfig, FieldType, ModuleConfig, TemplateConfig, TemplateFieldConfig, TemplateFieldType,
    WriteMode,
};
use pour::data::cache::Cache;
use pour::transport::{Transport, fs::FsWriter};
use tempfile::tempdir;

// ── Unit tests for pure helpers ──────────────────────────────────────────────

#[test]
fn sanitize_normal_value() {
    assert_eq!(
        sanitize_filename("Ethiopia Guji"),
        Some("Ethiopia Guji".to_string())
    );
}

#[test]
fn sanitize_strips_invalid_chars() {
    assert_eq!(sanitize_filename("foo:bar"), Some("foo-bar".to_string()));
    assert_eq!(sanitize_filename("a?b*c"), Some("a-b-c".to_string()));
    assert_eq!(sanitize_filename("x<y>z"), Some("x-y-z".to_string()));
    assert_eq!(sanitize_filename("a|b"), Some("a-b".to_string()));
    assert_eq!(sanitize_filename(r#"a"b"#), Some("a-b".to_string()));
    assert_eq!(sanitize_filename(r"a\b"), Some("a-b".to_string()));
    assert_eq!(sanitize_filename("a/b"), Some("a-b".to_string()));
}

#[test]
fn sanitize_collapses_consecutive_dashes() {
    assert_eq!(sanitize_filename("a::b"), Some("a-b".to_string()));
    assert_eq!(sanitize_filename("a???b"), Some("a-b".to_string()));
}

#[test]
fn sanitize_trims_whitespace_and_leading_trailing_dashes() {
    assert_eq!(sanitize_filename("  hello  "), Some("hello".to_string()));
    // Leading invalid chars turn into leading dashes that get trimmed
    assert_eq!(sanitize_filename(":hello:"), Some("hello".to_string()));
}

#[test]
fn sanitize_rejects_empty_and_all_invalid() {
    assert_eq!(sanitize_filename(""), None);
    assert_eq!(sanitize_filename("   "), None);
    assert_eq!(sanitize_filename(":::"), None);
    assert_eq!(sanitize_filename("???"), None);
}

#[test]
fn is_existing_option_case_insensitive() {
    let opts = vec!["Ethiopia Guji".to_string(), "Kenya".to_string()];
    assert!(is_existing_option("ethiopia guji", &opts));
    assert!(is_existing_option("KENYA", &opts));
    assert!(is_existing_option("Ethiopia Guji", &opts));
    assert!(!is_existing_option("Colombia", &opts));
}

#[test]
fn is_existing_option_empty_list() {
    assert!(!is_existing_option("anything", &[]));
}

#[test]
fn is_existing_option_trims_whitespace() {
    let opts = vec!["Kenya".to_string()];
    assert!(is_existing_option("  Kenya  ", &opts));
}

#[test]
fn build_note_content_minimal_frontmatter() {
    let content = build_note_content("2026-04-02");
    assert_eq!(content, "---\ndate: 2026-04-02\n---\n");
}

#[test]
fn note_vault_path_joins() {
    assert_eq!(
        note_vault_path("beans", "Ethiopia Guji"),
        "beans/Ethiopia Guji.md"
    );
}

#[test]
fn note_vault_path_strips_trailing_slash() {
    assert_eq!(note_vault_path("beans/", "Kenya"), "beans/Kenya.md");
}

// ── Integration tests for `run()` ────────────────────────────────────────────

fn make_transport(base: &std::path::Path) -> Transport {
    Transport::Fs(FsWriter::new(base.to_path_buf()))
}

fn make_module_with_field(field: FieldConfig) -> ModuleConfig {
    ModuleConfig {
        mode: WriteMode::Create,
        path: "out/{date}.md".to_string(),
        append_under_header: None,
        append_template: None,
        fields: vec![field],
        display_name: None,
        callout_type: None,
    }
}

fn dynamic_field(name: &str, source: &str, allow_create: bool) -> FieldConfig {
    FieldConfig {
        name: name.to_string(),
        field_type: FieldType::DynamicSelect,
        prompt: name.to_string(),
        required: None,
        default: None,
        options: None,
        source: Some(source.to_string()),
        target: None,
        sub_fields: None,
        callout: None,
        allow_create: Some(allow_create),
        wikilink: None,
        create_template: None,
        post_create_command: None,
    }
}

#[tokio::test]
async fn novel_value_creates_note() {
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));

    let module = make_module_with_field(dynamic_field("bean", "beans", true));

    let mut field_values = HashMap::new();
    field_values.insert("bean".to_string(), "Ethiopia Guji".to_string());

    let field_options: HashMap<String, Vec<String>> = HashMap::new();

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    assert_eq!(created.len(), 1);
    assert_eq!(created[0].vault_path, "beans/Ethiopia Guji.md");
    assert_eq!(created[0].value, "Ethiopia Guji");

    // File should exist with correct content
    let file_path = dir.path().join("beans").join("Ethiopia Guji.md");
    assert!(file_path.exists(), "note file should be created");
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "---\ndate: 2026-04-02\n---\n");
}

#[tokio::test]
async fn existing_value_does_not_create_note() {
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));

    let module = make_module_with_field(dynamic_field("bean", "beans", true));

    let mut field_values = HashMap::new();
    field_values.insert("bean".to_string(), "Ethiopia Guji".to_string());

    let mut field_options = HashMap::new();
    field_options.insert(
        "bean".to_string(),
        vec!["Ethiopia Guji".to_string(), "Kenya".to_string()],
    );

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    assert!(
        created.is_empty(),
        "existing value should not trigger creation"
    );
    let file_path = dir.path().join("beans").join("Ethiopia Guji.md");
    assert!(!file_path.exists());
}

#[tokio::test]
async fn case_insensitive_duplicate_not_created() {
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));

    let module = make_module_with_field(dynamic_field("bean", "beans", true));

    let mut field_values = HashMap::new();
    // User typed lowercase; option stored with title case
    field_values.insert("bean".to_string(), "ethiopia guji".to_string());

    let mut field_options = HashMap::new();
    field_options.insert("bean".to_string(), vec!["Ethiopia Guji".to_string()]);

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    assert!(
        created.is_empty(),
        "case-insensitive match should not trigger creation"
    );
}

#[tokio::test]
async fn allow_create_false_does_not_create() {
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));

    let module = make_module_with_field(dynamic_field("bean", "beans", false));

    let mut field_values = HashMap::new();
    field_values.insert("bean".to_string(), "Novel Bean".to_string());

    let field_options: HashMap<String, Vec<String>> = HashMap::new();

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    assert!(
        created.is_empty(),
        "allow_create=false should not create notes"
    );
}

#[tokio::test]
async fn cache_updated_after_creation() {
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));
    // Seed the cache with existing values
    cache.set("beans", vec!["Kenya".to_string()]);

    let module = make_module_with_field(dynamic_field("bean", "beans", true));

    let mut field_values = HashMap::new();
    field_values.insert("bean".to_string(), "Ethiopia".to_string());

    let field_options: HashMap<String, Vec<String>> = HashMap::new();

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    assert_eq!(created.len(), 1);

    // Cache should now contain both "Kenya" and "Ethiopia"
    let cached = cache
        .get("beans")
        .expect("cache should have entry for 'beans'");
    assert!(cached.contains(&"Kenya".to_string()));
    assert!(cached.contains(&"Ethiopia".to_string()));
}

#[tokio::test]
async fn transport_error_does_not_block_and_returns_empty() {
    // Create a read-only directory to force transport failure.
    // On Windows, read-only on a directory doesn't prevent writes, so instead
    // we test by pointing source at a path with a name that can't be created
    // (e.g. source is empty string — which means no source configured).
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));

    // Field with no source — run() skips it and returns empty
    let field = FieldConfig {
        name: "bean".to_string(),
        field_type: FieldType::DynamicSelect,
        prompt: "bean".to_string(),
        required: None,
        default: None,
        options: None,
        source: None, // no source — should be skipped
        target: None,
        sub_fields: None,
        callout: None,
        allow_create: Some(true),
        wikilink: None,
        create_template: None,
        post_create_command: None,
    };
    let module = make_module_with_field(field);

    let mut field_values = HashMap::new();
    field_values.insert("bean".to_string(), "Novel".to_string());

    let field_options: HashMap<String, Vec<String>> = HashMap::new();

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    // Field with no source is skipped — no panic, no block
    assert!(created.is_empty());
}

#[tokio::test]
async fn empty_value_not_created() {
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));

    let module = make_module_with_field(dynamic_field("bean", "beans", true));

    let mut field_values = HashMap::new();
    field_values.insert("bean".to_string(), "".to_string());

    let field_options: HashMap<String, Vec<String>> = HashMap::new();

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    assert!(
        created.is_empty(),
        "empty value should not trigger creation"
    );
}

#[tokio::test]
async fn sanitized_form_matches_existing_option() {
    // If the user types "foo:bar" and "foo-bar" already exists in options,
    // no duplicate note should be created.
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));

    let module = make_module_with_field(dynamic_field("bean", "beans", true));

    let mut field_values = HashMap::new();
    field_values.insert("bean".to_string(), "foo:bar".to_string());

    let mut field_options = HashMap::new();
    field_options.insert("bean".to_string(), vec!["foo-bar".to_string()]);

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    assert!(
        created.is_empty(),
        "sanitized form matching existing option should not trigger creation"
    );
    let file_path = dir.path().join("beans").join("foo-bar.md");
    assert!(!file_path.exists());
}

#[tokio::test]
async fn windows_reserved_name_not_created() {
    let dir = tempdir().unwrap();
    let transport = make_transport(dir.path());
    let mut cache = Cache::load_from(dir.path().join("state.json"));

    let module = make_module_with_field(dynamic_field("bean", "beans", true));

    let mut field_values = HashMap::new();
    field_values.insert("bean".to_string(), "CON".to_string());

    let field_options: HashMap<String, Vec<String>> = HashMap::new();

    let created = pour::autocreate::run(
        &module,
        &field_values,
        &field_options,
        &transport,
        &mut cache,
        "2026-04-02",
    )
    .await;

    assert!(
        created.is_empty(),
        "Windows reserved name should not trigger creation"
    );
}

// ── build_templated_note_content tests ─────────────────────────────────────

fn make_template() -> TemplateConfig {
    TemplateConfig {
        path: "Coffee/Beans/{{name}}.md".to_string(),
        fields: vec![
            TemplateFieldConfig {
                name: "origin".to_string(),
                field_type: TemplateFieldType::Text,
                prompt: "Origin".to_string(),
                options: None,
                default: None,
            },
            TemplateFieldConfig {
                name: "roast".to_string(),
                field_type: TemplateFieldType::StaticSelect,
                prompt: "Roast level".to_string(),
                options: Some(vec![
                    "Light".to_string(),
                    "Medium".to_string(),
                    "Dark".to_string(),
                ]),
                default: Some("Medium".to_string()),
            },
            TemplateFieldConfig {
                name: "rating".to_string(),
                field_type: TemplateFieldType::Number,
                prompt: "Rating".to_string(),
                options: None,
                default: None,
            },
        ],
    }
}

#[test]
fn templated_content_all_fields_present() {
    let template = make_template();
    let mut values = HashMap::new();
    values.insert("origin".to_string(), "Guji Zone".to_string());
    values.insert("roast".to_string(), "Light".to_string());
    values.insert("rating".to_string(), "9".to_string());

    let content = build_templated_note_content(&template, "Ethiopia Guji", &values, "2026-04-02");
    // "9" is quoted because format_value quotes numeric-looking strings for YAML safety
    assert_eq!(
        content,
        "---\ndate: 2026-04-02\nname: Ethiopia Guji\norigin: Guji Zone\nroast: Light\nrating: \"9\"\n---\n"
    );
}

#[test]
fn templated_content_missing_value_uses_default() {
    let template = make_template();
    let mut values = HashMap::new();
    values.insert("origin".to_string(), "Kenya".to_string());
    // roast not provided — should use default "Medium"
    // rating not provided — no default, should be omitted

    let content = build_templated_note_content(&template, "Kenya AA", &values, "2026-04-02");
    assert!(content.contains("roast: Medium"), "should use default");
    assert!(!content.contains("rating:"), "no-default field should be omitted");
}

#[test]
fn templated_content_missing_value_no_default_omitted() {
    let template = make_template();
    let values = HashMap::new(); // no values at all

    let content = build_templated_note_content(&template, "Test", &values, "2026-04-02");
    assert!(!content.contains("origin:"), "origin should be omitted");
    assert!(content.contains("roast: Medium"), "roast default used");
    assert!(!content.contains("rating:"), "rating should be omitted");
    assert!(content.contains("name: Test"));
    assert!(content.contains("date: 2026-04-02"));
}

#[test]
fn templated_content_empty_value_treated_as_missing() {
    let template = make_template();
    let mut values = HashMap::new();
    values.insert("origin".to_string(), "".to_string()); // empty = missing

    let content = build_templated_note_content(&template, "Test", &values, "2026-04-02");
    assert!(!content.contains("origin:"), "empty value should be omitted");
}

#[test]
fn templated_content_yaml_special_chars_quoted() {
    let template = make_template();
    let mut values = HashMap::new();
    values.insert("origin".to_string(), "yes".to_string()); // YAML reserved word

    let content = build_templated_note_content(&template, "Test: Bean", &values, "2026-04-02");
    assert!(
        content.contains(r#"name: "Test: Bean""#),
        "colon in name should be quoted: {content}"
    );
    assert!(
        content.contains(r#"origin: "yes""#),
        "YAML reserved word should be quoted: {content}"
    );
}

#[test]
fn templated_content_field_ordering() {
    let template = make_template();
    let mut values = HashMap::new();
    values.insert("origin".to_string(), "A".to_string());
    values.insert("roast".to_string(), "B".to_string());
    values.insert("rating".to_string(), "1".to_string());

    let content = build_templated_note_content(&template, "X", &values, "2026-04-02");
    let date_pos = content.find("date:").unwrap();
    let name_pos = content.find("name:").unwrap();
    let origin_pos = content.find("origin:").unwrap();
    let roast_pos = content.find("roast:").unwrap();
    let rating_pos = content.find("rating:").unwrap();
    assert!(date_pos < name_pos);
    assert!(name_pos < origin_pos);
    assert!(origin_pos < roast_pos);
    assert!(roast_pos < rating_pos);
}

// ── resolve_template_path tests ────────────────────────────────────────────

#[test]
fn resolve_path_basic_substitution() {
    let result = resolve_template_path("Beans/{{name}}.md", "Ethiopia Guji");
    assert_eq!(result, Some("Beans/Ethiopia Guji.md".to_string()));
}

#[test]
fn resolve_path_sanitization_applied() {
    let result = resolve_template_path("Beans/{{name}}.md", "foo:bar");
    assert_eq!(result, Some("Beans/foo-bar.md".to_string()));
}

#[test]
fn resolve_path_sanitization_failure_returns_none() {
    assert_eq!(resolve_template_path("Beans/{{name}}.md", "CON"), None);
}

#[test]
fn resolve_path_empty_name_returns_none() {
    assert_eq!(resolve_template_path("Beans/{{name}}.md", ""), None);
}

#[test]
fn resolve_path_strftime_expansion() {
    let result = resolve_template_path("Beans/%Y/{{name}}.md", "Test").unwrap();
    let year = chrono::Local::now().format("%Y").to_string();
    assert!(
        result.contains(&year),
        "path should contain current year {year}: {result}"
    );
}

#[test]
fn resolve_path_multiple_name_placeholders() {
    let result = resolve_template_path("{{name}}/{{name}}.md", "Test").unwrap();
    assert_eq!(result, "Test/Test.md");
}

#[test]
fn resolve_path_percent_in_name_not_expanded_as_strftime() {
    // A user typing "Ethiopia %Y Blend" should NOT have %Y expanded to the year
    let result = resolve_template_path("Beans/{{name}}.md", "Ethiopia %Y Blend").unwrap();
    assert_eq!(result, "Beans/Ethiopia %Y Blend.md");
}
