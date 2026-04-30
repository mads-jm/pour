use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, ConfigureLevel, PendingConfirm};

use super::super::ConfigureAction;

/// Handle keys when in `FieldList` mode.
pub(super) fn handle_field_list(app: &mut App, key: crossterm::event::KeyEvent) -> ConfigureAction {
    let state = match &mut app.configure_state {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    // Ctrl+Up / Ctrl+Down: reorder fields
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        let field_count = app
            .config
            .modules
            .get(&state.module_key)
            .map(|m| m.fields.len())
            .unwrap_or(0);

        // Re-borrow after immutable config access
        let state = match &mut app.configure_state {
            Some(s) => s,
            None => return ConfigureAction::None,
        };

        match key.code {
            KeyCode::Up => {
                // active_field > 1: not on Back row and not first field
                if state.active_field > 1 {
                    let a = state.active_field - 2; // field index of item above
                    let b = state.active_field - 1; // field index of current
                    state.active_field -= 1;
                    return ConfigureAction::ReorderFields(a, b);
                }
                return ConfigureAction::None;
            }
            KeyCode::Down => {
                // active_field > 0 (not on Back) and not last field
                if state.active_field > 0 && state.active_field < field_count {
                    let a = state.active_field - 1; // field index of current
                    let b = state.active_field; // field index of item below
                    state.active_field += 1;
                    return ConfigureAction::ReorderFields(a, b);
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    let module_key = state.module_key.clone();
    let field_count = app
        .config
        .modules
        .get(&module_key)
        .map(|m| m.fields.len())
        .unwrap_or(0);
    // total items = 1 ("< Back") + field_count
    let total = 1 + field_count;

    let state = match &mut app.configure_state {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    match key.code {
        KeyCode::Esc => {
            // Back to module settings
            state.level = ConfigureLevel::ModuleSettings;
            state.active_field = 0;
            ConfigureAction::None
        }
        KeyCode::Up => {
            if state.active_field > 0 {
                state.active_field -= 1;
            }
            ConfigureAction::None
        }
        KeyCode::Down => {
            if state.active_field + 1 < total {
                state.active_field += 1;
            }
            ConfigureAction::None
        }
        KeyCode::Enter => {
            if state.active_field == 0 {
                // "< Back" row
                state.level = ConfigureLevel::ModuleSettings;
                state.active_field = 0;
            } else {
                // Select a field — transition to FieldEditor
                let field_idx = state.active_field - 1;
                if let Some(field) = app
                    .config
                    .modules
                    .get(&module_key)
                    .and_then(|m| m.fields.get(field_idx))
                {
                    let settings = crate::app::App::build_field_settings(field);
                    if let Some(ref mut s) = app.configure_state {
                        s.settings = settings;
                        s.level = ConfigureLevel::FieldEditor(field_idx);
                        s.active_field = 0;
                    }
                    return ConfigureAction::None;
                }
            }
            ConfigureAction::None
        }
        KeyCode::Char('n') => {
            // Add a new field
            ConfigureAction::AddField
        }
        KeyCode::Char('d') => {
            // Delete selected field (requires confirmation)
            if state.active_field > 0 {
                let field_idx = state.active_field - 1;
                let field_name = app
                    .config
                    .modules
                    .get(&module_key)
                    .and_then(|m| m.fields.get(field_idx))
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| "?".to_string());
                if let Some(ref mut s) = app.configure_state {
                    s.confirm = Some(PendingConfirm::DeleteField {
                        field_index: field_idx,
                        field_name,
                    });
                }
            }
            ConfigureAction::None
        }
        _ => ConfigureAction::None,
    }
}
