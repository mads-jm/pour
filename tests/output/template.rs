use chrono::Local;
use pour::config::Config;
use pour::output::CompositeData;
use pour::output::template::{render_append_template, render_path, slug_from_title, slug_tokens};
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

fn no_fields_map() -> HashMap<String, String> {
    HashMap::new()
}

fn no_composites() -> CompositeData {
    CompositeData::new()
}

fn no_overrides() -> HashMap<String, String> {
    HashMap::new()
}

#[test]
fn render_path_substitutes_date_tokens() {
    let fields = HashMap::new();
    let result = render_path(
        "Journal/%Y/%Y-%m-%d.md",
        &fields,
        None,
        chrono::Local::now(),
    );
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
    let result = render_path("static/path.md", &fields, None, chrono::Local::now());
    assert_eq!(result, "static/path.md");
}

#[test]
fn render_path_substitutes_field_placeholders() {
    let mut fields = HashMap::new();
    fields.insert("bean".to_string(), "Ethiopian".to_string());
    let result = render_path(
        "Coffee/{{bean}} %Y%m%d.md",
        &fields,
        None,
        chrono::Local::now(),
    );
    let today = Local::now().format("%Y%m%d").to_string();
    assert_eq!(result, format!("Coffee/Ethiopian {today}.md"));
}

#[test]
fn render_path_date_token_uses_vault_format() {
    let fields = HashMap::new();
    let result = render_path(
        "Daily/{{date}}.md",
        &fields,
        Some("%Y-%m-%d"),
        chrono::Local::now(),
    );
    let today = Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(result, format!("Daily/{today}.md"));
}

#[test]
fn render_path_date_token_uses_default_without_vault_format() {
    let fields = HashMap::new();
    let result = render_path("Daily/{{date}}.md", &fields, None, chrono::Local::now());
    let today = Local::now().format("%Y%m%d").to_string();
    assert_eq!(result, format!("Daily/{today}.md"));
}

#[test]
fn render_path_strips_unresolved_placeholders() {
    let fields = HashMap::new();
    let result = render_path("Coffee/{{unknown}}.md", &fields, None, chrono::Local::now());
    assert_eq!(result, "Coffee/.md");
}

#[test]
fn render_append_template_replaces_fields() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Hello world".to_string());
    fields.insert("mood".to_string(), "happy".to_string());

    let m = dummy_module();
    let result = render_append_template(
        "Mood: {{mood}} | {{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );
    assert_eq!(result, "Mood: happy | Hello world");
}

#[test]
fn render_append_template_special_time_token() {
    let fields = HashMap::new();
    let m = dummy_module();
    let result = render_append_template(
        "> [!note] {{time}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );
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
    let result = render_append_template(
        "Date: {{date}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );
    let today = Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(result, format!("Date: {today}"));
}

#[test]
fn render_append_template_missing_field_left_as_is() {
    let fields = HashMap::new();
    let m = dummy_module();
    let result = render_append_template(
        "Value: {{unknown}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );
    assert_eq!(result, "Value: {{unknown}}");
}

#[test]
fn render_append_template_mixed_known_and_unknown() {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), "Alice".to_string());

    let m = dummy_module();
    let result = render_append_template(
        "{{name}} said {{quote}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );
    assert_eq!(result, "Alice said {{quote}}");
}

#[test]
fn render_append_template_realistic_journal() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "Morning reflection".to_string());
    fields.insert("body".to_string(), "Felt productive today.".to_string());

    let m = dummy_module();
    let template = "#### {{time}}\n> [!note] {{title}}\n> {{body}}";
    let result = render_append_template(
        template,
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

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
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
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
    let result = render_append_template(
        "> [!{{callout}}]",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

    assert!(
        result.contains("{{callout}}"),
        "unresolved {{{{callout}}}} should be left as-is when no callout_type, got: {result}"
    );
}

/// Regression: field values containing `%` must not be treated as strftime
/// specifiers. Previously, field substitution ran before strftime expansion,
/// causing e.g. "Fixed 20% of bugs" to corrupt the output.
#[test]
fn render_path_percent_in_field_value_is_literal() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "Fixed 20% of bugs".to_string());
    // Template has no strftime tokens other than what's in the field value.
    let result = render_path("Notes/{{title}}.md", &fields, None, chrono::Local::now());
    assert_eq!(
        result, "Notes/Fixed 20% of bugs.md",
        "percent in field value should be preserved literally, got: {result}"
    );
}

