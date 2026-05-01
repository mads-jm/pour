use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pour::app::{App, PresetDialogTarget};
use pour::config::Config;
use pour::data::field_presets::{FieldPresetEntry, FieldPresets};
use pour::data::history::History;
use pour::data::presets::Presets;
use pour::transport::Transport;
use pour::transport::fs::FsWriter;
use pour::tui::form::{FormAction, handle_key};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const FIELD_PRESET_TOML: &str = r####"
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
name = "recipe"
field_type = "composite_array"
prompt = "Recipe"

[[modules.coffee.fields.sub_fields]]
name = "stage"
field_type = "static_select"
prompt = "Stage"
options = ["Bloom", "First Pour", "Draw Down"]

[[modules.coffee.fields.sub_fields]]
name = "weight_g"
field_type = "number"
prompt = "Weight (g)"

[[modules.coffee.fields.sub_fields]]
name = "duration_s"
field_type = "number"
prompt = "Duration (s)"
"####;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

fn make_app() -> App {
    let config = Config::from_toml(FIELD_PRESET_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut app = App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-field-preset-history.json",
        )),
        Presets::empty(),
        FieldPresets::empty(),
    );
    app.selected_module = app.module_keys.iter().position(|k| k == "coffee").unwrap();
    app.form_state = app.init_form("coffee");
    app.screen = pour::app::Screen::Form;
    app
}

/// Open the composite overlay on `recipe` and seed it with the given rows.
/// `recipe` is at active_field index 2 (preset row 0, bean 1, recipe 2).
fn open_composite_with_rows(app: &mut App, rows: Vec<Vec<String>>) {
    let fs = app.form_state.as_mut().unwrap();
    fs.active_field = 2;
    fs.composite_open = true;
    fs.composite_values.insert("recipe".to_string(), rows);
}

fn rows_3(triples: &[(&str, &str, &str)]) -> Vec<Vec<String>> {
    triples
        .iter()
        .map(|(a, b, c)| vec![a.to_string(), b.to_string(), c.to_string()])
        .collect()
}

// ---------------------------------------------------------------------------
// Save flow
// ---------------------------------------------------------------------------

#[test]
fn s_in_composite_overlay_opens_save_dialog_with_composite_target() {
    let mut app = make_app();
    open_composite_with_rows(
        &mut app,
        rows_3(&[("Bloom", "60", "30"), ("First Pour", "120", "30")]),
    );

    handle_key(&mut app, key(KeyCode::Char('s')));

    let fs = app.form_state.as_ref().unwrap();
    let dialog = fs.preset_overlay.as_ref().expect("dialog opens");
    match &dialog.target {
        PresetDialogTarget::CompositeField { field_name } => {
            assert_eq!(field_name, "recipe");
        }
        PresetDialogTarget::Module => panic!("wrong target — expected CompositeField"),
    }
}

#[test]
fn s_with_no_rows_shows_status_and_no_dialog() {
    let mut app = make_app();
    open_composite_with_rows(&mut app, Vec::new());

    handle_key(&mut app, key(KeyCode::Char('s')));

    let fs = app.form_state.as_ref().unwrap();
    assert!(fs.preset_overlay.is_none());
    assert_eq!(
        fs.composite_status.as_deref(),
        Some("nothing to save"),
        "status should explain why no dialog"
    );
}

#[test]
fn s_with_only_empty_rows_shows_status_and_no_dialog() {
    let mut app = make_app();
    open_composite_with_rows(&mut app, vec![vec![String::new(); 3]; 2]);

    handle_key(&mut app, key(KeyCode::Char('s')));

    let fs = app.form_state.as_ref().unwrap();
    assert!(fs.preset_overlay.is_none());
    assert_eq!(fs.composite_status.as_deref(), Some("nothing to save"));
}

#[test]
fn save_field_preset_persists_rows_and_sets_subtitle() {
    let mut app = make_app();
    let rows = rows_3(&[("Bloom", "60", "30"), ("First Pour", "120", "30")]);
    open_composite_with_rows(&mut app, rows.clone());

    let _ = app.save_field_preset("recipe", "Hoffmann 4:6", None, rows.clone());

    // Stored under the "coffee.recipe" key.
    let entries = app.field_presets.get("coffee.recipe");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "Hoffmann 4:6");
    assert_eq!(entries[0].rows, rows);

    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(
        fs.last_applied_field_preset
            .get("recipe")
            .map(String::as_str),
        Some("Hoffmann 4:6"),
        "subtitle should reflect just-saved preset"
    );
}

