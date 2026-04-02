use std::io;
use std::process;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use pour::app::{App, BrowserState, ConfigureLevel, Screen, SummaryState};
use pour::config::{Config, ConfigError, FieldConfig, FieldTarget, FieldType, FieldUpdates, ModuleConfig, VaultUpdates, WriteMode};
use pour::data::cache::Cache;
use pour::data::fetch_options;
use pour::data::history::History;
use pour::output;
use pour::tui;

#[tokio::main]
async fn main() {
    // Parse CLI args: `pour` = dashboard, `pour <module>` = fast path
    let args: Vec<String> = std::env::args().collect();

    // Handle `pour init` before config loading
    if args.get(1).map(|s| s.as_str()) == Some("init") {
        let force = args.iter().any(|a| a == "--force");
        match pour::init::run(pour::init::InitOptions { force }) {
            Ok(_) => process::exit(0),
            Err(e) => {
                eprintln!("pour init: {e}");
                process::exit(1);
            }
        }
    }

    let fast_path_module = args.get(1).cloned();

    // Load config — exit with user-friendly error on failure
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("pour: {e}");
            process::exit(1);
        }
    };

    // Connect transport (auto-fallback from API to filesystem)
    let transport = pour::transport::Transport::connect(&config).await;

    // Load capture history for dashboard stats
    let history = History::load();

    // Build app state
    let mut app = App::new(config, transport, history);

    // Check for path issues at startup; shown as a dismissable overlay on the dashboard
    app.startup_warnings =
        app.config.check_paths(std::path::Path::new(&app.config.vault.base_path));

    // Load cache for dynamic selects
    let mut cache = Cache::load();

    // Fast path: validate module name and jump directly to form
    if let Some(ref module_name) = fast_path_module {
        if !app.config.modules.contains_key(module_name) {
            eprintln!("pour: unknown module '{module_name}'");
            eprintln!("available modules: {}", app.module_keys.join(", "));
            process::exit(1);
        }

        // Set selected_module index to match the fast-path module
        if let Some(idx) = app.module_keys.iter().position(|k| k == module_name) {
            app.selected_module = idx;
        }

        app.form_state = app.init_form(module_name);
        app.screen = Screen::Form;

        // Fetch dynamic select options for this module
        fetch_dynamic_options(&mut app, module_name, &mut cache).await;
    }

    // Install panic hook that restores terminal before printing panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        original_hook(info);
    }));

    // Initialize terminal
    let mut terminal = ratatui::init();

    // Main event loop
    let result = run_loop(&mut terminal, &mut app, &mut cache).await;

    // Restore terminal
    ratatui::restore();

    // Report any error from the main loop
    if let Err(e) = result {
        eprintln!("pour: {e}");
        process::exit(1);
    }
}