#[test]
fn render_path_percent_in_field_value_with_strftime_tokens() {
    let mut fields = HashMap::new();
    fields.insert("tag".to_string(), "gain-5%".to_string());
    let result = render_path("Log/%Y/{{tag}}.md", &fields, None, chrono::Local::now());
    let year = Local::now().format("%Y").to_string();
    assert_eq!(
        result,
        format!("Log/{year}/gain-5%.md"),
        "strftime tokens in template should expand, but % in field value must not, got: {result}"
    );
}

#[test]
fn render_append_template_percent_in_field_value_is_literal() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Improved by 30% today".to_string());
    let m = dummy_module();
    let result = render_append_template(
        "Note: {{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );
    assert_eq!(
        result, "Note: Improved by 30% today",
        "percent in field value must not be interpreted as strftime, got: {result}"
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
    let result = render_append_template(
        "Bean: {{bean}}\n{{recipe}}",
        &fields,
        &m,
        &composites,
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

    assert!(result.contains("Bean: Ethiopian"), "scalar field replaced");
    assert!(result.contains("| Pour (g)"), "table header");
    assert!(result.contains("| Time (s)"), "table header");
    assert!(result.contains("| Technique |"), "table header");
    assert!(result.contains("| 50"), "first row data");
    assert!(result.contains("| 100"), "second row data");
}

/// Build a module with a `drink_type` trigger field and a `drink_detail`
/// field that is only visible when `drink_type == "coffee"`.
fn visibility_module() -> pour::config::ModuleConfig {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.v]
mode = "append"
path = "v.md"
append_under_header = "## Log"

[[modules.v.fields]]
name = "drink_type"
field_type = "static_select"
prompt = "Drink type"
options = ["coffee", "tea"]

[[modules.v.fields]]
name = "drink_detail"
field_type = "text"
prompt = "Detail"
show_when = { field = "drink_type", equals = "coffee" }
"####;
    let config = pour::config::Config::from_toml(toml).unwrap();
    config.modules.into_values().next().unwrap()
}

#[test]
fn render_append_template_hidden_field_placeholder_empty() {
    // drink_type is "tea" so drink_detail's show_when condition is NOT met.
    let mut fields = HashMap::new();
    fields.insert("drink_type".to_string(), "tea".to_string());
    fields.insert("drink_detail".to_string(), "Ethiopian".to_string());

    let m = visibility_module();
    let result = render_append_template(
        "Type: {{drink_type}} Detail: {{drink_detail}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

    assert!(
        result.contains("Type: tea"),
        "visible field should render its value, got: {result}"
    );
    assert!(
        result.contains("Detail: "),
        "hidden field placeholder should be present but empty, got: {result}"
    );
    assert!(
        !result.contains("Ethiopian"),
        "hidden field value must not appear, got: {result}"
    );
}

#[test]
fn render_append_template_visible_field_renders_normally() {
    // drink_type is "coffee" so drink_detail's show_when condition IS met.
    let mut fields = HashMap::new();
    fields.insert("drink_type".to_string(), "coffee".to_string());
    fields.insert("drink_detail".to_string(), "Ethiopian".to_string());

    let m = visibility_module();
    let result = render_append_template(
        "Type: {{drink_type}} Detail: {{drink_detail}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

    assert_eq!(
        result, "Type: coffee Detail: Ethiopian",
        "both visible fields should render their values, got: {result}"
    );
}

// ── Field-level callout wrapping in append templates ────────────────────────

fn field_callout_module() -> pour::config::ModuleConfig {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.t]
mode = "append"
path = "t.md"
append_under_header = "## Log"

[[modules.t.fields]]
name = "body"
field_type = "textarea"
prompt = "Body"
callout = "tip"
"####;
    let config = Config::from_toml(toml).unwrap();
    config.modules.into_values().next().unwrap()
}

#[test]
fn render_append_template_field_callout_wraps_value() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Line one\nLine two".to_string());

    let m = field_callout_module();
    let result = render_append_template(
        "{{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

    assert!(
        result.contains("> [!tip]"),
        "should contain callout opener, got: {result}"
    );
    assert!(
        result.contains("> Line one"),
        "first line should be blockquoted, got: {result}"
    );
    assert!(
        result.contains("> Line two"),
        "second line should be blockquoted, got: {result}"
    );
}

#[test]
fn render_append_template_field_callout_empty_value() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), String::new());

    let m = field_callout_module();
    let result = render_append_template(
        "Content: {{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

    assert!(
        !result.contains("> [!tip]"),
        "empty value should not produce callout block, got: {result}"
    );
}

