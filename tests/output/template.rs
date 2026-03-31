use chrono::Local;
use pour::output::template::{render_append_template, render_path};
use std::collections::HashMap;

#[test]
fn render_path_substitutes_date_tokens() {
    let result = render_path("Journal/%Y/%Y-%m-%d.md");
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
    let result = render_path("static/path.md");
    assert_eq!(result, "static/path.md");
}

#[test]
fn render_append_template_replaces_fields() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Hello world".to_string());
    fields.insert("mood".to_string(), "happy".to_string());

    let result = render_append_template("Mood: {{mood}} | {{body}}", &fields);
    assert_eq!(result, "Mood: happy | Hello world");
}

#[test]
fn render_append_template_special_time_token() {
    let fields = HashMap::new();
    let result = render_append_template("> [!note] {{time}}", &fields);
    let now = Local::now().format("%H:%M").to_string();
    assert!(
        result.contains(&now),
        "should contain current time, got: {result}"
    );
}

#[test]
fn render_append_template_special_date_token() {
    let fields = HashMap::new();
    let result = render_append_template("Date: {{date}}", &fields);
    let today = Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(result, format!("Date: {today}"));
}

#[test]
fn render_append_template_missing_field_left_as_is() {
    let fields = HashMap::new();
    let result = render_append_template("Value: {{unknown}}", &fields);
    assert_eq!(result, "Value: {{unknown}}");
}

#[test]
fn render_append_template_mixed_known_and_unknown() {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), "Alice".to_string());

    let result = render_append_template("{{name}} said {{quote}}", &fields);
    assert_eq!(result, "Alice said {{quote}}");
}

#[test]
fn render_append_template_realistic_journal() {
    let mut fields = HashMap::new();
    fields.insert("body".to_string(), "Felt productive today.".to_string());

    let template = "> [!note] {{time}}\n> {{body}}";
    let result = render_append_template(template, &fields);

    let now = Local::now().format("%H:%M").to_string();
    assert!(result.contains(&now), "should have time");
    assert!(
        result.contains("Felt productive today."),
        "should have body"
    );
}
