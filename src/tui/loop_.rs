// LINTOK: oversized: event loop + 21 action handlers colocated; per-handler split deferred to v1.1
use std::io;
use std::time::Duration;

use crate::app::{App, BrowserState, ConfigureLevel, Screen, SummaryState};
use crate::config::{
    Config, ConfigError, FieldConfig, FieldType, ModuleConfig, SubFieldConfig, SubFieldType,
    WriteMode,
};
use crate::config_updates::{
    build_field_updates, build_module_updates, build_sub_field_updates, build_vault_updates,
    validate_vault_settings,
};
use crate::data::cache::Cache;
use crate::data::fetch_options;
use crate::output;
use crate::tui;
use crate::visibility::visible_field_indices;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};

/// The main TUI event loop. Returns Ok(()) on clean exit, Err on fatal error.
pub async fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    cache: &mut Cache,
) -> io::Result<()> {
    'main: loop {
        // Draw
        terminal.draw(|frame| tui::render(app, frame))?;

        // Poll for events with a short timeout to keep the UI responsive
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        // Collect key events from this poll. A Paste event is expanded into
        // synthetic Char key events so pasted text (e.g. API keys) is handled
        // the same as typed text.
        let ev = event::read()?;
        let key_events: Vec<crossterm::event::KeyEvent> = match ev {
            Event::Key(k) => vec![k],
            Event::Paste(text) => text
                .chars()
                .map(|c| crossterm::event::KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
                .collect(),
            _ => continue,
        };

        for key_event in key_events {
            if !crate::should_handle_key_event(key_event) {
                continue;
            }

            // Ctrl+C always quits cleanly
            if key_event.modifiers.contains(KeyModifiers::CONTROL)
                && key_event.code == KeyCode::Char('c')
            {
                break 'main;
            }

            let action = tui::handle_event(app, key_event);

            match action {
                tui::Action::Quit => break 'main,

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

                    if !is_vault_settings && let Some(ref state) = app.configure_state {
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

                tui::Action::AddSubField(field_idx) => {
                    handle_add_sub_field(app, field_idx);
                }

                tui::Action::RemoveSubField(fi, si) => {
                    handle_remove_sub_field(app, fi, si);
                }

                tui::Action::ReorderSubFields(fi, a, b) => {
                    handle_reorder_sub_fields(app, fi, a, b);
                }

                tui::Action::RefreshTransport => {
                    app.transport = crate::transport::Transport::connect(&app.config).await;
                }

                // Suspend the TUI, run the PWA server inline, then resume.
                //
                // Lifecycle:
                //   1. Register the Ctrl+C signal handler immediately via a spawned
                //      task + oneshot, BEFORE leaving the TUI. This closes the window
                //      where a Ctrl+C during banner/bind setup would kill the process
                //      with no TUI restore.
                //   2. `ratatui::restore()` — leave alt-screen, exit raw mode so the
                //      cooked terminal is available for QR/banner output.
                //   3. Probe the port: bind it now to detect conflicts before printing
                //      the banner (TOCTOU-free — the listener is passed through).
                //   4. `print_banner` — QR code + URL + footer hint to cooked terminal.
                //   5. `run_with_shutdown` with the oneshot shutdown future — Ctrl+C
                //      fires the signal task, which sends on the channel, which resolves
                //      the server's shutdown future.
                //   6. Bounded drain: 5s timeout on the server future; force-drop on
                //      timeout so we always return to the dashboard.
                //   7. `ratatui::init()` + `terminal.clear()` — re-enter alt-screen.
                //   8. Any error (port conflict, bind error, drain timeout) is pushed to
                //      `app.startup_warnings` so it surfaces as a dismissable overlay on
                //      the very next dashboard render.
                tui::Action::Serve => {
                    use std::net::SocketAddr;
                    use std::time::Duration;

                    let port: u16 = 8421;

                    // C3: Install the Ctrl+C signal handler NOW, before any setup work.
                    // A tokio::spawn'd task registers the handler immediately; a oneshot
                    // channel carries the signal to the server's shutdown future.
                    // If ctrl_c() returns Err (signal handler install fails — sandbox,
                    // conflicting handler, Windows ConsoleCtrlHandler issues), the task
                    // just exits without sending, which means the server will drain only
                    // on the 5s timeout — acceptable fallback behavior.
                    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
                    let _signal_task = tokio::spawn(async move {
                        if tokio::signal::ctrl_c().await.is_ok() {
                            let _ = shutdown_tx.send(());
                        }
                        // On Err: handler install failed. The server will run until the
                        // 5s drain timeout fires, then return control to the dashboard.
                    });

                    // Leave TUI before printing anything.
                    ratatui::restore();

                    // Probe the port. Passing the already-bound listener into the server
                    // eliminates the TOCTOU race between "port free?" and "server bind".
                    let probe_addr = SocketAddr::from(([0, 0, 0, 0], port));
                    match tokio::net::TcpListener::bind(probe_addr).await {
                        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
                            // Port is in use — return to dashboard with a visible warning.
                            *terminal = ratatui::init();
                            terminal.clear()?;
                            app.startup_warnings.push(format!(
                                "serve: port {port} already in use — is `pour serve` running elsewhere?"
                            ));
                        }
                        Err(e) => {
                            *terminal = ratatui::init();
                            terminal.clear()?;
                            app.startup_warnings
                                .push(format!("serve: could not bind port {port}: {e}"));
                        }
                        Ok(listener) => {
                            // H1: Reuse app.config and app.transport so the server and TUI share
                            // the same config view, eliminating reload latency and divergence.
                            let config_clone = app.config.clone();
                            let transport_mode = app.transport.mode();

                            // Resolve or generate the mobile token (same semantics as CLI).
                            let token = crate::server::startup::resolve_token();

                            // Print the styled banner (QR, URL, transport, footer hint).
                            crate::server::startup::print_banner(
                                &crate::server::startup::StartupContext {
                                    port,
                                    token: token.clone(),
                                    transport_mode,
                                },
                            );

                            // Build the shutdown future from the oneshot receiver installed
                            // above. This is safe: shutdown_rx.await resolves to Ok(()) when
                            // the signal task fires, or Err if the sender was dropped (which
                            // means the signal handler failed — the future still resolves and
                            // triggers shutdown, which is the desired safe behavior).
                            let shutdown = async move {
                                let _ = shutdown_rx.await;
                            };

                            // H1: Reconnect transport for the server using the same config.
                            // We can't move `app.transport` (app is borrowed mutably by this
                            // loop), so we connect a fresh transport from the shared config.
                            let server_transport =
                                crate::transport::Transport::connect(&config_clone).await;

                            let serve_fut = crate::server::run_with_shutdown(
                                config_clone,
                                server_transport,
                                port,
                                token,
                                listener,
                                shutdown,
                            );

                            // Await with a 5s drain timeout. Ctrl+C fires the oneshot above;
                            // axum drains open connections and then resolves. We print
                            // "Stopping…" + outcome after the future resolves.
                            match tokio::time::timeout(Duration::from_secs(5), serve_fut).await {
                                Ok(Ok(())) => {
                                    eprintln!("\nStopping…\nServer stopped.");
                                }
                                Ok(Err(e)) => {
                                    eprintln!("\nStopping…");
                                    app.startup_warnings
                                        .push(format!("serve: server error: {e}"));
                                }
                                Err(_) => {
                                    eprintln!("\nStopping…");
                                    app.startup_warnings.push(
                                        "serve: server did not drain within 5s; forced exit."
                                            .to_string(),
                                    );
                                }
                            }

                            *terminal = ratatui::init();
                            terminal.clear()?;
                        }
                    }
                    // H4: Discard any keys that were buffered in this same poll batch.
                    // Without this, a key pressed just after 's' (e.g. 'q') would be
                    // processed against the freshly re-initialised dashboard and quit.
                    break;
                }

                tui::Action::CreateFromTemplate {
                    field_name,
                    template_name,
                    note_name,
                    field_values,
                } => {
                    handle_create_from_template(
                        app,
                        cache,
                        &field_name,
                        &template_name,
                        &note_name,
                        &field_values,
                    )
                    .await;
                }

                tui::Action::OpenInObsidian(file_path) => {
                    let uri = obsidian_uri(&app.config.vault.base_path, file_path.as_deref());
                    if let Err(e) = open::that(&uri)
                        && let Some(ref mut ss) = app.summary_state
                    {
                        ss.message
                            .push_str(&format!(" (Warning: could not open Obsidian: {e})"));
                    }
                }

                tui::Action::SavePreset {
                    name,
                    description,
                    values,
                } => {
                    handle_save_preset(app, &name, description, values);
                }

                tui::Action::DeletePreset { name } => {
                    handle_delete_preset(app, &name);
                }

                tui::Action::ReorderPreset { name, direction } => {
                    handle_reorder_preset(app, &name, direction);
                }

                tui::Action::AppendStaticOption { field_index, value } => {
                    handle_append_static_option(app, field_index, &value);
                }

                tui::Action::SaveFieldPreset {
                    field_name,
                    name,
                    description,
                    rows,
                } => {
                    app.save_field_preset(&field_name, &name, description, rows);
                }

                tui::Action::ApplyFieldPreset {
                    field_name,
                    preset_name,
                } => {
                    app.apply_field_preset(&field_name, &preset_name);
                }

                tui::Action::DeleteFieldPreset {
                    field_name,
                    preset_name,
                } => {
                    app.delete_field_preset(&field_name, &preset_name);
                }

                tui::Action::None => {}
            }
        }
    }

    Ok(())
}