/// The main TUI event loop. Returns Ok(()) on clean exit, Err on fatal error.
async fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    cache: &mut Cache,
) -> io::Result<()> {
    loop {
        // Draw
        terminal.draw(|frame| tui::render(app, frame))?;

        // Poll for events with a short timeout to keep the UI responsive
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key_event) = event::read()?
        {
            if !pour::should_handle_key_event(key_event) {
                continue;
            }

            // Ctrl+C always quits cleanly
            if key_event.modifiers.contains(KeyModifiers::CONTROL)
                && key_event.code == KeyCode::Char('c')
            {
                break;
            }

            let action = tui::handle_event(app, key_event);

            match action {
                tui::Action::Quit => break,

                tui::Action::Navigate(Screen::Form) => {
                    // Screen transition already happened inside handle_event.
                    // Fetch dynamic select options for the newly opened form.
                    if let Some(key) = app.module_keys.get(app.selected_module).cloned() {
                        fetch_dynamic_options(app, &key, cache).await;
                    }
                }

                tui::Action::Navigate(Screen::Configure) => {
                    // Pre-fetch the directory listing for the module's current path.
                    // Skip for VaultSettings — base_path is a system path, not vault-relative.
                    let is_vault_settings = app
                        .configure_state
                        .as_ref()
                        .map(|s| s.level == ConfigureLevel::VaultSettings)
                        .unwrap_or(false);

                    if !is_vault_settings
                        && let Some(ref state) = app.configure_state
                    {
                        let path = state
                            .settings
                            .iter()
                            .find(|s| s.key == "path")
                            .map(|s| {
                                // Use the directory portion of the path value
                                let v = s.value.as_str();
                                let trimmed = v.trim_end_matches('/');
                                if let Some(pos) = trimmed.rfind('/') {
                                    trimmed[..pos].to_string()
                                } else {
                                    String::new()
                                }
                            })
                            .unwrap_or_default();
                        handle_browse(app, &path).await;
                        // Close the browser — the pre-fetch just seeds the state
                        if let Some(ref mut s) = app.configure_state {
                            s.browser_open = false;
                        }
                    }
                }

                tui::Action::Navigate(_) => {
                    // Other screen transitions are handled inside handle_event
                }

                tui::Action::Submit => {
                    handle_submit(app, cache).await;
                }

                tui::Action::Save => {
                    handle_save(app).await;
                }

                tui::Action::Browse(path) => {
                    handle_browse(app, &path).await;
                }

                tui::Action::AddField => {
                    handle_add_field(app);
                }

                tui::Action::RemoveField(idx) => {
                    handle_remove_field(app, idx);
                }

                tui::Action::ReorderFields(a, b) => {
                    handle_reorder_fields(app, a, b);
                }

                tui::Action::DeleteModule => {
                    handle_delete_module(app);
                }

                tui::Action::ReorderModules(dir) => {
                    handle_reorder_modules(app, dir);
                }

                tui::Action::NewModule => {
                    handle_new_module(app);
                }

                tui::Action::SaveNewModule => {
                    handle_save_new_module(app);
                }

                tui::Action::None => {}
            }
        }
    }

    Ok(())
}

/// Handle form submission: validate, write, transition to summary.
async fn handle_submit(app: &mut App, cache: &mut Cache) {
    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k.clone(),
        None => return,
    };

    let module = match app.config.modules.get(&module_key) {
        Some(m) => m,
        None => return,
    };

    // Validate form and extract field values
    let (field_values, composite_data) = {
        let form_state = match &app.form_state {
            Some(fs) => fs,
            None => return,
        };

        let errors = App::validate_form(module, form_state);
        if !errors.is_empty() {
            let errors_clone = errors;
            if let Some(ref mut fs) = app.form_state {
                fs.validation_errors = errors_clone;
            }
            return;
        }

        (form_state.field_values.clone(), form_state.composite_values.clone())
    };

    // Clear validation errors
    if let Some(ref mut fs) = app.form_state {
        fs.validation_errors.clear();
    }
    let transport_mode = app.transport.mode();

    // Execute write based on module mode
    let date_fmt = app.config.vault.date_format.as_deref();
    let write_result = match module.mode {
        WriteMode::Create => output::write_create(&app.transport, module, &field_values, &composite_data, date_fmt).await,
        WriteMode::Append => output::write_append(&app.transport, module, &field_values, &composite_data, date_fmt).await,
    };

    // Transition to summary screen
    match write_result {
        Ok(vault_path) => {
            // Record successful capture in history
            app.history.record(&module_key, &vault_path);

            app.summary_state = Some(SummaryState {
                message: "Entry saved successfully.".to_string(),
                file_path: Some(vault_path),
                transport_mode,
            });
        }
        Err(e) => {
            app.summary_state = Some(SummaryState {
                message: format!("Write failed: {e}"),
                file_path: None,
                transport_mode,
            });
        }
    }

    app.form_state = None;
    app.screen = Screen::Summary;

    // Persist cache after write (best-effort)
    let _ = cache.save();
}