// ---------------------------------------------------------------------------
// Load picker flow
// ---------------------------------------------------------------------------

#[test]
fn l_with_no_saved_presets_shows_status_and_does_not_open_picker() {
    let mut app = make_app();
    open_composite_with_rows(&mut app, Vec::new());

    handle_key(&mut app, key(KeyCode::Char('l')));

    let fs = app.form_state.as_ref().unwrap();
    assert!(fs.field_preset_picker.is_none());
    assert!(fs.composite_status.is_some());
}

#[test]
fn l_opens_picker_populated_from_saved_presets() {
    let mut app = make_app();
    // Seed two presets directly into field_presets (not through the dialog).
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "Hoffmann 4:6".to_string(),
            description: Some("light roast".to_string()),
            rows: rows_3(&[("Bloom", "60", "30")]),
        },
    );
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "Tetsu Kasuya".to_string(),
            description: None,
            rows: rows_3(&[("Bloom", "50", "45")]),
        },
    );
    open_composite_with_rows(&mut app, Vec::new());

    handle_key(&mut app, key(KeyCode::Char('l')));

    let fs = app.form_state.as_ref().unwrap();
    let picker = fs.field_preset_picker.as_ref().expect("picker opens");
    assert_eq!(picker.field_name, "recipe");
    assert_eq!(picker.names, vec!["Hoffmann 4:6", "Tetsu Kasuya"]);
    assert_eq!(
        picker.descriptions,
        vec![Some("light roast".to_string()), None]
    );
    assert_eq!(picker.selected, 0);
}

#[test]
fn picker_up_down_navigates_selection() {
    let mut app = make_app();
    for n in ["A", "B", "C"] {
        app.field_presets.set(
            "coffee.recipe",
            FieldPresetEntry {
                name: n.to_string(),
                description: None,
                rows: rows_3(&[("Bloom", "60", "30")]),
            },
        );
    }
    open_composite_with_rows(&mut app, Vec::new());
    handle_key(&mut app, key(KeyCode::Char('l')));

    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(
        app.form_state
            .as_ref()
            .unwrap()
            .field_preset_picker
            .as_ref()
            .unwrap()
            .selected,
        1
    );
    handle_key(&mut app, key(KeyCode::Down));
    handle_key(&mut app, key(KeyCode::Down)); // wraps to 0
    assert_eq!(
        app.form_state
            .as_ref()
            .unwrap()
            .field_preset_picker
            .as_ref()
            .unwrap()
            .selected,
        0
    );
    handle_key(&mut app, key(KeyCode::Up)); // wraps to 2
    assert_eq!(
        app.form_state
            .as_ref()
            .unwrap()
            .field_preset_picker
            .as_ref()
            .unwrap()
            .selected,
        2
    );
}

#[test]
fn picker_esc_cancels_without_changes() {
    let mut app = make_app();
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "X".to_string(),
            description: None,
            rows: rows_3(&[("Bloom", "60", "30")]),
        },
    );
    let baseline = rows_3(&[("First Pour", "120", "30")]);
    open_composite_with_rows(&mut app, baseline.clone());

    handle_key(&mut app, key(KeyCode::Char('l')));
    handle_key(&mut app, key(KeyCode::Esc));

    let fs = app.form_state.as_ref().unwrap();
    assert!(fs.field_preset_picker.is_none());
    assert_eq!(fs.composite_values["recipe"], baseline, "rows untouched");
}

#[test]
fn picker_enter_emits_apply_action() {
    let mut app = make_app();
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "Hoffmann 4:6".to_string(),
            description: None,
            rows: rows_3(&[("Bloom", "60", "30")]),
        },
    );
    open_composite_with_rows(&mut app, Vec::new());
    handle_key(&mut app, key(KeyCode::Char('l')));

    let action = handle_key(&mut app, key(KeyCode::Enter));

    assert_eq!(
        action,
        FormAction::ApplyFieldPreset {
            field_name: "recipe".to_string(),
            preset_name: "Hoffmann 4:6".to_string(),
        }
    );
    // Picker closes after Enter.
    assert!(
        app.form_state
            .as_ref()
            .unwrap()
            .field_preset_picker
            .is_none()
    );
}