/// Upsert a preset for the current module and refresh form state.
fn handle_save_preset(
    app: &mut App,
    name: &str,
    description: Option<String>,
    values: std::collections::HashMap<String, String>,
) {
    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k.clone(),
        None => return,
    };

    let entry = crate::data::presets::PresetEntry {
        name: name.to_string(),
        description,
        values,
    };
    app.presets.set(&module_key, entry);
    if let Err(e) = app.presets.save() {
        // Silently swallow — we're in raw terminal mode, eprintln would corrupt display.
        let _ = e;
    }

    // Refresh preset_names + descriptions and select the newly saved preset
    let saved = app.presets.get(&module_key);
    let names: Vec<String> = saved.iter().map(|p| p.name.clone()).collect();
    let descriptions: Vec<Option<String>> = saved.iter().map(|p| p.description.clone()).collect();
    if let Some(ref mut fs) = app.form_state {
        fs.preset_names = names;
        fs.preset_descriptions = descriptions;
        fs.selected_preset_name = Some(name.to_string());
    }
}

/// Delete a preset for the current module and reset preset selection.
///
/// Only removes the preset from the saved list — does NOT reset form field
/// values, since the user may have manually edited fields they want to keep.
fn handle_delete_preset(app: &mut App, name: &str) {
    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k.clone(),
        None => return,
    };

    app.presets.delete(&module_key, name);
    if let Err(e) = app.presets.save() {
        // Silently swallow — we're in raw terminal mode, eprintln would corrupt display.
        let _ = e;
    }

    let saved = app.presets.get(&module_key);
    let names: Vec<String> = saved.iter().map(|p| p.name.clone()).collect();
    let descriptions: Vec<Option<String>> = saved.iter().map(|p| p.description.clone()).collect();
    if let Some(ref mut fs) = app.form_state {
        fs.preset_names = names;
        fs.preset_descriptions = descriptions;
        fs.selected_preset_name = None;
    }
}