/// Save configure state to disk and reload the config in memory.
async fn handle_save(app: &mut App) {
    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };

    let module_key = state.module_key.clone();
    let level = state.level.clone();

    let result = match level {
        ConfigureLevel::FieldEditor(field_idx) => {
            // Build FieldUpdates from the current settings
            let updates = build_field_updates(state);
            Config::update_field_on_disk(&module_key, field_idx, &updates)
        }
        ConfigureLevel::VaultSettings => {
            // Pre-validate vault settings before attempting disk write
            if let Err(msg) = validate_vault_settings(state) {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(msg);
                }
                return;
            }
            let updates = build_vault_updates(state);
            Config::update_vault_on_disk(&updates)
        }
        _ => {
            // Build ModuleUpdates from the current settings
            let updates = build_module_updates(state);
            Config::update_module_on_disk(&module_key, &updates)
        }
    };

    match result {
        Ok(()) => match Config::load() {
            Ok(new_config) => {
                app.config = new_config;

                // Rebuild settings from the fresh config to reflect the saved state
                if let ConfigureLevel::FieldEditor(idx) = level
                    && let Some(field) = app
                        .config
                        .modules
                        .get(&module_key)
                        .and_then(|m| m.fields.get(idx))
                    && let Some(ref mut s) = app.configure_state
                {
                    s.settings = App::build_field_settings(field);
                }

                // Rebuild vault settings from the fresh config
                if level == ConfigureLevel::VaultSettings {
                    let vault = &app.config.vault;
                    if let Some(ref mut s) = app.configure_state {
                        s.settings = vec![
                            pour::app::ConfigSetting {
                                label: "Base Path".to_string(),
                                key: "base_path".to_string(),
                                value: vault.base_path.clone(),
                                kind: pour::app::SettingKind::Text,
                            },
                            pour::app::ConfigSetting {
                                label: "API Port".to_string(),
                                key: "api_port".to_string(),
                                value: vault.api_port.map(|p| p.to_string()).unwrap_or_default(),
                                kind: pour::app::SettingKind::Text,
                            },
                            pour::app::ConfigSetting {
                                label: "API Key".to_string(),
                                key: "api_key".to_string(),
                                value: vault.api_key.clone().unwrap_or_default(),
                                kind: pour::app::SettingKind::Text,
                            },
                        ];
                    }
                }

                if let Some(ref mut s) = app.configure_state {
                    s.dirty = false;
                    s.status_message = None;
                }

                // Warn if the saved config introduced path issues
                let path_warnings = app
                    .config
                    .check_paths(std::path::Path::new(&app.config.vault.base_path));
                if !path_warnings.is_empty()
                    && let Some(ref mut s) = app.configure_state
                {
                    s.status_message = Some(format!("Warning: {}", path_warnings.join("; ")));
                }
            }
            Err(e) => {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(format!("Reload failed: {e}"));
                }
            }
        },
        Err(ConfigError::ValidationError(errs)) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Validation: {}", errs.join("; ")));
            }
        }
        Err(e) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Save failed: {e}"));
            }
        }
    }
}

/// Extract ModuleUpdates from the current configure settings.
/// Add a new default field to the current module and open its editor.
fn handle_add_field(app: &mut App) {
    let module_key = match &app.configure_state {
        Some(s) => s.module_key.clone(),
        None => return,
    };

    let new_field = FieldConfig {
        name: "new_field".to_string(),
        field_type: FieldType::Text,
        prompt: "New field".to_string(),
        required: None,
        default: None,
        options: None,
        source: None,
        target: None,
        sub_fields: None,
    };

    match Config::add_field_on_disk(&module_key, &new_field) {
        Ok(()) => match Config::load() {
            Ok(new_config) => {
                let new_field_idx = new_config
                    .modules
                    .get(&module_key)
                    .map(|m| m.fields.len().saturating_sub(1))
                    .unwrap_or(0);

                app.config = new_config;

                // Open the field editor for the new field
                if let Some(field) = app
                    .config
                    .modules
                    .get(&module_key)
                    .and_then(|m| m.fields.get(new_field_idx))
                    && let Some(ref mut s) = app.configure_state
                {
                    s.settings = App::build_field_settings(field);
                    s.level = ConfigureLevel::FieldEditor(new_field_idx);
                    s.active_field = 0;
                    s.status_message = None;
                }
            }
            Err(e) => {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(format!("Reload failed: {e}"));
                }
            }
        },
        Err(e) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Add failed: {e}"));
            }
        }
    }
}