#[test]
fn render_append_template_callout_override_takes_precedence() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Important".to_string());

    let m = field_callout_module(); // config has callout = "tip"
    let mut overrides = HashMap::new();
    overrides.insert("body".to_string(), "warning".to_string());

    let result = render_append_template(
        "{{body}}",
        &fields,
        &m,
        &no_composites(),
        &overrides,
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

    assert!(
        result.contains("> [!warning]"),
        "override should take precedence over config callout, got: {result}"
    );
    assert!(
        !result.contains("> [!tip]"),
        "config callout should not appear when overridden, got: {result}"
    );
}

#[test]
fn render_append_template_field_callout_title_from_runtime() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Hello".to_string());

    let m = field_callout_module();
    let mut titles = HashMap::new();
    titles.insert("body".to_string(), "Reminder".to_string());

    let result = render_append_template(
        "{{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &titles,
        chrono::Local::now(),
    );

    assert!(
        result.contains("> [!tip] Reminder"),
        "runtime title should appear after callout type, got: {result}"
    );
}

#[test]
fn render_append_template_field_callout_title_from_config() {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.t]
mode = "append"
path = "t.md"
append_under_header = "## Log"

[[modules.t.fields]]
name = "body"
field_type = "textarea"
prompt = "Body"
callout = "note"
callout_title = "Default Title"
"####;
    let m = Config::from_toml(toml)
        .unwrap()
        .modules
        .into_values()
        .next()
        .unwrap();

    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "text".to_string());

    let result = render_append_template(
        "{{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &::std::collections::HashMap::new(),
        chrono::Local::now(),
    );

    assert!(
        result.contains("> [!note] Default Title"),
        "config callout_title should appear, got: {result}"
    );
}

#[test]
fn render_append_template_runtime_title_overrides_config_title() {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.t]
mode = "append"
path = "t.md"
append_under_header = "## Log"

[[modules.t.fields]]
name = "body"
field_type = "textarea"
prompt = "Body"
callout = "note"
callout_title = "Default"
"####;
    let m = Config::from_toml(toml)
        .unwrap()
        .modules
        .into_values()
        .next()
        .unwrap();

    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "text".to_string());

    let mut titles = HashMap::new();
    titles.insert("body".to_string(), "Custom".to_string());

    let result = render_append_template(
        "{{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &titles,
        chrono::Local::now(),
    );

    assert!(
        result.contains("> [!note] Custom"),
        "runtime title should override config title, got: {result}"
    );
    assert!(
        !result.contains("Default"),
        "default title should be hidden when runtime title set, got: {result}"
    );
}

// ── {{slug}} / {{slug_or_time}} ──────────────────────────────────────────────
//
// The slug must match the Lyra Templater's JS exactly:
//   title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")
// so a hand-authored toss and a poured one land on the same filename.

