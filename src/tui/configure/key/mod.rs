pub(super) mod fields;
pub(super) mod modules;
pub(super) mod sub_fields;
pub(super) mod vault;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, ConfigureLevel, PendingConfirm, SettingKind};

use super::ConfigureAction;

/// Handle a key event on the configure screen.
///
/// Returns a `ConfigureAction` that the wiring layer should act on.
pub fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) -> ConfigureAction {
    let state = match &mut app.configure_state {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    // --- Confirmation dialog mode ---
    if let Some(ref pending) = state.confirm.clone() {
        match key.code {
            KeyCode::Char('y') => {
                state.confirm = None;
                match pending {
                    PendingConfirm::DeleteField { field_index, .. } => {
                        return ConfigureAction::RemoveField(*field_index);
                    }
                    PendingConfirm::DeleteModule { .. } => {
                        return ConfigureAction::DeleteModule;
                    }
                    PendingConfirm::DeleteSubField {
                        field_index,
                        sub_field_index,
                        ..
                    } => {
                        return ConfigureAction::RemoveSubField(*field_index, *sub_field_index);
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                state.confirm = None;
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- Path help overlay mode ---
    if state.help_overlay_open {
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => {
                state.help_overlay_open = false;
            }
            _ => {}
        }
        return ConfigureAction::None;
    }

    // --- Quick-select overlay mode ---
    if state.quick_select_open {
        match key.code {
            KeyCode::Esc => {
                state.quick_select_open = false;
            }
            KeyCode::Backspace => {
                // Clear selection
                state.settings[state.active_field].value = String::new();
                state.dirty = true;
                state.quick_select_open = false;
            }
            KeyCode::Char(c) => {
                // Match hotkey against the options
                if let SettingKind::QuickSelect(ref options) =
                    state.settings[state.active_field].kind
                    && let Some((_, label)) = options.iter().find(|(k, _)| *k == c)
                {
                    state.settings[state.active_field].value = label.clone();
                    state.dirty = true;
                    state.quick_select_open = false;
                }
            }
            _ => {}
        }
        return ConfigureAction::None;
    }

    // --- Browser mode ---
    if state.browser_open {
        let browser = match &mut state.browser_state {
            Some(b) => b,
            None => {
                // No state yet; only Esc works
                if key.code == KeyCode::Esc {
                    state.browser_open = false;
                }
                return ConfigureAction::None;
            }
        };

        let at_root = browser.current_path.is_empty() || browser.current_path == "/";

        let dirs: Vec<String> = browser
            .entries
            .iter()
            .filter(|e| e.is_dir)
            .map(|e| e.name.clone())
            .collect();

        let total = if at_root { dirs.len() } else { dirs.len() + 1 };

        match key.code {
            KeyCode::Up => {
                if total > 0 && browser.selected > 0 {
                    browser.selected -= 1;
                }
                return ConfigureAction::None;
            }
            KeyCode::Down => {
                if total > 0 && browser.selected + 1 < total {
                    browser.selected += 1;
                }
                return ConfigureAction::None;
            }
            KeyCode::Esc => {
                state.browser_open = false;
                return ConfigureAction::None;
            }
            KeyCode::Backspace => {
                // Go up one level
                let parent = parent_path(&browser.current_path);
                return ConfigureAction::BrowseDirectory(parent);
            }
            KeyCode::Enter => {
                let selected = browser.selected;
                let current_path = browser.current_path.clone();

                if !at_root && selected == 0 {
                    // ".." — go up
                    let parent = parent_path(&current_path);
                    return ConfigureAction::BrowseDirectory(parent);
                }

                let dir_idx = if at_root { selected } else { selected - 1 };
                if let Some(name) = dirs.get(dir_idx) {
                    let new_path = if current_path.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{}", current_path.trim_end_matches('/'), name)
                    };
                    return ConfigureAction::BrowseDirectory(new_path);
                }
                return ConfigureAction::None;
            }
            KeyCode::Tab => {
                // Select current directory as the path value
                let selected = browser.selected;
                let current_path = browser.current_path.clone();
                let at_root_local = current_path.is_empty() || current_path == "/";

                let chosen_dir = if !at_root_local && selected == 0 {
                    // ".." selected → use parent
                    parent_path(&current_path)
                } else {
                    let dir_idx = if at_root_local {
                        selected
                    } else {
                        selected - 1
                    };
                    if let Some(name) = dirs.get(dir_idx) {
                        if current_path.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", current_path.trim_end_matches('/'), name)
                        }
                    } else {
                        // Nothing selected — just use current directory
                        current_path
                    }
                };

                // Snapshot the active setting key and configure level before
                // we take a mutable borrow of settings.
                let active_setting_key = state
                    .settings
                    .get(state.active_field)
                    .map(|s| s.key.clone())
                    .unwrap_or_default();
                let level = state.level.clone();

                // Auto-append /{date_format}.md when the browser selects a
                // directory for a module path, so the user starts with a
                // sensible date-based filename template to tweak.
                let chosen_path = if active_setting_key == "path"
                    && matches!(level, ConfigureLevel::ModuleSettings)
                {
                    let date_fmt = app.config.vault.date_format.as_deref().unwrap_or("%Y%m%d");
                    format!("{}/{}.md", chosen_dir.trim_end_matches('/'), date_fmt)
                } else {
                    chosen_dir
                };

                // Apply to the active Path setting and transition to
                // freetext edit so the user can tweak the filename template
                // (e.g. append `{{bean}} {{date}}.md`).
                // Re-borrow configure_state mutably after the immutable app.config
                // access above is complete.
                let is_module_path =
                    active_setting_key == "path" && matches!(level, ConfigureLevel::ModuleSettings);
                if let Some(state) = &mut app.configure_state {
                    if let Some(setting) = state.settings.get_mut(state.active_field) {
                        setting.value = chosen_path.clone();
                        state.dirty = true;
                    }
                    state.browser_open = false;

                    // For module path settings, drop into freetext edit so
                    // the user can append a filename template after browsing.
                    if is_module_path {
                        // Ensure trailing slash so cursor is ready for filename entry
                        let mut path = chosen_path;
                        if !path.ends_with('/') && !path.contains('.') {
                            path.push('/');
                        }
                        state.edit_original = path.clone();
                        state.edit_buffer = path.clone();
                        state.cursor_position = path.chars().count();
                        state.editing = true;
                    }
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- List editor overlay mode ---
    if state.list_editor_open {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                // Save: write buffer back to the setting value
                let buf = state.list_editor_buffer.clone();
                if let Some(setting) = state.settings.get_mut(state.active_field) {
                    setting.value = buf;
                    state.dirty = true;
                }
                state.list_editor_open = false;
                state.list_editor_buffer.clear();
                state.list_editor_cursor_line = 0;
                state.list_editor_cursor_col = 0;
                return ConfigureAction::None;
            }
            (_, KeyCode::Esc) => {
                // Cancel — discard changes
                state.list_editor_open = false;
                state.list_editor_buffer.clear();
                state.list_editor_cursor_line = 0;
                state.list_editor_cursor_col = 0;
                return ConfigureAction::None;
            }
            (_, KeyCode::Enter) => {
                // Insert newline at cursor position
                let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                let line_idx = state
                    .list_editor_cursor_line
                    .min(lines.len().saturating_sub(1));
                let col = state.list_editor_cursor_col;

                // Find byte offset for the cursor position
                let mut byte_offset = 0;
                for (i, line) in state.list_editor_buffer.lines().enumerate() {
                    if i == line_idx {
                        byte_offset += col.min(line.len());
                        break;
                    }
                    byte_offset += line.len() + 1; // +1 for '\n'
                }
                // Handle empty buffer or cursor at end
                byte_offset = byte_offset.min(state.list_editor_buffer.len());
                state.list_editor_buffer.insert(byte_offset, '\n');
                state.list_editor_cursor_line += 1;
                state.list_editor_cursor_col = 0;
                return ConfigureAction::None;
            }
            (_, KeyCode::Char(c)) => {
                let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                let line_idx = state
                    .list_editor_cursor_line
                    .min(lines.len().saturating_sub(1));
                let col = state.list_editor_cursor_col;

                let mut byte_offset = 0;
                for (i, line) in state.list_editor_buffer.lines().enumerate() {
                    if i == line_idx {
                        byte_offset += col.min(line.len());
                        break;
                    }
                    byte_offset += line.len() + 1;
                }
                byte_offset = byte_offset.min(state.list_editor_buffer.len());
                state.list_editor_buffer.insert(byte_offset, c);
                state.list_editor_cursor_col += 1;
                return ConfigureAction::None;
            }
            (_, KeyCode::Backspace) => {
                if state.list_editor_cursor_col > 0 {
                    let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                    let line_idx = state
                        .list_editor_cursor_line
                        .min(lines.len().saturating_sub(1));
                    let col = state.list_editor_cursor_col;

                    let mut byte_offset = 0;
                    for (i, line) in state.list_editor_buffer.lines().enumerate() {
                        if i == line_idx {
                            byte_offset += (col - 1).min(line.len());
                            break;
                        }
                        byte_offset += line.len() + 1;
                    }
                    if byte_offset < state.list_editor_buffer.len() {
                        state.list_editor_buffer.remove(byte_offset);
                    }
                    state.list_editor_cursor_col -= 1;
                } else if state.list_editor_cursor_line > 0 {
                    // Merge with previous line
                    let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                    let prev_line_len = lines
                        .get(state.list_editor_cursor_line - 1)
                        .map(|l| l.len())
                        .unwrap_or(0);

                    // Find the newline byte offset at end of previous line
                    let mut byte_offset = 0;
                    for (i, line) in state.list_editor_buffer.lines().enumerate() {
                        if i == state.list_editor_cursor_line - 1 {
                            byte_offset += line.len();
                            break;
                        }
                        byte_offset += line.len() + 1;
                    }
                    if byte_offset < state.list_editor_buffer.len() {
                        state.list_editor_buffer.remove(byte_offset); // remove '\n'
                    }
                    state.list_editor_cursor_line -= 1;
                    state.list_editor_cursor_col = prev_line_len;
                }
                return ConfigureAction::None;
            }
            (_, KeyCode::Up) => {
                if state.list_editor_cursor_line > 0 {
                    state.list_editor_cursor_line -= 1;
                    let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                    let line_len = lines
                        .get(state.list_editor_cursor_line)
                        .map(|l| l.len())
                        .unwrap_or(0);
                    state.list_editor_cursor_col = state.list_editor_cursor_col.min(line_len);
                }
                return ConfigureAction::None;
            }
            (_, KeyCode::Down) => {
                let line_count = state.list_editor_buffer.lines().count().max(1);
                if state.list_editor_cursor_line + 1 < line_count {
                    state.list_editor_cursor_line += 1;
                    let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                    let line_len = lines
                        .get(state.list_editor_cursor_line)
                        .map(|l| l.len())
                        .unwrap_or(0);
                    state.list_editor_cursor_col = state.list_editor_cursor_col.min(line_len);
                }
                return ConfigureAction::None;
            }
            (_, KeyCode::Left) => {
                if state.list_editor_cursor_col > 0 {
                    state.list_editor_cursor_col -= 1;
                }
                return ConfigureAction::None;
            }
            (_, KeyCode::Right) => {
                let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                let line_len = lines
                    .get(state.list_editor_cursor_line)
                    .map(|l| l.len())
                    .unwrap_or(0);
                if state.list_editor_cursor_col < line_len {
                    state.list_editor_cursor_col += 1;
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- Editing mode ---
    if state.editing {
        let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
        match key.code {
            KeyCode::Enter => {
                // Confirm edit
                let buf = state.edit_buffer.clone();
                if let Some(setting) = state.settings.get_mut(state.active_field) {
                    setting.value = buf;
                    state.dirty = true;
                }
                state.editing = false;
                state.edit_buffer.clear();
                state.edit_original.clear();
                state.cursor_position = 0;
                state.scroll_offset = 0;
                return ConfigureAction::None;
            }
            KeyCode::Esc => {
                // Cancel edit — restore original
                if let Some(setting) = state.settings.get_mut(state.active_field) {
                    setting.value = state.edit_original.clone();
                }
                state.editing = false;
                state.edit_buffer.clear();
                state.edit_original.clear();
                state.cursor_position = 0;
                state.scroll_offset = 0;
                return ConfigureAction::None;
            }
            KeyCode::Char(c) => {
                // '?' on Path fields opens the placeholder help overlay
                if c == '?'
                    && matches!(
                        state.settings.get(state.active_field).map(|s| &s.kind),
                        Some(SettingKind::Path)
                    )
                {
                    state.help_overlay_open = true;
                    return ConfigureAction::None;
                }
                // Identifier fields: reject characters that aren't TOML-key-safe
                if matches!(
                    state.settings.get(state.active_field).map(|s| &s.kind),
                    Some(SettingKind::Identifier)
                ) && !(c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return ConfigureAction::None;
                }
                // Use char indices for correct Unicode handling
                let byte_pos = state
                    .edit_buffer
                    .char_indices()
                    .nth(state.cursor_position)
                    .map(|(i, _)| i)
                    .unwrap_or(state.edit_buffer.len());
                state.edit_buffer.insert(byte_pos, c);
                state.cursor_position += 1;
                super::sync_scroll_offset(state, term_cols);
                return ConfigureAction::None;
            }
            KeyCode::Backspace => {
                if state.cursor_position > 0 {
                    let char_count = state.edit_buffer.chars().count();
                    let pos = state.cursor_position.min(char_count);
                    let byte_pos = state
                        .edit_buffer
                        .char_indices()
                        .nth(pos - 1)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    state.edit_buffer.remove(byte_pos);
                    state.cursor_position = pos - 1;
                    super::sync_scroll_offset(state, term_cols);
                }
                return ConfigureAction::None;
            }
            KeyCode::Left => {
                if state.cursor_position > 0 {
                    state.cursor_position -= 1;
                    super::sync_scroll_offset(state, term_cols);
                }
                return ConfigureAction::None;
            }
            KeyCode::Right => {
                let char_count = state.edit_buffer.chars().count();
                if state.cursor_position < char_count {
                    state.cursor_position += 1;
                    super::sync_scroll_offset(state, term_cols);
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- Mode dispatch ---
    let level = app
        .configure_state
        .as_ref()
        .map(|s| s.level.clone())
        .unwrap_or(ConfigureLevel::ModuleSettings);

    match level {
        ConfigureLevel::FieldList => fields::handle_field_list(app, key),
        ConfigureLevel::SubFieldList(field_idx) => {
            sub_fields::handle_sub_field_list(app, key, field_idx)
        }
        _ => vault::handle_settings(app, key),
    }
}

/// Compute the parent directory of a vault-relative path.
/// Returns an empty string if already at the root.
pub(super) fn parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if let Some(pos) = trimmed.rfind('/') {
        trimmed[..pos].to_string()
    } else {
        String::new()
    }
}

/// Get the directory portion of a vault-relative file path.
/// If the path contains no slash, returns an empty string (vault root).
pub(super) fn dir_of(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if let Some(pos) = trimmed.rfind('/') {
        trimmed[..pos].to_string()
    } else {
        String::new()
    }
}
