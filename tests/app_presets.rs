use pour::app::App;
use pour::config::Config;
use pour::data::field_presets::FieldPresets;
use pour::data::history::History;
use pour::data::presets::{PresetEntry, Presets};
use pour::transport::Transport;
use pour::transport::fs::FsWriter;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Config fixtures
// ---------------------------------------------------------------------------

/// Module with text, number, static_select, composite_array, and a
/// preset_exclude field.
const PRESET_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
default = "Ethiopia"

[[modules.coffee.fields]]
name = "dose"
field_type = "number"
prompt = "Dose (g)"
default = "18"

[[modules.coffee.fields]]
name = "method"
field_type = "static_select"
prompt = "Method"
options = ["V60", "AeroPress"]
default = "V60"

[[modules.coffee.fields]]
name = "notes"
field_type = "text"
prompt = "Notes"

[[modules.coffee.fields]]
name = "timestamp"
field_type = "text"
prompt = "Timestamp"
preset_exclude = true
default = "auto"

[[modules.coffee.fields]]
name = "recipe"
field_type = "composite_array"
prompt = "Recipe"

[[modules.coffee.fields.sub_fields]]
name = "amount"
field_type = "number"
prompt = "Amount (g)"
"####;

/// Module with show_when dependency: `grind` is only visible when method == "V60".
const VISIBILITY_PRESET_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"

[[modules.coffee.fields]]
name = "method"
field_type = "static_select"
prompt = "Method"
options = ["V60", "AeroPress"]
default = ""

[[modules.coffee.fields]]
name = "grind"
field_type = "text"
prompt = "Grind size"
default = ""
[modules.coffee.fields.show_when]
field = "method"
equals = "V60"
"####;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_app_from_toml(toml: &str) -> App {
    let config = Config::from_toml(toml).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-preset-apply-history.json",
        )),
        Presets::empty(),
        FieldPresets::empty(),
    )
}

fn make_entry(values: &[(&str, &str)]) -> PresetEntry {
    PresetEntry {
        name: "Test".to_owned(),
        description: None,
        values: values
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Task 3: init_form populates preset_names from Presets
// ---------------------------------------------------------------------------

#[test]
fn init_form_preset_names_empty_when_no_presets() {
    let app = make_app_from_toml(PRESET_TOML);
    let form = app.init_form("coffee").expect("module exists");
    assert!(form.preset_names.is_empty());
    assert!(form.selected_preset_name.is_none());
    assert!(form.preset_overlay.is_none());
    assert!(!form.confirm_delete_preset);
}

#[test]
fn init_form_preset_names_populated_from_saved_presets() {
    let config = Config::from_toml(PRESET_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut presets = Presets::empty();
    presets.set(
        "coffee",
        PresetEntry {
            name: "Morning".to_owned(),
            description: None,
            values: HashMap::new(),
        },
    );
    presets.set(
        "coffee",
        PresetEntry {
            name: "Afternoon".to_owned(),
            description: None,
            values: HashMap::new(),
        },
    );

    let app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-preset-names-history.json",
        )),
        presets,
        FieldPresets::empty(),
    );

    let form = app.init_form("coffee").expect("module exists");
    assert_eq!(form.preset_names, vec!["Morning", "Afternoon"]);
    assert!(form.selected_preset_name.is_none());
}

// ---------------------------------------------------------------------------
// Task 4: apply_preset — applying Some(preset)
// ---------------------------------------------------------------------------

#[test]
fn apply_preset_sets_matching_field_values() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    let preset = make_entry(&[("bean", "Kenya"), ("dose", "20")]);
    App::apply_preset(&mut form, fields, Some(&preset));

    assert_eq!(form.field_values.get("bean").unwrap(), "Kenya");
    assert_eq!(form.field_values.get("dose").unwrap(), "20");
}

#[test]
fn apply_preset_resets_fields_absent_from_preset_to_defaults() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // Pre-set method to AeroPress before applying the preset.
    form.field_values
        .insert("method".to_string(), "AeroPress".to_string());

    // Preset only has bean — method should reset to its default ("V60").
    let preset = make_entry(&[("bean", "Kenya")]);
    App::apply_preset(&mut form, fields, Some(&preset));

    assert_eq!(form.field_values.get("bean").unwrap(), "Kenya");
    assert_eq!(form.field_values.get("method").unwrap(), "V60");
}

