use std::io;
use std::process;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use pour::app::{App, Screen, SummaryState};
use pour::config::{Config, FieldType, WriteMode};
use pour::data::cache::Cache;
use pour::data::fetch_options;
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

    // Build app state
    let mut app = App::new(config, transport);

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

                tui::Action::Navigate(_) => {
                    // Other screen transitions are handled inside handle_event
                }

                tui::Action::Submit => {
                    handle_submit(app, cache).await;
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
    let field_values = {
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

        form_state.field_values.clone()
    };

    // Clear validation errors
    if let Some(ref mut fs) = app.form_state {
        fs.validation_errors.clear();
    }
    let transport_mode = app.transport.mode();

    // Execute write based on module mode
    let write_result = match module.mode {
        WriteMode::Create => output::write_create(&app.transport, module, &field_values).await,
        WriteMode::Append => output::write_append(&app.transport, module, &field_values).await,
    };

    // Transition to summary screen
    match write_result {
        Ok(vault_path) => {
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
