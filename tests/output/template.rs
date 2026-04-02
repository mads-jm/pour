use chrono::Local;
use pour::config::Config;
use pour::output::CompositeData;
use pour::output::template::{render_append_template, render_path};
use std::collections::HashMap;

/// Minimal module config for template tests that don't use composite fields.
fn dummy_module() -> pour::config::ModuleConfig {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.t]
mode = "append"
path = "t.md"
append_under_header = "## Log"

[[modules.t.fields]]
name = "body"
field_type = "text"
prompt = "Body"
"####;
    let config = Config::from_toml(toml).unwrap();
    config.modules.into_values().next().unwrap()
}

fn no_composites() -> CompositeData {
    CompositeData::new()
}

#[test]
fn render_path_substitutes_date_tokens() {
    let fields = HashMap::new();
    let result = render_path("Journal/%Y/%Y-%m-%d.md", &fields, None);
    let today = Local::now().format("%Y-%m-%d").to_string();
    let year = Local::now().format("%Y").to_string();

    assert!(
        result.contains(&today),
        "path should contain today's date, got: {result}"
    );
    assert!(
        result.starts_with(&format!("Journal/{year}/")),
        "path should start with Journal/YYYY/, got: {result}"
    );
    assert!(result.ends_with(".md"), "path should end with .md");
}

#[test]
fn render_path_no_tokens_passes_through() {
    let fields = HashMap::new();
    let result = render_path("static/path.md", &fields, None);
    assert_eq!(result, "static/path.md");
}

#[test]
fn render_path_substitutes_field_placeholders() {
    let mut fields = HashMap::new();
    fields.insert("bean".to_string(), "Ethiopian".to_string());
    let result = render_path("Coffee/{{bean}} %Y%m%d.md", &fields, None);
    let today = Local::now().format("%Y%m%d").to_string();
    assert_eq!(result, format!("Coffee/Ethiopian {today}.md"));
}

#[test]
fn render_path_date_token_uses_vault_format() {
    let fields = HashMap::new();
    let result = render_path("Daily/{{date}}.md", &fields, Some("%Y-%m-%d"));
    let today = Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(result, format!("Daily/{today}.md"));
}

#[test]
fn render_path_date_token_uses_default_without_vault_format() {
    let fields = HashMap::new();
    let result = render_path("Daily/{{date}}.md", &fields, None);
    let today = Local::now().format("%Y%m%d").to_string();
    assert_eq!(result, format!("Daily/{today}.md"));
}

#[test]
fn render_path_strips_unresolved_placeholders() {
    let fields = HashMap::new();
    let result = render_path("Coffee/{{unknown}}.md", &fields, None);
    assert_eq!(result, "Coffee/.md");
}

#[test]
fn render_append_template_replaces_fields() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Hello world".to_string());
    fields.insert("mood".to_string(), "happy".to_string());

    let m = dummy_module();
    let result = render_append_template("Mood: {{mood}} | {{body}}", &fields, &m, &no_composites());
    assert_eq!(result, "Mood: happy | Hello world");
}

#[test]
fn render_append_template_special_time_token() {
    let fields = HashMap::new();
    let m = dummy_module();
    let result = render_append_template("> [!note] {{time}}", &fields, &m, &no_composites());
    let now = Local::now().format("%H:%M").to_string();
    assert!(
        result.contains(&now),
        "should contain current time, got: {result}"
    );
}

#[test]
fn render_append_template_special_date_token() {
    let fields = HashMap::new();
    let m = dummy_module();
    let result = render_append_template("Date: {{date}}", &fields, &m, &no_composites());
    let today = Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(result, format!("Date: {today}"));
}

#[test]
fn render_append_template_missing_field_left_as_is() {
    let fields = HashMap::new();
    let m = dummy_module();
    let result = render_append_template("Value: {{unknown}}", &fields, &m, &no_composites());
    assert_eq!(result, "Value: {{unknown}}");
}