/// Fixed local timestamp: 2026-07-16 14:32:55.
fn slug_now() -> chrono::DateTime<Local> {
    use chrono::TimeZone as _;
    Local.with_ymd_and_hms(2026, 7, 16, 14, 32, 55).unwrap()
}

#[test]
fn slug_kebab_cases_a_title() {
    assert_eq!(slug_from_title("Peace vs Effort"), "peace-vs-effort");
}

#[test]
fn slug_collapses_punctuation_runs_into_single_dashes() {
    assert_eq!(slug_from_title("a!!!b???c"), "a-b-c");
    assert_eq!(slug_from_title("hello -- world"), "hello-world");
}

#[test]
fn slug_trims_leading_and_trailing_dashes() {
    assert_eq!(slug_from_title("---trim me---"), "trim-me");
    assert_eq!(slug_from_title("  spaced  "), "spaced");
}

#[test]
fn slug_of_untitled_is_empty() {
    assert_eq!(slug_from_title(""), "");
}

#[test]
fn slug_of_punctuation_only_is_empty() {
    // Every character is dropped, and the trim leaves nothing behind — not "-".
    assert_eq!(slug_from_title("!!!"), "");
    assert_eq!(slug_from_title("---"), "");
}

#[test]
fn slug_drops_non_ascii_like_the_js_regex() {
    // `[a-z0-9]` never matches a non-ASCII letter, so JS turns "café" into
    // "caf-" and then trims the trailing dash. Pour must do the same, however
    // unintuitive — parity with the Templater is the whole point.
    assert_eq!(slug_from_title("café"), "caf");
    assert_eq!(slug_from_title("naïve idea"), "na-ve-idea");
    assert_eq!(slug_from_title("日本語"), "");
}

#[test]
fn slug_keeps_digits() {
    assert_eq!(slug_from_title("v2 Roadmap 2026"), "v2-roadmap-2026");
}

#[test]
fn slug_token_is_dash_prefixed_in_a_path() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "Peace vs Effort".to_string());

    let result = render_path("inbox/%Y%m%d-%H%M%S{{slug}}.md", &fields, None, slug_now());

    assert_eq!(result, "inbox/20260716-143255-peace-vs-effort.md");
}

#[test]
fn slug_token_is_empty_when_untitled() {
    // The regression this guards: an unregistered {{slug}} would be *stripped*
    // by render_path's unknown-placeholder pass, which produces this same
    // filename — so this test only means something alongside the titled case.
    let result = render_path(
        "inbox/%Y%m%d-%H%M%S{{slug}}.md",
        &no_fields_map(),
        None,
        slug_now(),
    );

    assert_eq!(result, "inbox/20260716-143255.md");
}

#[test]
fn slug_token_is_empty_when_title_is_punctuation_only() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "!!!".to_string());

    let result = render_path("inbox/%Y%m%d-%H%M%S{{slug}}.md", &fields, None, slug_now());

    assert_eq!(result, "inbox/20260716-143255.md");
}

#[test]
fn slug_or_time_falls_back_to_timestamp_when_untitled() {
    let (slug, slug_or_time) = slug_tokens("", slug_now());

    assert_eq!(slug, "");
    assert_eq!(slug_or_time, "20260716-143255");
}

#[test]
fn slug_or_time_is_the_bare_slug_when_titled() {
    let (slug, slug_or_time) = slug_tokens("Peace vs Effort", slug_now());

    assert_eq!(slug, "-peace-vs-effort");
    assert_eq!(slug_or_time, "peace-vs-effort");
}

#[test]
fn slug_tokens_are_disjoint_in_a_template() {
    // {{slug}} is a prefix of neither {{slug_or_time}} nor vice versa — pin it,
    // because a naive substitution order would corrupt one of them.
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "Two Tokens".to_string());

    let result = render_path("{{slug_or_time}}{{slug}}.md", &fields, None, slug_now());

    assert_eq!(result, "two-tokens-two-tokens.md");
}