#[test]
fn picker_ctrl_d_emits_delete_action() {
    let mut app = make_app();
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "DropMe".to_string(),
            description: None,
            rows: rows_3(&[("Bloom", "60", "30")]),
        },
    );
    open_composite_with_rows(&mut app, Vec::new());
    handle_key(&mut app, key(KeyCode::Char('l')));

    let action = handle_key(&mut app, ctrl(KeyCode::Char('d')));

    assert_eq!(
        action,
        FormAction::DeleteFieldPreset {
            field_name: "recipe".to_string(),
            preset_name: "DropMe".to_string(),
        }
    );
}

// ---------------------------------------------------------------------------
// Apply: replace silently, schema reconciliation
// ---------------------------------------------------------------------------

#[test]
fn apply_field_preset_replaces_rows_silently() {
    let mut app = make_app();
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "Hoffmann 4:6".to_string(),
            description: None,
            rows: rows_3(&[("Bloom", "60", "30"), ("First Pour", "120", "30")]),
        },
    );
    let dirty = rows_3(&[("Manual", "999", "999")]);
    open_composite_with_rows(&mut app, dirty);

    app.apply_field_preset("recipe", "Hoffmann 4:6");

    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(
        fs.composite_values["recipe"],
        rows_3(&[("Bloom", "60", "30"), ("First Pour", "120", "30")]),
        "rows fully replaced — no append, no confirm"
    );
    assert_eq!(
        fs.last_applied_field_preset
            .get("recipe")
            .map(String::as_str),
        Some("Hoffmann 4:6")
    );
    assert!(
        fs.composite_status.is_none(),
        "no status when shape matches"
    );
}

#[test]
fn apply_field_preset_pads_rows_when_schema_widened() {
    let mut app = make_app();
    // Saved when sub_fields had only 2 columns.
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "Legacy".to_string(),
            description: None,
            rows: vec![vec!["Bloom".to_string(), "60".to_string()]],
        },
    );
    open_composite_with_rows(&mut app, Vec::new());

    app.apply_field_preset("recipe", "Legacy");

    let fs = app.form_state.as_ref().unwrap();
    // Reconciled to 3 columns.
    assert_eq!(
        fs.composite_values["recipe"],
        vec![vec!["Bloom".to_string(), "60".to_string(), String::new(),]]
    );
    assert!(
        fs.composite_status.as_deref() == Some("preset shape adjusted to current schema"),
        "status should call out the adjustment"
    );
}

#[test]
fn apply_field_preset_truncates_rows_when_schema_narrowed() {
    let mut app = make_app();
    // Saved when sub_fields had 4 columns; schema is now 3.
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "Wide".to_string(),
            description: None,
            rows: vec![vec![
                "Bloom".to_string(),
                "60".to_string(),
                "30".to_string(),
                "extra-col".to_string(),
            ]],
        },
    );
    open_composite_with_rows(&mut app, Vec::new());

    app.apply_field_preset("recipe", "Wide");

    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(
        fs.composite_values["recipe"],
        rows_3(&[("Bloom", "60", "30")]),
        "extra trailing cells dropped"
    );
}

// ---------------------------------------------------------------------------
// Quick cycle
// ---------------------------------------------------------------------------

#[test]
fn p_cycles_to_next_preset_via_apply_action() {
    let mut app = make_app();
    for n in ["A", "B", "C"] {
        app.field_presets.set(
            "coffee.recipe",
            FieldPresetEntry {
                name: n.to_string(),
                description: None,
                rows: rows_3(&[("Bloom", "60", "30")]),
            },
        );
    }
    open_composite_with_rows(&mut app, Vec::new());

    // No preset applied yet → cycle starts at index 0 ("A").
    let action = handle_key(&mut app, key(KeyCode::Char('p')));
    assert_eq!(
        action,
        FormAction::ApplyFieldPreset {
            field_name: "recipe".to_string(),
            preset_name: "A".to_string(),
        }
    );

    // Mark "A" as the last applied preset, then cycle to "B".
    app.form_state
        .as_mut()
        .unwrap()
        .last_applied_field_preset
        .insert("recipe".to_string(), "A".to_string());
    let action = handle_key(&mut app, key(KeyCode::Char('p')));
    assert_eq!(
        action,
        FormAction::ApplyFieldPreset {
            field_name: "recipe".to_string(),
            preset_name: "B".to_string(),
        }
    );

    // From "C" wraps back to "A".
    app.form_state
        .as_mut()
        .unwrap()
        .last_applied_field_preset
        .insert("recipe".to_string(), "C".to_string());
    let action = handle_key(&mut app, key(KeyCode::Char('p')));
    assert_eq!(
        action,
        FormAction::ApplyFieldPreset {
            field_name: "recipe".to_string(),
            preset_name: "A".to_string(),
        }
    );
}