/// Reorder a preset for the current module in the given direction.
fn handle_reorder_preset(app: &mut App, name: &str, direction: i32) {
    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k.clone(),
        None => return,
    };

    app.presets.reorder(&module_key, name, direction);
    if let Err(e) = app.presets.save() {
        let _ = e;
    }

    // Refresh preset_names + descriptions and find the moved preset's new position
    let saved = app.presets.get(&module_key);
    let names: Vec<String> = saved.iter().map(|p| p.name.clone()).collect();
    let descriptions: Vec<Option<String>> = saved.iter().map(|p| p.description.clone()).collect();
    if let Some(ref mut fs) = app.form_state {
        fs.preset_names = names;
        fs.preset_descriptions = descriptions;
        // Do NOT overwrite selected_preset_name here: reordering changes list position
        // but not identity. The name-keyed selection survives unchanged.
    }
}

/// Append a novel option to a static_select module field's options list.
///
/// Mutates the in-memory config so the field's dropdown reflects the new
/// option immediately, then persists the change to config.toml. The
/// form's `field_options` snapshot has already been updated by the form
/// handler — this function only handles the Config + disk side.
fn handle_append_static_option(app: &mut App, field_index: usize, value: &str) {
    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k.clone(),
        None => return,
    };

    // Update in-memory config so subsequent form inits see the new option.
    if let Some(module) = app.config.modules.get_mut(&module_key)
        && let Some(field) = module.fields.get_mut(field_index)
    {
        let already = field
            .options
            .as_ref()
            .map(|opts| opts.iter().any(|o| o == value))
            .unwrap_or(false);
        if !already {
            field
                .options
                .get_or_insert_with(Vec::new)
                .push(value.to_string());
        }
    }

    // Persist to disk; swallow errors silently (raw terminal mode).
    let _ = crate::config::Config::append_option_to_field_on_disk(&module_key, field_index, value);
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
    let (field_values, field_options, composite_data, callout_overrides, callout_titles) = {
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

        // Collect visible field names so stale hidden values are not written to the vault
        let visible_names: std::collections::HashSet<String> = {
            let visible_indices = visible_field_indices(&module.fields, &form_state.field_values);
            visible_indices
                .into_iter()
                .map(|i| module.fields[i].name.clone())
                .collect()
        };

        let mut values = form_state.field_values.clone();
        values.retain(|k, _| visible_names.contains(k));

        (
            values,
            form_state.field_options.clone(),
            form_state.composite_values.clone(),
            form_state.callout_overrides.clone(),
            form_state.callout_titles.clone(),
        )
    };

    // Clear validation errors
    if let Some(ref mut fs) = app.form_state {
        fs.validation_errors.clear();
    }
    let transport_mode = app.transport.mode();

    // Capture current time once so all engine calls use the same instant.
    let now_local = chrono::Local::now();
    let now_utc = chrono::Utc::now();

    // Auto-create bare notes for novel dynamic_select values (best-effort, before main write)
    let today = now_local.format("%Y-%m-%d").to_string();
    let auto_created = crate::autocreate::run(
        module,
        &field_values,
        &field_options,
        &app.transport,
        cache,
        &today,
        &mut app.deferred_stderr,
    )
    .await;

    // Execute write based on module mode
    let date_fmt = app.config.vault.date_format.as_deref();
    let write_result = match module.mode {
        WriteMode::Create => {
            output::write_create(
                &app.transport,
                module,
                &field_values,
                &composite_data,
                date_fmt,
                &callout_overrides,
                &callout_titles,
                now_local,
            )
            .await
        }
        WriteMode::Append => {
            output::write_append(
                &app.transport,
                module,
                &field_values,
                &composite_data,
                date_fmt,
                &callout_overrides,
                &callout_titles,
                now_local,
            )
            .await
        }
    };

    // Transition to summary screen
    match write_result {
        Ok(vault_path) => {
            // Record successful capture in history
            let first_field = module
                .fields
                .first()
                .and_then(|f| field_values.get(&f.name))
                .map(|v| v.as_str());
            let history_warning =
                match app
                    .history
                    .record(&module_key, &vault_path, first_field, now_utc)
                {
                    Ok(_id) => None,
                    Err(e) => Some(format!(" (Warning: history not recorded: {e})")),
                };

            let mut summary_message = "Entry saved successfully.".to_string();
            if let Some(w) = history_warning {
                summary_message.push_str(&w);
            }

            app.summary_state = Some(SummaryState {
                message: summary_message,
                file_path: Some(vault_path),
                transport_mode,
                auto_created_notes: auto_created,
            });
        }
        Err(e) => {
            app.summary_state = Some(SummaryState {
                message: format!("Write failed: {e}"),
                file_path: None,
                transport_mode,
                auto_created_notes: auto_created,
            });
        }
    }

    app.form_state = None;
    app.screen = Screen::Summary;

    // Persist cache after write (best-effort)
    let _ = cache.save();
}

