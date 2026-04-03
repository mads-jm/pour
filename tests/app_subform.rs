use pour::app::SubFormState;
use pour::config::{TemplateConfig, TemplateFieldConfig, TemplateFieldType};

fn make_template() -> TemplateConfig {
    TemplateConfig {
        path: "Coffee/Beans/{{name}}.md".to_string(),
        fields: vec![
            TemplateFieldConfig {
                name: "origin".to_string(),
                field_type: TemplateFieldType::Text,
                prompt: "Origin".to_string(),
                options: None,
                default: Some("Unknown".to_string()),
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
                default: None,
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
fn sub_form_new_populates_defaults() {
    let template = make_template();
    let state = SubFormState::new(
        "bean_template".to_string(),
        "Ethiopia Guji".to_string(),
        "bean".to_string(),
        &template,
    );

    assert_eq!(state.field_values["origin"], "Unknown");
}

#[test]
fn sub_form_new_no_default_gives_empty_string() {
    let template = make_template();
    let state = SubFormState::new(
        "bean_template".to_string(),
        "Test".to_string(),
        "bean".to_string(),
        &template,
    );

    assert_eq!(state.field_values["rating"], "");
    assert_eq!(state.field_values["roast"], "");
}

#[test]
fn sub_form_new_populates_static_select_options() {
    let template = make_template();
    let state = SubFormState::new(
        "bean_template".to_string(),
        "Test".to_string(),
        "bean".to_string(),
        &template,
    );

    let roast_opts = &state.field_options["roast"];
    assert_eq!(roast_opts, &vec!["Light", "Medium", "Dark"]);
    // text and number fields should NOT have options
    assert!(!state.field_options.contains_key("origin"));
    assert!(!state.field_options.contains_key("rating"));
}

#[test]
fn sub_form_new_starts_at_field_zero() {
    let template = make_template();
    let state = SubFormState::new(
        "bean_template".to_string(),
        "Test".to_string(),
        "bean".to_string(),
        &template,
    );

    assert_eq!(state.active_field, 0);
    assert_eq!(state.cursor_position, 0);
    assert!(!state.dropdown_open);
}

#[test]
fn sub_form_preserves_note_name_and_parent() {
    let template = make_template();
    let state = SubFormState::new(
        "bean_template".to_string(),
        "Ethiopia Guji".to_string(),
        "bean".to_string(),
        &template,
    );

    assert_eq!(state.note_name, "Ethiopia Guji");
    assert_eq!(state.parent_field_name, "bean");
    assert_eq!(state.template_name, "bean_template");
}