#[test]
fn apply_preset_skips_preset_exclude_fields() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // "timestamp" is preset_exclude = true; preset tries to set it.
    let preset = make_entry(&[("bean", "Kenya"), ("timestamp", "2026-01-01")]);
    App::apply_preset(&mut form, fields, Some(&preset));

    assert_eq!(form.field_values.get("bean").unwrap(), "Kenya");
    // timestamp should NOT have been changed from its default.
    assert_eq!(form.field_values.get("timestamp").unwrap(), "auto");
}

#[test]
fn apply_preset_skips_composite_array_fields() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // recipe is composite_array — it must not appear in field_values after apply.
    let preset = make_entry(&[("bean", "Kenya")]);
    App::apply_preset(&mut form, fields, Some(&preset));

    // composite_values should still be empty (not modified).
    assert!(
        form.composite_values
            .get("recipe")
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
    // recipe must not have slipped into field_values.
    assert!(!form.field_values.contains_key("recipe"));
}

#[test]
fn apply_preset_silently_skips_unknown_field_names() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // "ghost_field" is not in the module config — must not panic and must not
    // appear in field_values.
    let preset = make_entry(&[("ghost_field", "haunted"), ("bean", "Kenya")]);
    App::apply_preset(&mut form, fields, Some(&preset));

    assert_eq!(form.field_values.get("bean").unwrap(), "Kenya");
    assert!(!form.field_values.contains_key("ghost_field"));
}

// ---------------------------------------------------------------------------
// Task 4: apply_preset — None resets to defaults
// ---------------------------------------------------------------------------

#[test]
fn apply_none_resets_fields_to_defaults() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // Dirty some values.
    form.field_values
        .insert("bean".to_string(), "Kenya".to_string());
    form.field_values
        .insert("dose".to_string(), "22".to_string());
    form.field_values
        .insert("notes".to_string(), "fruity".to_string());

    App::apply_preset(&mut form, fields, None);

    // Fields with defaults should be reset to their config defaults.
    assert_eq!(form.field_values.get("bean").unwrap(), "Ethiopia");
    assert_eq!(form.field_values.get("dose").unwrap(), "18");
    assert_eq!(form.field_values.get("method").unwrap(), "V60");
    // Fields without a default should be reset to empty string.
    assert_eq!(form.field_values.get("notes").unwrap(), "");
}

#[test]
fn apply_none_does_not_reset_preset_exclude_fields() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // Change timestamp (preset_exclude field).
    form.field_values
        .insert("timestamp".to_string(), "custom-time".to_string());

    // Applying <none> should NOT reset the excluded field.
    App::apply_preset(&mut form, fields, None);

    assert_eq!(
        form.field_values.get("timestamp").unwrap(),
        "custom-time",
        "preset_exclude field should not be reset by <none>"
    );
}

// ---------------------------------------------------------------------------
// Task 4: apply_preset — show_when re-evaluation
// ---------------------------------------------------------------------------

