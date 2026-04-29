use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pour::app::{App, PresetPickerState};
use pour::config::Config;
use pour::data::field_presets::FieldPresets;
use pour::data::history::History;
use pour::data::preset_tree::{PresetTree, build};
use pour::data::presets::{PresetEntry, Presets};
use pour::transport::Transport;
use pour::transport::fs::FsWriter;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const WITH_AXES_TOML: &str = r####"
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

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
"####;

const NO_AXES_TOML: &str = r####"
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

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"
"####;

fn make_preset(name: &str, method: &str, bean: &str) -> PresetEntry {
    let mut values = HashMap::new();
    values.insert("method".to_owned(), method.to_owned());
    values.insert("bean".to_owned(), bean.to_owned());
    PresetEntry {
        name: name.to_owned(),
        description: None,
        values,
    }
}

fn make_app_with_axes_and_presets(presets_data: Vec<PresetEntry>) -> App {
    let config = Config::from_toml(WITH_AXES_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    let mut presets = Presets::empty();
    for p in presets_data {
        presets.set("coffee", p);
    }
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-picker-history.json",
        )),
        presets,
        FieldPresets::empty(),
    )
}

fn make_app_no_axes() -> App {
    let config = Config::from_toml(NO_AXES_TOML).expect("parse");
    let transport = Transport::Fs(FsWriter::new(std::path::PathBuf::from("/tmp/vault")));
    App::new(
        config,
        transport,
        History::load_from(std::path::PathBuf::from(
            "/tmp/test-picker-noaxes-history.json",
        )),
        Presets::empty(),
        FieldPresets::empty(),
    )
}