/// Remove a field at the given index from the current module.
fn handle_remove_field(app: &mut App, field_index: usize) {
    let module_key = match &app.configure_state {
        Some(s) => s.module_key.clone(),
        None => return,
    };

    match Config::remove_field_on_disk(&module_key, field_index) {
        Ok(()) => match Config::load() {
            Ok(new_config) => {
                app.config = new_config;

                // Stay on the field list, adjust active_field if needed
                if let Some(ref mut s) = app.configure_state {
                    let new_field_count = app
                        .config
                        .modules
                        .get(&module_key)
                        .map(|m| m.fields.len())
                        .unwrap_or(0);
                    // active_field 0 is "< Back", fields start at 1
                    let max_field = new_field_count; // last valid index = field_count (offset by 1 for Back)
                    if s.active_field > max_field {
                        s.active_field = max_field;
                    }
                    s.status_message = None;
                }
            }
            Err(e) => {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(format!("Reload failed: {e}"));
                }
            }
        },
        Err(ConfigError::ValidationError(errs)) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Cannot delete: {}", errs.join("; ")));
            }
        }
        Err(e) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Delete failed: {e}"));
            }
        }
    }
}

/// Swap two fields (at indices a and b) in the current module and persist to disk.
fn handle_reorder_fields(app: &mut App, a: usize, b: usize) {
    let (module_key, original_active) = match &app.configure_state {
        Some(s) => (s.module_key.clone(), s.active_field),
        None => return,
    };

    let field_count = app
        .config
        .modules
        .get(&module_key)
        .map(|m| m.fields.len())
        .unwrap_or(0);

    // Build permutation: identity with a and b swapped
    let mut new_order: Vec<usize> = (0..field_count).collect();
    new_order.swap(a, b);

    match Config::reorder_fields_on_disk(&module_key, &new_order) {
        Ok(()) => match Config::load() {
            Ok(new_config) => {
                app.config = new_config;
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = None;
                }
            }
            Err(e) => {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(format!("Reload failed: {e}"));
                }
            }
        },
        Err(e) => {
            if let Some(ref mut s) = app.configure_state {
                // Restore cursor to its position before configure.rs moved it
                s.active_field = original_active;
                s.status_message = Some(format!("Reorder failed: {e}"));
            }
        }
    }
}

/// Delete the current module and return to the dashboard.
fn handle_delete_module(app: &mut App) {
    let module_key = match &app.configure_state {
        Some(s) => s.module_key.clone(),
        None => return,
    };

    match Config::delete_module_on_disk(&module_key) {
        Ok(()) => match Config::load() {
            Ok(new_config) => {
                app.config = new_config;

                // Rebuild module_keys from the fresh config, preserving existing order
                app.module_keys.retain(|k| app.config.modules.contains_key(k.as_str()));

                // Clamp selected_module to a valid index
                if !app.module_keys.is_empty() && app.selected_module >= app.module_keys.len() {
                    app.selected_module = app.module_keys.len() - 1;
                } else if app.module_keys.is_empty() {
                    app.selected_module = 0;
                }

                app.configure_state = None;
                app.screen = Screen::Dashboard;
            }
            Err(e) => {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(format!("Reload failed: {e}"));
                }
            }
        },
        Err(e) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Delete failed: {e}"));
            }
        }
    }
}

/// Swap the selected module in the given direction, persist to disk.
///
/// If the disk write fails, the swap is rolled back so in-memory state
/// stays consistent with on-disk state.
fn handle_reorder_modules(app: &mut App, dir: pour::tui::dashboard::MoveDirection) {
    let idx = app.selected_module;
    let new_idx = match dir {
        pour::tui::dashboard::MoveDirection::Up => {
            if idx == 0 { return; }
            idx - 1
        }
        pour::tui::dashboard::MoveDirection::Down => {
            if idx + 1 >= app.module_keys.len() { return; }
            idx + 1
        }
    };

    // Apply the swap optimistically
    app.module_keys.swap(idx, new_idx);
    app.selected_module = new_idx;

    // Persist to disk
    match Config::update_module_order_on_disk(&app.module_keys) {
        Ok(()) => {
            // Reload config to stay in sync, but preserve the current order and selection.
            if let Ok(new_config) = Config::load() {
                app.config = new_config;
            }
        }
        Err(_e) => {
            // Rollback: undo the swap so in-memory matches on-disk
            app.module_keys.swap(idx, new_idx);
            app.selected_module = idx;
        }
    }
}

/// Transition to the new-module creation screen.
fn handle_new_module(app: &mut App) {
    let state = app.init_new_module_configure();
    app.configure_state = Some(state);
    app.screen = Screen::Configure;
}