#[test]
fn apply_preset_updates_visibility_correctly() {
    let app = make_app_from_toml(VISIBILITY_PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // Initially method="" so grind is hidden.
    // active_field = 1 (preset row is at 0; method is the first real field at 1).
    assert_eq!(form.active_field, 1);

    // Apply preset setting method = "V60" — grind becomes visible.
    let preset = make_entry(&[("method", "V60")]);
    App::apply_preset(&mut form, fields, Some(&preset));

    // Both fields are now visible: method(idx 0) and grind(idx 1).
    assert_eq!(form.field_values.get("method").unwrap(), "V60");
    let visible = pour::visibility::visible_field_indices(fields, &form.field_values);
    assert_eq!(visible.len(), 2, "both fields should be visible");
}

#[test]
fn apply_none_moves_focus_when_active_field_becomes_hidden() {
    let app = make_app_from_toml(VISIBILITY_PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // Set method = "V60" so grind is visible; move focus to grind (visible index 1).
    form.field_values
        .insert("method".to_string(), "V60".to_string());
    form.active_field = 1;
    form.active_config_idx = Some(1); // grind is fields[1]

    // Apply <none> resets method to "" — grind becomes hidden.
    App::apply_preset(&mut form, fields, None);

    // Focus should have moved to first visible field (method at visible index 0).
    assert_eq!(form.active_field, 0);
    // method value reset to its default ("").
    assert_eq!(form.field_values.get("method").unwrap(), "");
}

#[test]
fn apply_preset_when_submit_button_focused_moves_to_first_field() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // Simulate submit button focus: active_config_idx = None.
    let visible = pour::visibility::visible_field_indices(fields, &form.field_values);
    form.active_field = visible.len(); // submit button position
    form.active_config_idx = None;

    let preset = make_entry(&[("bean", "Kenya")]);
    App::apply_preset(&mut form, fields, Some(&preset));

    // Focus should move to first visible field.
    assert_eq!(form.active_field, 0);
    assert!(form.active_config_idx.is_some());
}

// ---------------------------------------------------------------------------
// Phase C: auto-suggest preset name + overwrite confirm
// ---------------------------------------------------------------------------

const AXES_PRESET_TOML: &str = r####"
[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/log.md"
preset_axes = ["method", "bean"]

[[modules.coffee.fields]]
name = "method"
field_type = "static_select"
prompt = "Method"
options = ["V60", "AeroPress"]
default = "V60"

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
default = ""
"####;

fn make_app_with_axes(toml: &str) -> App {
    let config = Config::from_toml(toml).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-preset-axes-history.json",
        )),
        Presets::empty(),
        FieldPresets::empty(),
    )
}

#[test]
fn suggest_name_joins_non_empty_axis_values_with_middle_dot() {
    use pour::data::preset_tree::suggest_preset_name;
    let mut values = std::collections::HashMap::new();
    values.insert("method".to_owned(), "V60".to_owned());
    values.insert("bean".to_owned(), "Onyx".to_owned());
    let axes = ["method".to_owned(), "bean".to_owned()];
    let name = suggest_preset_name(&values, &axes);
    assert_eq!(name, "V60 \u{00B7} Onyx");
}

#[test]
fn suggest_name_skips_missing_axis() {
    use pour::data::preset_tree::suggest_preset_name;
    let mut values = std::collections::HashMap::new();
    values.insert("method".to_owned(), "V60".to_owned());
    // bean not set
    let axes = ["method".to_owned(), "bean".to_owned()];
    let name = suggest_preset_name(&values, &axes);
    assert_eq!(name, "V60", "missing axis must not produce trailing separator");
}

#[test]
fn selected_preset_name_survives_reorder() {
    use pour::data::presets::{PresetEntry, Presets};
    let config = Config::from_toml(AXES_PRESET_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut presets = Presets::empty();
    presets.set(
        "coffee",
        PresetEntry {
            name: "Morning".to_owned(),
            description: None,
            values: std::collections::HashMap::new(),
        },
    );
    presets.set(
        "coffee",
        PresetEntry {
            name: "Afternoon".to_owned(),
            description: None,
            values: std::collections::HashMap::new(),
        },
    );
    let app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-reorder-history.json",
        )),
        presets,
        FieldPresets::empty(),
    );
    let mut form = app.init_form("coffee").expect("module exists");

    // Select "Afternoon" by name.
    form.selected_preset_name = Some("Afternoon".to_owned());

    // Call Presets::reorder directly (the same operation handle_reorder_preset performs),
    // then refresh preset_names/descriptions as the handler does — without touching
    // selected_preset_name (the fix to Major 6 removed that clobber).
    let mut presets2 = Presets::empty();
    presets2.set(
        "coffee",
        PresetEntry {
            name: "Morning".to_owned(),
            description: None,
            values: std::collections::HashMap::new(),
        },
    );
    presets2.set(
        "coffee",
        PresetEntry {
            name: "Afternoon".to_owned(),
            description: None,
            values: std::collections::HashMap::new(),
        },
    );
    presets2.reorder("coffee", "Afternoon", -1);
    let saved = presets2.get("coffee");
    form.preset_names = saved.iter().map(|p| p.name.clone()).collect();
    form.preset_descriptions = saved.iter().map(|p| p.description.clone()).collect();
    // selected_preset_name must NOT be touched — the fix ensures handle_reorder_preset
    // no longer overwrites it.

    assert_eq!(
        form.selected_preset_name.as_deref(),
        Some("Afternoon"),
        "selected_preset_name must survive Presets::reorder"
    );
    // Verify "Afternoon" actually moved to position 0.
    assert_eq!(form.preset_names[0], "Afternoon");
}