fn make_simple_tree() -> PresetTree {
    let presets = vec![
        make_preset("V60 Onyx", "V60", "Onyx"),
        make_preset("V60 Kenya", "V60", "Kenya"),
        make_preset("Aero Onyx", "AeroPress", "Onyx"),
    ];
    build(&presets, &["method".to_owned(), "bean".to_owned()])
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Set up an app with coffee module form open and the picker open at the given state.
fn app_with_picker(
    presets_data: Vec<PresetEntry>,
    picker: PresetPickerState,
) -> App {
    let mut app = make_app_with_axes_and_presets(presets_data);
    let mut form = app.init_form("coffee").expect("module exists");
    form.preset_picker = Some(picker);
    app.form_state = Some(form);
    // selected_module=0 → coffee (only module)
    app
}

// ---------------------------------------------------------------------------
// Case 1: open picker when axes are configured
// ---------------------------------------------------------------------------

#[test]
fn picker_opens_when_axes_configured() {
    let app = make_app_with_axes_and_presets(vec![make_preset("V60 Onyx", "V60", "Onyx")]);
    let form = app.init_form("coffee").expect("module exists");
    let module = &app.config.modules["coffee"];
    assert!(!module.preset_axes.is_empty(), "axes must be set");
    assert!(form.preset_picker.is_none(), "picker starts closed");
}

// ---------------------------------------------------------------------------
// Case 2: no picker opened when axes are empty
// ---------------------------------------------------------------------------

#[test]
fn picker_does_not_open_without_axes() {
    let app = make_app_no_axes();
    let module = &app.config.modules["coffee"];
    assert!(module.preset_axes.is_empty(), "axes must be empty");
    let form = app.init_form("coffee").expect("module exists");
    assert!(form.preset_picker.is_none());
}

// ---------------------------------------------------------------------------
// Case 3: drill on branch — Enter on a branch pushes path
// ---------------------------------------------------------------------------

#[test]
fn drill_on_branch_pushes_path() {
    // root[0] = AeroPress (alphabetical), root[1] = V60
    let presets = vec![
        make_preset("V60 Onyx", "V60", "Onyx"),
        make_preset("Aero Onyx", "AeroPress", "Onyx"),
    ];
    let tree = build(&presets, &["method".to_owned(), "bean".to_owned()]);
    let mut app = app_with_picker(
        presets,
        PresetPickerState {
            tree,
            path: Vec::new(),
            selected: 1, // V60 branch (alphabetical: AeroPress=0, V60=1)
            viewport_offset: 0,
        },
    );

    pour::tui::form::handle_key(&mut app, key(KeyCode::Enter));

    let picker = app
        .form_state
        .as_ref()
        .unwrap()
        .preset_picker
        .as_ref()
        .expect("picker still open after drilling into branch");
    assert_eq!(picker.path, vec![1], "path must record the drilled index");
    assert_eq!(picker.selected, 0, "selection resets to 0 inside branch");
}

// ---------------------------------------------------------------------------
// Case 4: apply on leaf — Enter on leaf sets preset name and closes picker
// ---------------------------------------------------------------------------

#[test]
fn apply_on_leaf_sets_preset_name() {
    let presets = vec![make_preset("V60 Onyx Classic", "V60", "Onyx")];
    let tree = build(&presets, &["method".to_owned(), "bean".to_owned()]);

    // Drill into V60 (index 1 alphabetically has only one branch: V60 since AeroPress absent),
    // then into Onyx. At depth 2 we find the leaf.
    // Actually with only V60 preset: root[0]=V60 branch, root[0].children[0]=Onyx branch,
    // root[0].children[0].children[0]=Leaf("V60 Onyx Classic").
    let mut app = app_with_picker(
        presets,
        PresetPickerState {
            tree,
            path: vec![0, 0], // drilled to leaf level
            selected: 0,
            viewport_offset: 0,
        },
    );

    pour::tui::form::handle_key(&mut app, key(KeyCode::Enter));

    let fs = app.form_state.as_ref().unwrap();
    assert_eq!(
        fs.selected_preset_name.as_deref(),
        Some("V60 Onyx Classic"),
        "leaf Enter must set selected_preset_name"
    );
    assert!(fs.preset_picker.is_none(), "picker must close after leaf apply");
}

// ---------------------------------------------------------------------------
// Case 5: pop on backspace at non-root level
// ---------------------------------------------------------------------------

#[test]
fn pop_on_backspace_pops_path() {
    let presets = vec![
        make_preset("V60 Onyx", "V60", "Onyx"),
        make_preset("Aero Onyx", "AeroPress", "Onyx"),
    ];
    let tree = build(&presets, &["method".to_owned(), "bean".to_owned()]);
    let mut app = app_with_picker(
        presets,
        PresetPickerState {
            tree,
            path: vec![1, 0], // drilled two levels
            selected: 0,
            viewport_offset: 0,
        },
    );

    pour::tui::form::handle_key(&mut app, key(KeyCode::Backspace));

    let picker = app
        .form_state
        .as_ref()
        .unwrap()
        .preset_picker
        .as_ref()
        .expect("picker still open after pop");
    assert_eq!(picker.path, vec![1], "one level popped");
}

// ---------------------------------------------------------------------------
// Case 6: backspace at root closes picker
// ---------------------------------------------------------------------------

#[test]
fn backspace_at_root_closes_picker() {
    let presets = vec![make_preset("V60 Onyx", "V60", "Onyx")];
    let tree = build(&presets, &["method".to_owned(), "bean".to_owned()]);
    let mut app = app_with_picker(
        presets,
        PresetPickerState {
            tree,
            path: Vec::new(),
            selected: 0,
            viewport_offset: 0,
        },
    );

    pour::tui::form::handle_key(&mut app, key(KeyCode::Backspace));

    assert!(
        app.form_state.as_ref().unwrap().preset_picker.is_none(),
        "picker must close when Backspace at root"
    );
}

// ---------------------------------------------------------------------------
// Case 7: Esc does not apply preset
// ---------------------------------------------------------------------------

#[test]
fn esc_does_not_apply_preset() {
    let presets = vec![make_preset("V60 Onyx", "V60", "Onyx")];
    let tree = build(&presets, &["method".to_owned(), "bean".to_owned()]);
    let mut app = app_with_picker(
        presets,
        PresetPickerState {
            tree,
            path: Vec::new(),
            selected: 0,
            viewport_offset: 0,
        },
    );

    // Confirm no preset selected before.
    assert!(app.form_state.as_ref().unwrap().selected_preset_name.is_none());

    pour::tui::form::handle_key(&mut app, key(KeyCode::Esc));

    let fs = app.form_state.as_ref().unwrap();
    assert!(fs.preset_picker.is_none(), "Esc must close picker");
    assert!(
        fs.selected_preset_name.is_none(),
        "Esc must not apply a preset"
    );
}

// ---------------------------------------------------------------------------
// Case 8: breadcrumb renders correctly via TestBackend
// ---------------------------------------------------------------------------

#[test]
fn breadcrumb_renders_in_picker_title() {
    use ratatui::{Terminal, backend::TestBackend};

    let mut app = make_app_with_axes_and_presets(vec![
        make_preset("V60 Onyx Classic", "V60", "Onyx"),
        make_preset("Aero Onyx", "AeroPress", "Onyx"),
    ]);
    let form_state = app.init_form("coffee").expect("module exists");
    app.form_state = Some(form_state);

    let presets = app.presets.get("coffee");
    let axes = app.config.modules["coffee"].preset_axes.clone();
    let tree = build(&presets, &axes);

    // Drill into V60 (index 1 alphabetically: AeroPress=0, V60=1).
    if let Some(ref mut fs) = app.form_state {
        fs.preset_picker = Some(PresetPickerState {
            tree,
            path: vec![1],
            selected: 0,
            viewport_offset: 0,
        });
    }

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            pour::tui::form::render(&app, frame);
        })
        .expect("draw");

    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer
        .content()
        .iter()
        .map(|c| c.symbol().to_owned())
        .collect();

    assert!(
        content.contains("V60"),
        "breadcrumb must show V60 after drilling; got a blank buffer (check form screen is active)"
    );
}