/// Save the new module from ConfigureLevel::NewModule to disk, then open its configurator.
fn handle_save_new_module(app: &mut App) {
    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };

    // Extract fields from settings
    let mut module_key = String::new();
    let mut display_name: Option<String> = None;
    let mut mode = WriteMode::Create;
    let mut path = String::new();

    for setting in &state.settings {
        match setting.key.as_str() {
            "module_key" => module_key = setting.value.clone(),
            "display_name" => {
                if !setting.value.is_empty() {
                    display_name = Some(setting.value.clone());
                }
            }
            "mode" => {
                mode = if setting.value == "append" {
                    WriteMode::Append
                } else {
                    WriteMode::Create
                };
            }
            "path" => path = setting.value.clone(),
            _ => {}
        }
    }

    // Validate module_key
    if module_key.is_empty() {
        if let Some(ref mut s) = app.configure_state {
            s.status_message = Some("Module Key must not be empty".to_string());
        }
        return;
    }

    let valid_key = module_key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !valid_key {
        if let Some(ref mut s) = app.configure_state {
            s.status_message =
                Some("Module Key: only a-z, A-Z, 0-9, _ and - are allowed".to_string());
        }
        return;
    }

    if app.config.modules.contains_key(&module_key) {
        if let Some(ref mut s) = app.configure_state {
            s.status_message = Some(format!("Module '{module_key}' already exists"));
        }
        return;
    }

    // Build a minimal ModuleConfig with one default text field
    let new_module = ModuleConfig {
        mode,
        path,
        append_under_header: None,
        append_template: None,
        display_name,
        fields: vec![FieldConfig {
            name: "title".to_string(),
            field_type: FieldType::Text,
            prompt: "Title".to_string(),
            required: None,
            default: None,
            options: None,
            source: None,
            target: None,
            sub_fields: None,
        }],
    };

    match Config::add_module_on_disk(&module_key, &new_module) {
        Ok(()) => match Config::load() {
            Ok(new_config) => {
                app.config = new_config;

                // Rebuild module_keys preserving existing order, appending the new key
                let existing_order = app.module_keys.clone();
                let mut keys: Vec<String> = existing_order
                    .into_iter()
                    .filter(|k| app.config.modules.contains_key(k.as_str()))
                    .collect();
                if !keys.contains(&module_key) {
                    keys.push(module_key.clone());
                }
                app.module_keys = keys;

                // Open the configure screen for the newly created module
                app.configure_state = app.init_configure(&module_key);
                app.screen = Screen::Configure;
            }
            Err(e) => {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(format!("Reload failed: {e}"));
                }
            }
        },
        Err(ConfigError::DuplicateModule(key)) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Module '{key}' already exists"));
            }
        }
        Err(ConfigError::ValidationError(errs)) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Validation: {}", errs.join("; ")));
            }
        }
        Err(e) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Create failed: {e}"));
            }
        }
    }
}