#[test]
fn render_append_template_mixed_known_and_unknown() {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), "Alice".to_string());

    let m = dummy_module();
    let result = render_append_template("{{name}} said {{quote}}", &fields, &m, &no_composites());
    assert_eq!(result, "Alice said {{quote}}");
}

#[test]
fn render_append_template_realistic_journal() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "Morning reflection".to_string());
    fields.insert("body".to_string(), "Felt productive today.".to_string());

    let m = dummy_module();
    let template = "#### {{time}}\n> [!note] {{title}}\n> {{body}}";
    let result = render_append_template(template, &fields, &m, &no_composites());

    let now = Local::now().format("%H:%M").to_string();
    assert!(
        result.starts_with(&format!("#### {now}")),
        "should start with h4 time header, got: {result}"
    );
    assert!(
        result.contains("> [!note] Morning reflection"),
        "should have title in callout"
    );
    assert!(
        result.contains("Felt productive today."),
        "should have body"
    );
}

fn callout_module() -> pour::config::ModuleConfig {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.t]
mode = "append"
path = "t.md"
append_under_header = "## Log"
callout_type = "tip"

[[modules.t.fields]]
name = "body"
field_type = "text"
prompt = "Body"
"####;
    let config = Config::from_toml(toml).unwrap();
    config.modules.into_values().next().unwrap()
}

#[test]
fn render_append_template_callout_placeholder() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Some content".to_string());

    let m = callout_module();
    let result = render_append_template(
        "> [!{{callout}}] Title\n> {{body}}",
        &fields,
        &m,
        &no_composites(),
    );

    assert!(
        result.contains("> [!tip] Title"),
        "{{{{callout}}}} should resolve to module callout_type, got: {result}"
    );
    assert!(result.contains("> Some content"), "body should be present");
}

#[test]
fn render_append_template_callout_placeholder_without_type() {
    let fields = HashMap::new();
    let m = dummy_module(); // no callout_type set
    let result = render_append_template("> [!{{callout}}]", &fields, &m, &no_composites());

    assert!(
        result.contains("{{callout}}"),
        "unresolved {{{{callout}}}} should be left as-is when no callout_type, got: {result}"
    );
}

fn composite_module() -> pour::config::ModuleConfig {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.c]
mode = "append"
path = "c.md"
append_under_header = "## Brews"
append_template = "Bean: {{bean}}\n{{recipe}}"

[[modules.c.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"

[[modules.c.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Brew stages"

[[modules.c.fields.sub_fields]]
name = "pour"
field_type = "number"
prompt = "Pour (g)"

[[modules.c.fields.sub_fields]]
name = "time"
field_type = "number"
prompt = "Time (s)"

[[modules.c.fields.sub_fields]]
name = "technique"
field_type = "static_select"
prompt = "Technique"
options = ["Bloom", "Spiral"]
"####;
    let config = Config::from_toml(toml).unwrap();
    config.modules.into_values().next().unwrap()
}

#[test]
fn render_append_template_composite_as_markdown_table() {
    let mut fields = HashMap::new();
    fields.insert("bean".to_string(), "Ethiopian".to_string());

    let mut composites = CompositeData::new();
    composites.insert(
        "recipe".to_string(),
        vec![
            vec!["50".to_string(), "30".to_string(), "Bloom".to_string()],
            vec!["100".to_string(), "45".to_string(), "Spiral".to_string()],
        ],
    );

    let m = composite_module();
    let result = render_append_template("Bean: {{bean}}\n{{recipe}}", &fields, &m, &composites);

    assert!(result.contains("Bean: Ethiopian"), "scalar field replaced");
    assert!(result.contains("| Pour (g)"), "table header");
    assert!(result.contains("| Time (s)"), "table header");
    assert!(result.contains("| Technique |"), "table header");
    assert!(result.contains("| 50"), "first row data");
    assert!(result.contains("| 100"), "second row data");
}
