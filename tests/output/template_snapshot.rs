/// Pinned-behavior snapshot tests for `render_path` and `render_append_template`.
///
/// These tests use a fixed timestamp (2025-01-15 10:30:00 local) so that
/// every strftime assertion is deterministic.  Their purpose is NOT to test
/// new logic but to lock the **current** behavior so that Slice 4's
/// unification can be verified against these pins.
///
/// Where the two functions diverge the comment begins:
///   // BEHAVIOR: render_path …; render_append does …
use chrono::{Local, TimeZone};
use pour::config::Config;
use pour::output::CompositeData;
use pour::output::template::{render_append_template, render_path};
use std::collections::HashMap;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Fixed local timestamp: 2025-01-15 10:30:00.
/// Using `unwrap()` is safe for a compile-time-known valid date.
fn fixed_now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap()
}

fn no_fields() -> HashMap<String, String> {
    HashMap::new()
}

fn no_composites() -> CompositeData {
    CompositeData::new()
}

fn no_overrides() -> HashMap<String, String> {
    HashMap::new()
}

/// Minimal append-mode module with a single `text` field named "body".
fn dummy_module() -> pour::config::ModuleConfig {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.snap]
mode = "append"
path = "snap.md"
append_under_header = "## Log"

[[modules.snap.fields]]
name = "body"
field_type = "text"
prompt = "Body"
"####;
    Config::from_toml(toml)
        .unwrap()
        .modules
        .into_values()
        .next()
        .unwrap()
}

/// Module that declares "title" and "body" as text fields.
fn two_field_module() -> pour::config::ModuleConfig {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.snap]
mode = "append"
path = "snap.md"
append_under_header = "## Log"

[[modules.snap.fields]]
name = "title"
field_type = "text"
prompt = "Title"

[[modules.snap.fields]]
name = "body"
field_type = "text"
prompt = "Body"
"####;
    Config::from_toml(toml)
        .unwrap()
        .modules
        .into_values()
        .next()
        .unwrap()
}

// ── render_path snapshot tests ────────────────────────────────────────────────

/// 1. Plain string with no placeholders — passthrough.
#[test]
fn snap_render_path_plain_passthrough() {
    let result = render_path("static/notes.md", &no_fields(), None, fixed_now());
    assert_eq!(result, "static/notes.md");
}

/// 2. Single `{{key}}` substitution.
#[test]
fn snap_render_path_single_substitution() {
    let mut fields = HashMap::new();
    fields.insert("bean".to_string(), "Ethiopian".to_string());
    let result = render_path("Coffee/{{bean}}.md", &fields, None, fixed_now());
    assert_eq!(result, "Coffee/Ethiopian.md");
}

/// 3. Multiple `{{key}}` substitutions in one template.
#[test]
fn snap_render_path_multiple_substitutions() {
    let mut fields = HashMap::new();
    fields.insert("year".to_string(), "2025".to_string());
    fields.insert("tag".to_string(), "espresso".to_string());
    let result = render_path("Log/{{year}}/{{tag}}.md", &fields, None, fixed_now());
    assert_eq!(result, "Log/2025/espresso.md");
}

/// 4. `{{key}}` where key is missing from vars — placeholder is **stripped**.
///
/// BEHAVIOR: render_path strips unknown placeholders (leaves empty string).
///           render_append does NOT strip — unknown placeholders are left as-is.
#[test]
fn snap_render_path_missing_key_is_stripped() {
    let result = render_path("Coffee/{{unknown}}.md", &no_fields(), None, fixed_now());
    // The placeholder is removed entirely, leaving an empty filename segment.
    assert_eq!(result, "Coffee/.md");
}

/// 5a. Strftime `%Y/%m/%d` codes expand correctly.
#[test]
fn snap_render_path_strftime_expands() {
    let result = render_path("Journal/%Y/%m/%d.md", &no_fields(), None, fixed_now());
    assert_eq!(result, "Journal/2025/01/15.md");
}

/// 5b. Strftime codes and field placeholders coexist in one template.
#[test]
fn snap_render_path_strftime_and_field_together() {
    let mut fields = HashMap::new();
    fields.insert("bean".to_string(), "Kenyan".to_string());
    let result = render_path("Coffee/%Y%m%d-{{bean}}.md", &fields, None, fixed_now());
    assert_eq!(result, "Coffee/20250115-Kenyan.md");
}

/// 5c. `{{date}}` with explicit format uses that format.
///
/// BEHAVIOR: render_path `{{date}}` respects the `date_format` parameter
///           (defaults to `%Y%m%d` with no hyphens).
///           render_append `{{date}}` is always `%Y-%m-%d` (with hyphens),
///           ignoring any external format parameter.
#[test]
fn snap_render_path_date_token_default_format() {
    // Default date_format is %Y%m%d (no hyphens).
    let result = render_path("Daily/{{date}}.md", &no_fields(), None, fixed_now());
    assert_eq!(result, "Daily/20250115.md");
}

#[test]
fn snap_render_path_date_token_explicit_format() {
    let result = render_path(
        "Daily/{{date}}.md",
        &no_fields(),
        Some("%Y-%m-%d"),
        fixed_now(),
    );
    assert_eq!(result, "Daily/2025-01-15.md");
}

