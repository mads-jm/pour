use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, ConfigSetting, ConfigureLevel, PendingConfirm, SettingKind};

use super::super::ConfigureAction;
use super::super::autosave::{auto_save_field_settings, auto_save_module_settings};
use super::dir_of;

/// Handle keys when in any settings-list mode:
/// `ModuleSettings`, `VaultSettings`, `NewModule`, `FieldEditor`, `SubFieldEditor`.
pub(super) fn handle_settings(app: &mut App, key: crossterm::event::KeyEvent) -> ConfigureAction {
    let state = match &mut app.configure_state {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    // Ctrl+S in NewModule level → trigger save of new module
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('s')
        && state.level == ConfigureLevel::NewModule
    {
        // Validate module_key is non-empty and TOML-safe
        let module_key_val = state
            .settings
            .iter()
            .find(|s| s.key == "module_key")
            .map(|s| s.value.clone())
            .unwrap_or_default();

        if module_key_val.is_empty() {
            state.status_message = Some("Module Key must not be empty".to_string());
            return ConfigureAction::None;
        }

        let valid_key = module_key_val
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !valid_key {
            state.status_message =
                Some("Module Key: only a-z, A-Z, 0-9, _ and - are allowed".to_string());
            return ConfigureAction::None;
        }

        state.status_message = None;
        return ConfigureAction::SaveNewModule;
    }

    let setting_count = state.settings.len();

    match key.code {
        KeyCode::Esc => {
            if let ConfigureLevel::SubFieldEditor(field_idx, _) = state.level {
                // Back to sub-field list — no settings rebuild needed, SubFieldList reads from config
                state.level = ConfigureLevel::SubFieldList(field_idx);
                state.active_field = 0;
                ConfigureAction::None
            } else if let ConfigureLevel::FieldEditor(_) = state.level {
                // Back to field list, restore module-level settings
                state.level = ConfigureLevel::FieldList;
                state.active_field = 0;
                // Rebuild module-level settings since we replaced them with field settings
                let module_key = state.module_key.clone();
                if let Some(module) = app.config.modules.get(&module_key) {
                    let mode_str = match module.mode {
                        crate::config::WriteMode::Append => "append".to_string(),
                        crate::config::WriteMode::Create => "create".to_string(),
                    };
                    let mut settings = vec![
                        ConfigSetting {
                            label: "Path".to_string(),
                            key: "path".to_string(),
                            value: module.path.clone(),
                            kind: SettingKind::Path,
                        },
                        ConfigSetting {
                            label: "Display Name".to_string(),
                            key: "display_name".to_string(),
                            value: module.display_name.clone().unwrap_or_default(),
                            kind: SettingKind::Text,
                        },
                        ConfigSetting {
                            label: "Mode".to_string(),
                            key: "mode".to_string(),
                            value: mode_str.clone(),
                            kind: SettingKind::Toggle(vec![
                                "append".to_string(),
                                "create".to_string(),
                            ]),
                        },
                    ];
                    if mode_str == "append" {
                        settings.push(ConfigSetting {
                            label: "Append Header".to_string(),
                            key: "append_under_header".to_string(),
                            value: module.append_under_header.clone().unwrap_or_default(),
                            kind: SettingKind::Text,
                        });
                    }
                    let field_count = module.fields.len();
                    settings.push(ConfigSetting {
                        label: "Fields".to_string(),
                        key: "fields".to_string(),
                        value: format!(
                            "{field_count} field{}",
                            if field_count == 1 { "" } else { "s" }
                        ),
                        kind: SettingKind::NavLink,
                    });
                    if let Some(ref mut s) = app.configure_state {
                        s.settings = settings;
                    }
                }
                ConfigureAction::None
            } else {
                // ModuleSettings, VaultSettings, and NewModule all return to dashboard
                ConfigureAction::Cancel
            }
        }

        KeyCode::Up => {
            if let Some(ref mut s) = app.configure_state
                && setting_count > 0
                && s.active_field > 0
            {
                s.active_field -= 1;
            }
            ConfigureAction::None
        }

        KeyCode::Down => {
            if let Some(ref mut s) = app.configure_state
                && setting_count > 0
                && s.active_field + 1 < setting_count
            {
                s.active_field += 1;
            }
            ConfigureAction::None
        }

        // 's' saves for module/vault settings but is NOT wired for NewModule
        // (NewModule uses Ctrl+S to avoid confusion with typing 's' in an identifier)
        KeyCode::Char('s')
            if app
                .configure_state
                .as_ref()
                .map(|s| s.level != ConfigureLevel::NewModule && !s.editing)
                .unwrap_or(false) =>
        {
            ConfigureAction::Save
        }

        // 'd' on ModuleSettings: prompt to delete the entire module
        KeyCode::Char('d')
            if app
                .configure_state
                .as_ref()
                .map(|s| s.level == ConfigureLevel::ModuleSettings && !s.editing)
                .unwrap_or(false) =>
        {
            let module_key = app
                .configure_state
                .as_ref()
                .map(|s| s.module_key.clone())
                .unwrap_or_default();
            if let Some(ref mut s) = app.configure_state {
                s.confirm = Some(PendingConfirm::DeleteModule { module_key });
            }
            ConfigureAction::None
        }

        KeyCode::Char('?') => {
            // '?' on Path fields opens the placeholder help overlay
            if let Some(ref mut s) = app.configure_state
                && let Some(setting) = s.settings.get(s.active_field)
                && matches!(setting.kind, SettingKind::Path)
            {
                s.help_overlay_open = true;
            }
            ConfigureAction::None
        }

        KeyCode::Char('e') => {
            // 'e' on any field starts freetext editing (including Path and Identifier)
            if let Some(ref mut s) = app.configure_state
                && let Some(setting) = s.settings.get(s.active_field)
            {
                let orig = setting.value.clone();
                let cursor = orig.chars().count();
                s.edit_original = orig.clone();
                s.edit_buffer = orig;
                s.cursor_position = cursor;
                s.editing = true;
            }
            ConfigureAction::None
        }

        KeyCode::Enter => handle_enter(app),

        _ => ConfigureAction::None,
    }
}

/// Handle the Enter key in settings-list mode.
fn handle_enter(app: &mut App) -> ConfigureAction {
    let state = match &mut app.configure_state {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    let setting = match state.settings.get(state.active_field) {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    match &setting.kind.clone() {
        SettingKind::Path => {
            // Open vault browser at current path's directory.
            // For field-level source paths, default to the
            // module's path directory when the source is empty.
            let browse_path = if setting.value.is_empty() {
                app.config
                    .modules
                    .get(&state.module_key)
                    .map(|m| dir_of(&m.path))
                    .unwrap_or_default()
            } else {
                dir_of(&setting.value)
            };
            ConfigureAction::BrowseDirectory(browse_path)
        }
        SettingKind::Text | SettingKind::Identifier => {
            // Start freetext editing
            if let Some(ref mut s) = app.configure_state
                && let Some(setting) = s.settings.get(s.active_field)
            {
                let orig = setting.value.clone();
                let cursor = orig.chars().count();
                s.edit_original = orig.clone();
                s.edit_buffer = orig;
                s.cursor_position = cursor;
                s.editing = true;
            }
            ConfigureAction::None
        }
        SettingKind::Toggle(options) => {
            let options = options.clone();
            if let Some(ref mut s) = app.configure_state {
                let current = s.settings[s.active_field].value.clone();
                let key = s.settings[s.active_field].key.clone();
                let idx = options.iter().position(|o| *o == current);
                let next_idx = match idx {
                    Some(i) => (i + 1) % options.len(),
                    None => 0,
                };
                if let Some(next) = options.get(next_idx) {
                    let next = next.clone();
                    s.settings[s.active_field].value = next.clone();
                    s.dirty = true;

                    // Dynamically add/remove append_under_header when mode toggles
                    if key == "mode" {
                        let has_header = s.settings.iter().any(|s| s.key == "append_under_header");
                        if next == "append" && !has_header {
                            s.settings.push(ConfigSetting {
                                label: "Append Header".to_string(),
                                key: "append_under_header".to_string(),
                                value: "## Log".to_string(),
                                kind: SettingKind::Text,
                            });
                        } else if next == "create" && has_header {
                            s.settings.retain(|s| s.key != "append_under_header");
                        }
                    }

                    // Dynamically add/remove type-specific settings in field editor
                    if key == "field_type" {
                        if matches!(s.level, ConfigureLevel::SubFieldEditor(_, _)) {
                            // Sub-field editor: only options for static_select
                            s.settings.retain(|s| s.key != "options");
                            if next == "static_select" {
                                s.settings.push(ConfigSetting {
                                    label: "Options".to_string(),
                                    key: "options".to_string(),
                                    value: String::new(),
                                    kind: SettingKind::ListEditor,
                                });
                            }
                        } else {
                            // Field editor: remove all type-conditional settings
                            s.settings.retain(|s| {
                                s.key != "options"
                                    && s.key != "source"
                                    && s.key != "sub_fields"
                                    && s.key != "callout"
                            });

                            if next == "static_select" {
                                s.settings.push(ConfigSetting {
                                    label: "Options".to_string(),
                                    key: "options".to_string(),
                                    value: String::new(),
                                    kind: SettingKind::ListEditor,
                                });
                            } else if next == "dynamic_select" {
                                s.settings.push(ConfigSetting {
                                    label: "Source".to_string(),
                                    key: "source".to_string(),
                                    value: String::new(),
                                    kind: SettingKind::Path,
                                });
                            } else if next == "composite_array" {
                                s.settings.push(ConfigSetting {
                                    label: "Sub-fields".to_string(),
                                    key: "sub_fields".to_string(),
                                    value: "0 columns".to_string(),
                                    kind: SettingKind::NavLink,
                                });
                            }

                            if next == "textarea" {
                                s.settings.push(ConfigSetting {
                                    label: "Callout".to_string(),
                                    key: "callout".to_string(),
                                    value: String::new(),
                                    kind: SettingKind::QuickSelect(
                                        crate::app::callout_quick_select(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            ConfigureAction::None
        }
        SettingKind::NavLink => {
            // Navigate to the linked sub-screen.
            // Auto-save dirty settings before transitioning so
            // edits (path, mode, etc.) are not silently lost.
            let nav_key = setting.key.clone();
            let current_level = state.level.clone();
            let is_dirty = state.dirty;

            if nav_key == "fields" {
                if is_dirty {
                    auto_save_module_settings(app);
                }
                if let Some(ref mut s) = app.configure_state {
                    s.level = ConfigureLevel::FieldList;
                    s.active_field = 0;
                }
            } else if nav_key == "sub_fields"
                && let ConfigureLevel::FieldEditor(field_idx) = current_level
            {
                if is_dirty {
                    auto_save_field_settings(app, field_idx);
                }
                if let Some(ref mut s) = app.configure_state {
                    s.level = ConfigureLevel::SubFieldList(field_idx);
                    s.active_field = 0;
                }
            }
            ConfigureAction::None
        }
        SettingKind::ListEditor => {
            // Open the list editor overlay
            if let Some(ref mut s) = app.configure_state
                && let Some(setting) = s.settings.get(s.active_field)
            {
                let val = setting.value.clone();
                let line_count = val.lines().count().max(1);
                let last_line_len = val.lines().last().map(|l| l.len()).unwrap_or(0);
                s.list_editor_buffer = val;
                s.list_editor_cursor_line = line_count - 1;
                s.list_editor_cursor_col = last_line_len;
                s.list_editor_open = true;
            }
            ConfigureAction::None
        }
        SettingKind::QuickSelect(_) => {
            // Open the quick-select overlay
            if let Some(ref mut s) = app.configure_state {
                s.quick_select_open = true;
            }
            ConfigureAction::None
        }
    }
}
