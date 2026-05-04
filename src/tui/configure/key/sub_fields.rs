use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, ConfigureLevel, PendingConfirm};

use super::super::ConfigureAction;

/// Handle keys when in `SubFieldList` mode.
pub(super) fn handle_sub_field_list(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    field_idx: usize,
) -> ConfigureAction {
    let state = match &mut app.configure_state {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    // Ctrl+Up / Ctrl+Down: reorder sub-fields
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        let sub_count = app
            .config
            .modules
            .get(&state.module_key)
            .and_then(|m| m.fields.get(field_idx))
            .and_then(|f| f.sub_fields.as_ref())
            .map(|s| s.len())
            .unwrap_or(0);

        let state = match &mut app.configure_state {
            Some(s) => s,
            None => return ConfigureAction::None,
        };

        match key.code {
            KeyCode::Up => {
                if state.active_field > 1 {
                    let a = state.active_field - 2;
                    let b = state.active_field - 1;
                    state.active_field -= 1;
                    return ConfigureAction::ReorderSubFields(field_idx, a, b);
                }
                return ConfigureAction::None;
            }
            KeyCode::Down => {
                if state.active_field > 0 && state.active_field < sub_count {
                    let a = state.active_field - 1;
                    let b = state.active_field;
                    state.active_field += 1;
                    return ConfigureAction::ReorderSubFields(field_idx, a, b);
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    let module_key = state.module_key.clone();
    let sub_count = app
        .config
        .modules
        .get(&module_key)
        .and_then(|m| m.fields.get(field_idx))
        .and_then(|f| f.sub_fields.as_ref())
        .map(|s| s.len())
        .unwrap_or(0);
    let total = 1 + sub_count;

    let state = match &mut app.configure_state {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    match key.code {
        KeyCode::Esc => {
            // Back to field editor
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
            if let Some(ref mut s) = app.configure_state {
                s.level = ConfigureLevel::FieldEditor(field_idx);
                s.active_field = 0;
            }
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
                // "< Back" row — go back to field editor
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
                if let Some(ref mut s) = app.configure_state {
                    s.level = ConfigureLevel::FieldEditor(field_idx);
                    s.active_field = 0;
                }
            } else {
                // Select a sub-field — transition to SubFieldEditor
                let sub_idx = state.active_field - 1;
                if let Some(sub) = app
                    .config
                    .modules
                    .get(&module_key)
                    .and_then(|m| m.fields.get(field_idx))
                    .and_then(|f| f.sub_fields.as_ref())
                    .and_then(|s| s.get(sub_idx))
                {
                    let settings = crate::app::App::build_sub_field_settings(sub);
                    if let Some(ref mut s) = app.configure_state {
                        s.settings = settings;
                        s.level = ConfigureLevel::SubFieldEditor(field_idx, sub_idx);
                        s.active_field = 0;
                    }
                    return ConfigureAction::None;
                }
            }
            ConfigureAction::None
        }
        KeyCode::Char('n') => ConfigureAction::AddSubField(field_idx),
        KeyCode::Char('d') => {
            if state.active_field > 0 {
                let sub_idx = state.active_field - 1;
                let sub_name = app
                    .config
                    .modules
                    .get(&module_key)
                    .and_then(|m| m.fields.get(field_idx))
                    .and_then(|f| f.sub_fields.as_ref())
                    .and_then(|s| s.get(sub_idx))
                    .map(|sf| sf.name.clone())
                    .unwrap_or_else(|| "?".to_string());
                if let Some(ref mut s) = app.configure_state {
                    s.confirm = Some(PendingConfirm::DeleteSubField {
                        field_index: field_idx,
                        sub_field_index: sub_idx,
                        sub_field_name: sub_name,
                    });
                }
            }
            ConfigureAction::None
        }
        _ => ConfigureAction::None,
    }
}