/// 5d. `{{time}}` token.
#[test]
fn snap_render_path_time_token() {
    let result = render_path("Log/{{time}}.md", &no_fields(), None, fixed_now());
    // Time contains a colon; sanitize_path_filename replaces `:` with `-`.
    // BEHAVIOR: render_path sanitizes the filename component (colon → dash).
    //           render_append does NOT sanitize.
    assert_eq!(result, "Log/10-30.md");
}

/// 6a. Nested-looking `{{{{key}}}}` — outer `{{` and `}}` consumed first.
/// With input `{{{{key}}}}` the strftime pass is a no-op, then field
/// substitution matches `{{key}}` (the inner pair) and replaces it; the outer
/// `{{` and `}}` then form a new `{{}}` which the strip loop removes.
#[test]
fn snap_render_path_double_braced_outer_stripped() {
    let mut fields = HashMap::new();
    fields.insert("key".to_string(), "val".to_string());
    let result = render_path("{{{{key}}}}", &fields, None, fixed_now());
    // Inner {{key}} → "val", leaving "{{val}}" which gets stripped to "val"
    // because the strip loop sees "{{" and finds "}}" and removes the range.
    // Wait — after replacing {{key}} with "val", we have "{{val}}" which is
    // not a declared field; the strip loop removes it → "".
    assert_eq!(result, "");
}

/// 6b. `{{}}` empty placeholder — stripped to empty string.
#[test]
fn snap_render_path_empty_placeholder_stripped() {
    let result = render_path("prefix-{{}}-suffix", &no_fields(), None, fixed_now());
    // Strip loop removes `{{}}` → "prefix--suffix", then sanitize_path_filename
    // collapses consecutive dashes → "prefix-suffix".
    assert_eq!(result, "prefix-suffix");
}

/// 7. Special chars in value — spaces pass through; path separators pass
///    through in dir components; colons in filename are sanitized to dashes.
#[test]
fn snap_render_path_spaces_in_value() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "My Note Title".to_string());
    let result = render_path("Notes/{{title}}.md", &fields, None, fixed_now());
    assert_eq!(result, "Notes/My Note Title.md");
}

#[test]
fn snap_render_path_unicode_in_value() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "Café Résumé".to_string());
    let result = render_path("Notes/{{title}}.md", &fields, None, fixed_now());
    assert_eq!(result, "Notes/Café Résumé.md");
}

/// Colon in value — sanitized to dash in filename.
///
/// BEHAVIOR: render_path calls sanitize_path_filename on the result, so
///           characters illegal on Windows (`:`, `?`, `*`, `<`, `>`, `|`, `"`)
///           in the filename component become `-`.
///           render_append does NOT sanitize — caller receives the raw value.
#[test]
fn snap_render_path_colon_in_value_sanitized() {
    let mut fields = HashMap::new();
    fields.insert("ts".to_string(), "10:30:45".to_string());
    let result = render_path("Log/{{ts}}.md", &fields, None, fixed_now());
    // "10:30:45" → "10-30-45" after sanitization
    assert_eq!(result, "Log/10-30-45.md");
}

/// Backslash in template normalized to forward slash.
#[test]
fn snap_render_path_backslash_normalized() {
    let result = render_path(r"Notes\subdir\file.md", &no_fields(), None, fixed_now());
    assert_eq!(result, "Notes/subdir/file.md");
}

// ── render_append_template snapshot tests ────────────────────────────────────

/// 1. Plain string with no placeholders — passthrough.
#[test]
fn snap_render_append_plain_passthrough() {
    let m = dummy_module();
    let result = render_append_template(
        "static content",
        &no_fields(),
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "static content");
}

/// 2. Single `{{key}}` substitution.
#[test]
fn snap_render_append_single_substitution() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Hello world".to_string());
    let m = dummy_module();
    let result = render_append_template(
        "Note: {{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "Note: Hello world");
}

/// 3. Multiple `{{key}}` substitutions in one template.
#[test]
fn snap_render_append_multiple_substitutions() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "Stand-up".to_string());
    fields.insert("body".to_string(), "Done, doing, blocked.".to_string());
    let m = two_field_module();
    let result = render_append_template(
        "## {{title}}\n{{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "## Stand-up\nDone, doing, blocked.");
}

/// 4. `{{key}}` where key is missing from fields — left **as-is**.
///
/// BEHAVIOR: render_path strips unknown placeholders.
///           render_append leaves unknown placeholders untouched.
#[test]
fn snap_render_append_missing_key_left_as_is() {
    let m = dummy_module();
    let result = render_append_template(
        "Value: {{ghost}}",
        &no_fields(),
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "Value: {{ghost}}");
}

/// 5a. Strftime codes expand.
#[test]
fn snap_render_append_strftime_expands() {
    let m = dummy_module();
    let result = render_append_template(
        "Year: %Y Month: %m Day: %d",
        &no_fields(),
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "Year: 2025 Month: 01 Day: 15");
}