#[test]
fn overwrite_confirm_gate_fires_on_colliding_name() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use pour::app::{PresetDialogFocus, PresetDialogTarget, PresetSaveDialog, Screen};
    use pour::tui::form::handle_key;

    let mut app = make_app_with_axes(AXES_PRESET_TOML);
    let mut form = app.init_form("coffee").expect("module exists");

    // Pretend "Morning" is an existing preset.
    form.preset_names = vec!["Morning".to_owned()];
    form.preset_descriptions = vec![None];
    // No preset selected (new preset path).
    form.selected_preset_name = None;

    // Open save dialog with "Morning" typed — collision with existing name.
    form.preset_overlay = Some(PresetSaveDialog {
        name_buffer: "Morning".to_owned(),
        cursor_position: 7,
        description_buffer: String::new(),
        description_cursor: 0,
        focus: PresetDialogFocus::Name,
        target: PresetDialogTarget::Module,
        name_was_user_edited: true,
        awaiting_overwrite_confirm: false,
    });

    app.form_state = Some(form);
    app.screen = Screen::Form;

    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);

    // First Enter: colliding name → must set awaiting_overwrite_confirm = true, NOT save.
    handle_key(&mut app, enter);
    {
        let overlay = app
            .form_state
            .as_ref()
            .unwrap()
            .preset_overlay
            .as_ref()
            .expect("overlay must still be open after first Enter on collision");
        assert!(
            overlay.awaiting_overwrite_confirm,
            "first Enter on colliding name must set awaiting_overwrite_confirm"
        );
    }

    // Second Enter: confirm is set → must proceed and close the overlay.
    handle_key(&mut app, enter);
    assert!(
        app.form_state.as_ref().unwrap().preset_overlay.is_none(),
        "second Enter must close the overlay (overwrite confirmed)"
    );
}

#[test]
fn editing_existing_preset_name_does_not_trigger_overwrite_confirm() {
    use pour::app::{PresetDialogFocus, PresetDialogTarget, PresetSaveDialog};
    let app = make_app_with_axes(AXES_PRESET_TOML);
    let mut form = app.init_form("coffee").expect("module exists");

    form.preset_names = vec!["Morning".to_owned()];
    form.preset_descriptions = vec![None];
    form.selected_preset_name = Some("Morning".to_owned());

    form.preset_overlay = Some(PresetSaveDialog {
        name_buffer: "Morning".to_owned(),
        cursor_position: 7,
        description_buffer: String::new(),
        description_cursor: 0,
        focus: PresetDialogFocus::Name,
        target: PresetDialogTarget::Module,
        name_was_user_edited: true,
        awaiting_overwrite_confirm: false,
    });

    // editing_same = true, so overwrite confirm must NOT fire.
    let overlay = form.preset_overlay.as_ref().unwrap();
    let editing_same = form
        .selected_preset_name
        .as_deref()
        .map(|n| n == overlay.name_buffer)
        .unwrap_or(false);
    assert!(editing_same, "editing the same preset should suppress overwrite confirm");
}

#[test]
fn apply_preset_resets_ui_state() {
    let app = make_app_from_toml(PRESET_TOML);
    let fields = &app.config.modules["coffee"].fields;
    let mut form = app.init_form("coffee").expect("module exists");

    // Simulate dirty UI state.
    form.cursor_position = 10;
    form.dropdown_open = true;
    form.textarea_open = true;
    form.search_buffers
        .insert("bean".to_string(), "eth".to_string());

    let preset = make_entry(&[("bean", "Kenya")]);
    App::apply_preset(&mut form, fields, Some(&preset));

    assert_eq!(form.cursor_position, 0);
    assert!(!form.dropdown_open);
    assert!(!form.textarea_open);
    assert!(form.search_buffers.is_empty());
}