/// Handle a CreateFromTemplate action: create a templated note from the sub-form,
/// then close the sub-form and set the parent field value on success.
async fn handle_create_from_template(
    app: &mut App,
    cache: &mut Cache,
    field_name: &str,
    template_name: &str,
    note_name: &str,
    sub_form_values: &std::collections::HashMap<String, String>,
) {
    let template = match app
        .config
        .templates
        .as_ref()
        .and_then(|t| t.get(template_name))
    {
        Some(t) => t,
        None => {
            if let Some(ref mut fs) = app.form_state
                && let Some(ref mut sf) = fs.sub_form
            {
                sf.error_message = Some(format!("template '{template_name}' not found"));
            }
            return;
        }
    };

    let now_local = chrono::Local::now();
    let today = now_local.format("%Y-%m-%d").to_string();

    // Resolve the vault path from the template pattern
    let vault_path =
        match crate::autocreate::resolve_template_path(&template.path, note_name, now_local) {
            Some(p) => p,
            None => {
                if let Some(ref mut fs) = app.form_state
                    && let Some(ref mut sf) = fs.sub_form
                {
                    sf.error_message = Some(format!("failed to resolve path for '{note_name}'"));
                }
                return;
            }
        };

    // Build note content from template + sub-form values
    let content = crate::autocreate::build_templated_note_content(
        template,
        note_name,
        sub_form_values,
        &today,
    );

    // Look up post_create_command before the mutable borrow dance
    let post_command = {
        let module_key = app.module_keys.get(app.selected_module).cloned();
        module_key
            .as_ref()
            .and_then(|mk| app.config.modules.get(mk))
            .and_then(|m| m.fields.iter().find(|f| f.name == field_name))
            .and_then(|f| f.post_create_command.clone())
    };

    // Collect template-field option appends that need to persist to disk.
    // This is safe to compute now because `template` borrows from `app.config`
    // which we'll release before the mutable operations below.
    let template_option_appends: Vec<(usize, String)> = template
        .fields
        .iter()
        .enumerate()
        .filter_map(|(idx, tf)| {
            if tf.field_type != crate::config::TemplateFieldType::StaticSelect
                || !tf.allow_create.unwrap_or(false)
            {
                return None;
            }
            let value = sub_form_values.get(&tf.name)?.trim().to_string();
            if value.is_empty() {
                return None;
            }
            let already = tf
                .options
                .as_ref()
                .map(|opts| opts.iter().any(|o| o == &value))
                .unwrap_or(false);
            if already { None } else { Some((idx, value)) }
        })
        .collect();
    let template_name_owned = template_name.to_string();

    // Write via transport (best-effort)
    match app.transport.create_file(&vault_path, &content).await {
        Ok(()) => {
            // Fire post-creation command hook (best-effort). The note was already
            // created, so a hook failure does not block the user — swallow silently.
            if let Some(ref cmd) = post_command {
                let _ = app.transport.execute_command(cmd).await;
            }

            // Persist novel template-field options (both in-memory and on-disk).
            for (field_idx, value) in &template_option_appends {
                if let Some(templates) = app.config.templates.as_mut()
                    && let Some(tpl) = templates.get_mut(&template_name_owned)
                    && let Some(tf) = tpl.fields.get_mut(*field_idx)
                {
                    tf.options.get_or_insert_with(Vec::new).push(value.clone());
                }
                let _ = crate::config::Config::append_option_to_template_field_on_disk(
                    &template_name_owned,
                    *field_idx,
                    value,
                );
            }

            // Update cache: derive source from the field config
            let module_key = app.module_keys.get(app.selected_module).cloned();
            if let Some(ref mk) = module_key
                && let Some(module) = app.config.modules.get(mk)
                && let Some(field) = module.fields.iter().find(|f| f.name == field_name)
                && let Some(ref source) = field.source
            {
                let stem = crate::autocreate::sanitize_filename(note_name)
                    .unwrap_or_else(|| note_name.to_string());
                let mut cached = cache.get(source).unwrap_or_default();
                if !crate::autocreate::is_existing_option(&stem, &cached) {
                    cached.push(stem.clone());
                    cache.set(source, cached);
                }
                // Also add to live field_options so it appears in the dropdown
                if let Some(ref mut fs) = app.form_state {
                    let opts = fs.field_options.entry(field_name.to_string()).or_default();
                    if !crate::autocreate::is_existing_option(&stem, opts) {
                        opts.push(stem);
                    }
                }
            }

            // Close sub-form and set parent field value
            if let Some(ref mut fs) = app.form_state {
                fs.field_values
                    .insert(field_name.to_string(), note_name.to_string());
                fs.sub_form = None;
            }

            // Persist cache (best-effort)
            let _ = cache.save();
        }
        Err(e) => {
            // Sub-form stays open so the user can retry or cancel
            if let Some(ref mut fs) = app.form_state
                && let Some(ref mut sf) = fs.sub_form
            {
                sf.error_message = Some(format!("write failed: {e}"));
            }
        }
    }
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
            let updates = build_field_updates(&state.settings);
            Config::update_field_on_disk(&module_key, field_idx, &updates)
        }
        ConfigureLevel::SubFieldEditor(field_idx, sub_idx) => {
            let updates = build_sub_field_updates(&state.settings);
            Config::update_sub_field_on_disk(&module_key, field_idx, sub_idx, &updates)
        }
        ConfigureLevel::VaultSettings => {
            // Pre-validate vault settings before attempting disk write
            if let Err(msg) = validate_vault_settings(&state.settings) {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(msg);
                }
                return;
            }
            let updates = build_vault_updates(&state.settings);
            Config::update_vault_on_disk(&updates)
        }
        _ => {
            // Build ModuleUpdates from the current settings
            let updates = build_module_updates(&state.settings);
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

                // Rebuild sub-field settings from the fresh config
                if let ConfigureLevel::SubFieldEditor(fi, si) = level
                    && let Some(sub) = app
                        .config
                        .modules
                        .get(&module_key)
                        .and_then(|m| m.fields.get(fi))
                        .and_then(|f| f.sub_fields.as_ref())
                        .and_then(|s| s.get(si))
                    && let Some(ref mut state) = app.configure_state
                {
                    state.settings = App::build_sub_field_settings(sub);
                }

                // Rebuild vault settings from the fresh config and reconnect transport
                if level == ConfigureLevel::VaultSettings {
                    let vault = &app.config.vault;
                    // Read api_key from the raw config file rather than the in-memory
                    // value, which may carry a POUR_API_KEY env-var override. Without
                    // this guard a second save would write the env-var value to disk.
                    // Always show the persisted value, never the env-var override.
                    // secrets.toml is authoritative; config.toml is the legacy fallback.
                    let api_key_display = crate::config::Config::read_secret_api_key()
                        .or_else(|| {
                            std::fs::read_to_string(crate::config::Config::default_config_path())
                                .ok()
                                .and_then(|content| {
                                    let doc = content.parse::<toml_edit::DocumentMut>().ok()?;
                                    doc.get("vault")?.get("api_key")?.as_str().map(String::from)
                                })
                        })
                        .unwrap_or_default();
                    if let Some(ref mut s) = app.configure_state {
                        s.settings = vec![
                            crate::app::ConfigSetting {
                                label: "Base Path".to_string(),
                                key: "base_path".to_string(),
                                value: vault.base_path.clone(),
                                kind: crate::app::SettingKind::Text,
                            },
                            crate::app::ConfigSetting {
                                label: "API Port".to_string(),
                                key: "api_port".to_string(),
                                value: vault.api_port.map(|p| p.to_string()).unwrap_or_default(),
                                kind: crate::app::SettingKind::Text,
                            },
                            crate::app::ConfigSetting {
                                label: "API Key".to_string(),
                                key: "api_key".to_string(),
                                value: api_key_display,
                                kind: crate::app::SettingKind::Text,
                            },
                            crate::app::ConfigSetting {
                                label: "Date Format".to_string(),
                                key: "date_format".to_string(),
                                value: vault.date_format.clone().unwrap_or_default(),
                                kind: crate::app::SettingKind::Text,
                            },
                        ];
                    }
                    // Reconnect transport with updated vault settings
                    app.transport = crate::transport::Transport::connect(&app.config).await;
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
        callout: None,
        callout_title: None,
        allow_create: None,
        wikilink: None,
        create_template: None,
        post_create_command: None,
        show_when: None,
        icon: None,
        preset_exclude: None,
        list: false,
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
                app.module_keys
                    .retain(|k| app.config.modules.contains_key(k.as_str()));

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
fn handle_reorder_modules(app: &mut App, dir: crate::tui::dashboard::MoveDirection) {
    let idx = app.selected_module;
    let new_idx = match dir {
        crate::tui::dashboard::MoveDirection::Up => {
            if idx == 0 {
                return;
            }
            idx - 1
        }
        crate::tui::dashboard::MoveDirection::Down => {
            if idx + 1 >= app.module_keys.len() {
                return;
            }
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
            "display_name" if !setting.value.is_empty() => {
                display_name = Some(setting.value.clone());
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
        callout_type: None,
        icon: None,
        daily_link: None,
        append_shallow: None,
        mobile_visible: None,
        preset_axes: Vec::new(),
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
            callout: None,
            callout_title: None,
            allow_create: None,
            wikilink: None,
            create_template: None,
            post_create_command: None,
            show_when: None,
            icon: None,
            preset_exclude: None,
            list: false,
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

/// Add a new default sub-field to a composite_array field and open its editor.
fn handle_add_sub_field(app: &mut App, field_index: usize) {
    let module_key = match &app.configure_state {
        Some(s) => s.module_key.clone(),
        None => return,
    };

    let new_sub = SubFieldConfig {
        name: "new_column".to_string(),
        field_type: SubFieldType::Text,
        prompt: "New column".to_string(),
        options: None,
    };

    match Config::add_sub_field_on_disk(&module_key, field_index, &new_sub) {
        Ok(()) => match Config::load() {
            Ok(new_config) => {
                app.config = new_config;

                // Find the index of the newly added sub-field (last in the list)
                let sub_idx = app
                    .config
                    .modules
                    .get(&module_key)
                    .and_then(|m| m.fields.get(field_index))
                    .and_then(|f| f.sub_fields.as_ref())
                    .map(|s| s.len().saturating_sub(1))
                    .unwrap_or(0);

                // Open the sub-field editor for the new sub-field
                if let Some(sub) = app
                    .config
                    .modules
                    .get(&module_key)
                    .and_then(|m| m.fields.get(field_index))
                    .and_then(|f| f.sub_fields.as_ref())
                    .and_then(|s| s.get(sub_idx))
                    && let Some(ref mut s) = app.configure_state
                {
                    s.settings = App::build_sub_field_settings(sub);
                    s.level = ConfigureLevel::SubFieldEditor(field_index, sub_idx);
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

/// Remove a sub-field at the given indices from a composite_array field.
fn handle_remove_sub_field(app: &mut App, field_index: usize, sub_field_index: usize) {
    let module_key = match &app.configure_state {
        Some(s) => s.module_key.clone(),
        None => return,
    };

    match Config::remove_sub_field_on_disk(&module_key, field_index, sub_field_index) {
        Ok(()) => match Config::load() {
            Ok(new_config) => {
                app.config = new_config;

                // Stay on the sub-field list; clamp active_field if the last item was removed
                if let Some(ref mut s) = app.configure_state {
                    let new_sub_count = app
                        .config
                        .modules
                        .get(&module_key)
                        .and_then(|m| m.fields.get(field_index))
                        .and_then(|f| f.sub_fields.as_ref())
                        .map(|sf| sf.len())
                        .unwrap_or(0);
                    // index 0 is "< Back", sub-fields start at 1
                    let max_idx = new_sub_count;
                    if s.active_field > max_idx {
                        s.active_field = max_idx;
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

/// Swap two sub-fields (at indices a and b) within a composite_array field and persist to disk.
fn handle_reorder_sub_fields(app: &mut App, field_index: usize, a: usize, b: usize) {
    let (module_key, original_active) = match &app.configure_state {
        Some(s) => (s.module_key.clone(), s.active_field),
        None => return,
    };

    match Config::swap_sub_fields_on_disk(&module_key, field_index, a, b) {
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
                // Restore cursor to its pre-reorder position
                s.active_field = original_active;
                s.status_message = Some(format!("Reorder failed: {e}"));
            }
        }
    }
}

/// Fetch a directory listing and populate the browser state.
async fn handle_browse(app: &mut App, path: &str) {
    // Clear stale error immediately so the previous failure doesn't linger
    // on screen while the new listing is in flight.
    if let Some(ref mut state) = app.configure_state
        && let Some(ref mut browser) = state.browser_state
    {
        browser.error = None;
    }

    let (entries, error) = match app.transport.list_directory_entries(path).await {
        Ok(e) => (e, None),
        Err(e) => (Vec::new(), Some(format!("pour: browse error: {e}"))),
    };

    if let Some(ref mut state) = app.configure_state {
        state.browser_state = Some(BrowserState {
            current_path: path.to_string(),
            entries,
            selected: 0,
            error,
        });
        state.browser_open = true;
    }
}

/// Fetch dynamic select options for all dynamic_select fields in a module.
pub async fn fetch_dynamic_options(app: &mut App, module_key: &str, cache: &mut Cache) {
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

/// Build an `obsidian://open` URI for the given vault and optional file path.
fn obsidian_uri(vault_base_path: &str, file_path: Option<&str>) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

    let vault_name = std::path::Path::new(vault_base_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("vault");
    let encoded_vault = utf8_percent_encode(vault_name, NON_ALPHANUMERIC);

    match file_path {
        Some(path) => {
            let clean = path.strip_suffix(".md").unwrap_or(path);
            let encoded_path = utf8_percent_encode(clean, NON_ALPHANUMERIC);
            format!("obsidian://open?vault={encoded_vault}&file={encoded_path}")
        }
        None => format!("obsidian://open?vault={encoded_vault}"),
    }
}