fn build_module_updates(state: &pour::app::ConfigureState) -> pour::config::ModuleUpdates {
    let mut path: Option<String> = None;
    let mut display_name: Option<Option<String>> = None;
    let mut mode: Option<WriteMode> = None;
    let mut append_under_header: Option<Option<String>> = None;

    for setting in &state.settings {
        match setting.key.as_str() {
            "path" => path = Some(setting.value.clone()),
            "display_name" => {
                display_name = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "mode" => {
                mode = Some(if setting.value == "append" {
                    WriteMode::Append
                } else {
                    WriteMode::Create
                });
            }
            "append_under_header" => {
                append_under_header = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            _ => {}
        }
    }

    pour::config::ModuleUpdates {
        path,
        display_name,
        mode,
        append_under_header,
    }
}

/// Pre-validate vault settings before saving. Returns Err(message) on failure.
fn validate_vault_settings(state: &pour::app::ConfigureState) -> Result<(), String> {
    for setting in &state.settings {
        match setting.key.as_str() {
            "base_path" => {
                if setting.value.trim().is_empty() {
                    return Err("Base Path must not be empty".to_string());
                }
            }
            "api_port" => {
                let trimmed = setting.value.trim();
                if !trimmed.is_empty() && trimmed.parse::<u16>().is_err() {
                    return Err(format!("API Port must be a number (1-65535), got '{trimmed}'"));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Extract VaultUpdates from the current vault configure settings.
///
/// Callers must run `validate_vault_settings` first to ensure values are valid.
fn build_vault_updates(state: &pour::app::ConfigureState) -> VaultUpdates {
    let mut base_path: Option<String> = None;
    let mut api_port: Option<Option<u16>> = None;
    let mut api_key: Option<Option<String>> = None;
    let mut date_format: Option<Option<String>> = None;

    for setting in &state.settings {
        match setting.key.as_str() {
            "base_path" => {
                base_path = Some(setting.value.clone());
            }
            "api_port" => {
                let trimmed = setting.value.trim();
                api_port = Some(if trimmed.is_empty() {
                    None
                } else {
                    // Pre-validated by validate_vault_settings
                    trimmed.parse::<u16>().ok()
                });
            }
            "api_key" => {
                api_key = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "date_format" => {
                date_format = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            _ => {}
        }
    }

    VaultUpdates {
        base_path,
        api_port,
        api_key,
        date_format,
    }
}

/// Extract FieldUpdates from the current configure settings.
fn build_field_updates(state: &pour::app::ConfigureState) -> FieldUpdates {
    let mut name: Option<String> = None;
    let mut field_type: Option<FieldType> = None;
    let mut prompt: Option<String> = None;
    let mut required: Option<Option<bool>> = None;
    let mut default: Option<Option<String>> = None;
    let mut options: Option<Option<Vec<String>>> = None;
    let mut source: Option<Option<String>> = None;
    let mut target: Option<Option<FieldTarget>> = None;

    for setting in &state.settings {
        match setting.key.as_str() {
            "name" => name = Some(setting.value.clone()),
            "prompt" => prompt = Some(setting.value.clone()),
            "field_type" => {
                field_type = Some(match setting.value.as_str() {
                    "text" => FieldType::Text,
                    "textarea" => FieldType::Textarea,
                    "number" => FieldType::Number,
                    "static_select" => FieldType::StaticSelect,
                    "dynamic_select" => FieldType::DynamicSelect,
                    "composite_array" => FieldType::CompositeArray,
                    _ => FieldType::Text,
                });
            }
            "required" => {
                required = Some(if setting.value == "true" {
                    Some(true)
                } else {
                    None // false is the default, remove the key
                });
            }
            "default" => {
                default = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "options" => {
                let items: Vec<String> = setting
                    .value
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.to_string())
                    .collect();
                options = Some(if items.is_empty() { None } else { Some(items) });
            }
            "source" => {
                source = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "target" => {
                target = Some(match setting.value.as_str() {
                    "frontmatter" => Some(FieldTarget::Frontmatter),
                    "body" => Some(FieldTarget::Body),
                    _ => None,
                });
            }
            _ => {}
        }
    }

    FieldUpdates {
        name,
        field_type,
        prompt,
        required,
        default,
        options,
        source,
        target,
    }
}

/// Fetch a directory listing and populate the browser state.
async fn handle_browse(app: &mut App, path: &str) {
    let entries = app
        .transport
        .list_directory_entries(path)
        .await
        .unwrap_or_default();

    if let Some(ref mut state) = app.configure_state {
        state.browser_state = Some(BrowserState {
            current_path: path.to_string(),
            entries,
            selected: 0,
            loading: false,
        });
        state.browser_open = true;
    }
}

/// Fetch dynamic select options for all dynamic_select fields in a module.
async fn fetch_dynamic_options(app: &mut App, module_key: &str, cache: &mut Cache) {
    let module = match app.config.modules.get(module_key) {
        Some(m) => m,
        None => return,
    };

    // Collect (field_name, source) pairs for dynamic_select fields
    let dynamic_fields: Vec<(String, String)> = module
        .fields
        .iter()
        .filter(|f| f.field_type == FieldType::DynamicSelect)
        .filter_map(|f| f.source.as_ref().map(|s| (f.name.clone(), s.clone())))
        .collect();

    for (field_name, source) in dynamic_fields {
        let options = fetch_options(&app.transport, &source, cache).await;
        if let Some(ref mut fs) = app.form_state {
            fs.field_options.insert(field_name, options);
        }
    }
}