#[test]
fn p_with_no_saved_presets_shows_status() {
    let mut app = make_app();
    open_composite_with_rows(&mut app, Vec::new());

    handle_key(&mut app, key(KeyCode::Char('p')));

    let fs = app.form_state.as_ref().unwrap();
    assert!(fs.composite_status.is_some());
}

// ---------------------------------------------------------------------------
// Delete: subtitle clears, picker re-populates
// ---------------------------------------------------------------------------

#[test]
fn delete_field_preset_clears_subtitle_when_active_preset_removed() {
    let mut app = make_app();
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "ToDelete".to_string(),
            description: None,
            rows: rows_3(&[("Bloom", "60", "30")]),
        },
    );
    open_composite_with_rows(&mut app, Vec::new());
    app.form_state
        .as_mut()
        .unwrap()
        .last_applied_field_preset
        .insert("recipe".to_string(), "ToDelete".to_string());

    let _ = app.delete_field_preset("recipe", "ToDelete");

    let fs = app.form_state.as_ref().unwrap();
    assert!(
        !fs.last_applied_field_preset.contains_key("recipe"),
        "subtitle marker cleared when its preset was deleted"
    );
}

#[test]
fn delete_field_preset_repopulates_open_picker() {
    let mut app = make_app();
    for n in ["A", "B"] {
        app.field_presets.set(
            "coffee.recipe",
            FieldPresetEntry {
                name: n.to_string(),
                description: None,
                rows: rows_3(&[("Bloom", "60", "30")]),
            },
        );
    }
    open_composite_with_rows(&mut app, Vec::new());
    handle_key(&mut app, key(KeyCode::Char('l')));

    let _ = app.delete_field_preset("recipe", "A");

    let fs = app.form_state.as_ref().unwrap();
    let picker = fs.field_preset_picker.as_ref().expect("picker still open");
    assert_eq!(picker.names, vec!["B"]);
    assert_eq!(picker.selected, 0);
}

#[test]
fn delete_last_field_preset_closes_picker() {
    let mut app = make_app();
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "OnlyOne".to_string(),
            description: None,
            rows: rows_3(&[("Bloom", "60", "30")]),
        },
    );
    open_composite_with_rows(&mut app, Vec::new());
    handle_key(&mut app, key(KeyCode::Char('l')));

    let _ = app.delete_field_preset("recipe", "OnlyOne");

    let fs = app.form_state.as_ref().unwrap();
    assert!(fs.field_preset_picker.is_none());
    assert!(fs.composite_status.is_some());
}

// ---------------------------------------------------------------------------
// Sandboxing: s/l/p outside the composite overlay should NOT trigger field-preset flow
// ---------------------------------------------------------------------------

#[test]
fn s_outside_composite_does_not_open_field_preset_save_dialog() {
    let mut app = make_app();
    // Do NOT open the composite overlay — focus is on the bean text field.
    let fs_before = app.form_state.as_ref().unwrap();
    assert!(!fs_before.composite_open);

    handle_key(&mut app, key(KeyCode::Char('s')));

    // If a save dialog opened (e.g., the module-level one because we're on a
    // text field), its target must be Module, not CompositeField. The point
    // of this test is that the composite-field save path is gated by
    // composite_open.
    if let Some(dialog) = app.form_state.as_ref().unwrap().preset_overlay.as_ref() {
        assert!(
            matches!(dialog.target, PresetDialogTarget::Module),
            "outside the composite overlay, s must not use CompositeField target"
        );
    }
}

#[test]
fn l_outside_composite_is_a_noop_for_field_picker() {
    let mut app = make_app();
    app.field_presets.set(
        "coffee.recipe",
        FieldPresetEntry {
            name: "X".to_string(),
            description: None,
            rows: rows_3(&[("Bloom", "60", "30")]),
        },
    );
    // composite_open stays false.
    handle_key(&mut app, key(KeyCode::Char('l')));

    let fs = app.form_state.as_ref().unwrap();
    assert!(
        fs.field_preset_picker.is_none(),
        "l outside the composite overlay must not open the field-preset picker"
    );
}