/// 5b. `{{date}}` always uses `%Y-%m-%d` (with hyphens), regardless of caller.
///
/// BEHAVIOR: render_path `{{date}}` uses the `date_format` param (default `%Y%m%d`).
///           render_append `{{date}}` is always hardcoded to `%Y-%m-%d`.
#[test]
fn snap_render_append_date_token_always_hyphenated() {
    let m = dummy_module();
    let result = render_append_template(
        "Date: {{date}}",
        &no_fields(),
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "Date: 2025-01-15");
}

/// 5c. `{{time}}` token.
///
/// BEHAVIOR: render_append does NOT sanitize — colon is preserved literally.
///           render_path sanitizes colons to dashes.
#[test]
fn snap_render_append_time_token_colon_preserved() {
    let m = dummy_module();
    let result = render_append_template(
        "Time: {{time}}",
        &no_fields(),
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    // Colon is NOT sanitized in append output.
    assert_eq!(result, "Time: 10:30");
}

/// 6a. Nested-looking `{{{{key}}}}` — same inner substitution behavior.
#[test]
fn snap_render_append_double_braced_outer_left_as_is() {
    let mut fields = HashMap::new();
    fields.insert("key".to_string(), "val".to_string());
    // dummy_module declares "body" not "key", so "key" is undeclared — substituted normally.
    let m = dummy_module();
    let result = render_append_template(
        "{{{{key}}}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    // Inner {{key}} → "val", leaving "{{val}}". "val" is not in fields, so
    // it is left as-is: "{{val}}".
    assert_eq!(result, "{{val}}");
}

/// 6b. `{{}}` empty placeholder — left as-is (no strip loop in render_append).
///
/// BEHAVIOR: render_path strips remaining `{{...}}` patterns after substitution.
///           render_append has NO strip loop — unresolved placeholders stay.
#[test]
fn snap_render_append_empty_placeholder_left_as_is() {
    let m = dummy_module();
    let result = render_append_template(
        "prefix-{{}}-suffix",
        &no_fields(),
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    // No stripping — stays literally.
    assert_eq!(result, "prefix-{{}}-suffix");
}

/// 7. Special chars in value — colon preserved (no sanitization).
#[test]
fn snap_render_append_colon_in_value_preserved() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "10:30:45".to_string());
    let m = dummy_module();
    let result = render_append_template(
        "Time: {{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "Time: 10:30:45");
}

/// 7b. Unicode in value — preserved.
#[test]
fn snap_render_append_unicode_in_value() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Café ☕".to_string());
    let m = dummy_module();
    let result = render_append_template(
        "Note: {{body}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "Note: Café ☕");
}

/// 8. Multi-line template.
#[test]
fn snap_render_append_multiline_template() {
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), "Morning".to_string());
    fields.insert("body".to_string(), "Rested well.".to_string());
    let m = two_field_module();
    let result = render_append_template(
        "#### {{time}} — {{title}}\n\n{{body}}\n\n---",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "#### 10:30 — Morning\n\nRested well.\n\n---");
}

/// 8b. Multi-line field value in template.
#[test]
fn snap_render_append_multiline_field_value() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Line 1\nLine 2\nLine 3".to_string());
    let m = dummy_module();
    let result = render_append_template(
        "---\n{{body}}\n---",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "---\nLine 1\nLine 2\nLine 3\n---");
}

/// 9. Percent sign in field value — not re-processed as strftime.
///
/// Same protection in both functions: strftime runs first on the raw template,
/// then field values (containing `%`) are injected — so they are never seen
/// by chrono.
#[test]
fn snap_render_append_percent_in_value_is_literal() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Improved 50% this week".to_string());
    let m = dummy_module();
    let result = render_append_template(
        "Update: {{body}} on %Y-%m-%d",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    assert_eq!(result, "Update: Improved 50% this week on 2025-01-15");
}

/// 10. Declared field that is hidden (show_when not met) → resolves to empty.
///
/// BEHAVIOR: render_append clears placeholders for declared-but-hidden fields.
///           render_path has no visibility concept — it always substitutes or strips.
#[test]
fn snap_render_append_declared_hidden_field_is_empty() {
    let toml = r####"
[vault]
base_path = "/tmp"

[modules.snap]
mode = "append"
path = "snap.md"
append_under_header = "## Log"

[[modules.snap.fields]]
name = "drink_type"
field_type = "static_select"
prompt = "Type"
options = ["coffee", "tea"]

[[modules.snap.fields]]
name = "notes"
field_type = "text"
prompt = "Notes"
show_when = { field = "drink_type", equals = "coffee" }
"####;
    let m = Config::from_toml(toml)
        .unwrap()
        .modules
        .into_values()
        .next()
        .unwrap();

    let mut fields = HashMap::new();
    fields.insert("drink_type".to_string(), "tea".to_string());
    fields.insert("notes".to_string(), "Tasty brew".to_string());

    let result = render_append_template(
        "{{drink_type}} | {{notes}}",
        &fields,
        &m,
        &no_composites(),
        &no_overrides(),
        &no_overrides(),
        fixed_now(),
    );
    // notes is hidden because drink_type != "coffee", so its placeholder becomes "".
    assert_eq!(result, "tea | ");
}
