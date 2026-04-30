use crossterm::event::KeyEvent;

use crate::app::{
    FieldPresetPickerState, FormState, PresetDialogFocus, PresetDialogTarget, PresetSaveDialog,
};
use crate::config::{FieldConfig, SubFieldConfig, SubFieldType};
use crate::tui::form::FormAction;

/// Handle key events while the composite array editor is open (`composite_open == true`).
///
/// The per-field preset picker overlay intercepts all keys when open.
pub(in crate::tui::form) fn handle_composite_key(
    form_state: &mut FormState,
    field: &FieldConfig,
    field_presets: &crate::data::field_presets::FieldPresets,
    module_key: &str,
    key: KeyEvent,
) -> FormAction {
    use crossterm::event::KeyCode;

    let sub_fields = match &field.sub_fields {
        Some(subs) if !subs.is_empty() => subs,
        _ => return FormAction::None,
    };
    let col_count = sub_fields.len();
    let field_name = field.name.clone();

    // Picker overlay intercepts ALL keys when open.
    if form_state.field_preset_picker.is_some() {
        return handle_field_preset_picker_key(form_state, &field_name, key);
    }

    let preset_storage_key = crate::data::field_presets::preset_key(module_key, &field_name);

    let row = form_state.composite_row;
    let col = form_state.composite_col;

    // ── per-field preset bindings ─────────────────────────────────────────────
    if key.code == KeyCode::Char('s') {
        let rows = form_state
            .composite_values
            .get(&field_name)
            .cloned()
            .unwrap_or_default();
        let has_data = rows.iter().any(|r| r.iter().any(|c| !c.is_empty()));
        if !has_data {
            form_state.composite_status = Some("nothing to save".to_string());
            return FormAction::None;
        }
        let prefill_name = form_state
            .last_applied_field_preset
            .get(&field_name)
            .cloned()
            .unwrap_or_default();
        let cursor_position = prefill_name.chars().count();
        form_state.composite_status = None;
        form_state.preset_overlay = Some(PresetSaveDialog {
            name_buffer: prefill_name,
            cursor_position,
            description_buffer: String::new(),
            description_cursor: 0,
            focus: PresetDialogFocus::Name,
            target: PresetDialogTarget::CompositeField {
                field_name: field_name.clone(),
            },
            name_was_user_edited: false,
            awaiting_overwrite_confirm: false,
        });
        return FormAction::None;
    }

    if key.code == KeyCode::Char('l') {
        form_state.composite_status = None;
        let entries = field_presets.get(&preset_storage_key);
        if entries.is_empty() {
            form_state.composite_status = Some("no presets saved for this field".to_string());
            return FormAction::None;
        }
        let names: Vec<String> = entries.iter().map(|p| p.name.clone()).collect();
        let descriptions: Vec<Option<String>> =
            entries.iter().map(|p| p.description.clone()).collect();
        let selected = form_state
            .last_applied_field_preset
            .get(&field_name)
            .and_then(|cur| names.iter().position(|n| n == cur))
            .unwrap_or(0);
        form_state.field_preset_picker = Some(FieldPresetPickerState {
            field_name: field_name.clone(),
            names,
            descriptions,
            selected,
        });
        return FormAction::None;
    }

    if key.code == KeyCode::Char('p') {
        form_state.composite_status = None;
        let entries = field_presets.get(&preset_storage_key);
        if entries.is_empty() {
            form_state.composite_status = Some("no presets saved for this field".to_string());
            return FormAction::None;
        }
        let names: Vec<String> = entries.iter().map(|p| p.name.clone()).collect();
        let cur_idx = form_state
            .last_applied_field_preset
            .get(&field_name)
            .and_then(|cur| names.iter().position(|n| n == cur));
        let next_idx = match cur_idx {
            Some(i) => (i + 1) % names.len(),
            None => 0,
        };
        return FormAction::ApplyFieldPreset {
            field_name,
            preset_name: names[next_idx].clone(),
        };
    }

    match key.code {
        KeyCode::Esc => {
            form_state.composite_open = false;
            form_state.composite_status = None;
        }

        KeyCode::Enter => {
            let rows = form_state.composite_values.entry(field_name).or_default();
            let new_row = vec![String::new(); col_count];
            if rows.is_empty() {
                rows.push(new_row);
                form_state.composite_row = 0;
            } else {
                let insert_at = (row + 1).min(rows.len());
                rows.insert(insert_at, new_row);
                form_state.composite_row = insert_at;
            }
            form_state.composite_col = 0;
            form_state.cursor_position = 0;
        }

        KeyCode::Delete => {
            let rows = form_state.composite_values.entry(field_name).or_default();
            if !rows.is_empty() {
                let idx = row.min(rows.len() - 1);
                rows.remove(idx);
                if rows.is_empty() {
                    form_state.composite_row = 0;
                } else {
                    form_state.composite_row = row.min(rows.len() - 1);
                }
                form_state.cursor_position = 0;
            }
        }

        KeyCode::Tab => {
            let rows = form_state.composite_values.get(&field_name);
            let row_count = rows.map(|r| r.len()).unwrap_or(0);
            if row_count == 0 {
                return FormAction::None;
            }
            let mut new_col = col + 1;
            let mut new_row = row;
            if new_col >= col_count {
                new_col = 0;
                new_row = (row + 1).min(row_count - 1);
            }
            form_state.composite_col = new_col;
            form_state.composite_row = new_row;
            form_state.cursor_position = composite_cell_len(form_state, &field_name);
        }

        KeyCode::BackTab => {
            let rows = form_state.composite_values.get(&field_name);
            if rows.map(|r| r.len()).unwrap_or(0) == 0 {
                return FormAction::None;
            }
            if col == 0 {
                if row > 0 {
                    form_state.composite_row = row - 1;
                    form_state.composite_col = col_count - 1;
                }
            } else {
                form_state.composite_col = col - 1;
            }
            form_state.cursor_position = composite_cell_len(form_state, &field_name);
        }

        KeyCode::Up => {
            let row_count = form_state
                .composite_values
                .get(&field_name)
                .map(|r| r.len())
                .unwrap_or(0);
            if row_count > 0 && row > 0 {
                form_state.composite_row = row - 1;
            }
            form_state.cursor_position = composite_cell_len(form_state, &field_name);
        }

        KeyCode::Down => {
            let row_count = form_state
                .composite_values
                .get(&field_name)
                .map(|r| r.len())
                .unwrap_or(0);
            if row_count > 0 && row < row_count - 1 {
                form_state.composite_row = row + 1;
            }
            form_state.cursor_position = composite_cell_len(form_state, &field_name);
        }

        KeyCode::Left => {
            if let Some(sub) = sub_fields.get(col) {
                if sub.field_type == SubFieldType::StaticSelect {
                    cycle_composite_select_in(form_state, &field_name, sub, -1);
                } else if form_state.cursor_position > 0 {
                    form_state.cursor_position -= 1;
                }
            }
        }

        KeyCode::Right => {
            if let Some(sub) = sub_fields.get(col) {
                if sub.field_type == SubFieldType::StaticSelect {
                    cycle_composite_select_in(form_state, &field_name, sub, 1);
                } else {
                    let len = composite_cell_len(form_state, &field_name);
                    if form_state.cursor_position < len {
                        form_state.cursor_position += 1;
                    }
                }
            }
        }

        KeyCode::Char(' ') => {
            if let Some(sub) = sub_fields.get(col)
                && sub.field_type == SubFieldType::StaticSelect
            {
                cycle_composite_select_in(form_state, &field_name, sub, 1);
                return FormAction::None;
            }
            insert_composite_char_in(form_state, &field_name, sub_fields, ' ');
        }

        KeyCode::Char(c) => {
            insert_composite_char_in(form_state, &field_name, sub_fields, c);
        }

        KeyCode::Backspace => {
            let r = form_state.composite_row;
            let c = form_state.composite_col;
            if let Some(rows) = form_state.composite_values.get_mut(&field_name)
                && let Some(row) = rows.get_mut(r)
                && let Some(cell) = row.get_mut(c)
                && form_state.cursor_position > 0
                && !cell.is_empty()
            {
                let pos = form_state.cursor_position.min(cell.len());
                cell.remove(pos - 1);
                form_state.cursor_position = pos - 1;
            }
        }

        _ => {}
    }

    FormAction::None
}

// ── Per-field preset picker overlay ──────────────────────────────────────────

fn handle_field_preset_picker_key(
    form_state: &mut FormState,
    field_name: &str,
    key: KeyEvent,
) -> FormAction {
    use crossterm::event::{KeyCode, KeyModifiers};

    let picker = match &mut form_state.field_preset_picker {
        Some(p) => p,
        None => return FormAction::None,
    };
    let count = picker.names.len();

    match key.code {
        KeyCode::Esc => {
            form_state.field_preset_picker = None;
            FormAction::None
        }
        KeyCode::Up => {
            if count > 0 {
                picker.selected = (picker.selected + count - 1) % count;
            }
            FormAction::None
        }
        KeyCode::Down => {
            if count > 0 {
                picker.selected = (picker.selected + 1) % count;
            }
            FormAction::None
        }
        KeyCode::Enter => {
            if count == 0 {
                return FormAction::None;
            }
            let preset_name = picker.names[picker.selected].clone();
            form_state.field_preset_picker = None;
            FormAction::ApplyFieldPreset {
                field_name: field_name.to_string(),
                preset_name,
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            if count == 0 {
                return FormAction::None;
            }
            let preset_name = picker.names[picker.selected].clone();
            FormAction::DeleteFieldPreset {
                field_name: field_name.to_string(),
                preset_name,
            }
        }
        _ => FormAction::None,
    }
}

// ── Composite cell helpers ────────────────────────────────────────────────────

pub(super) fn composite_cell_len(form_state: &FormState, field_name: &str) -> usize {
    form_state
        .composite_values
        .get(field_name)
        .and_then(|rows| rows.get(form_state.composite_row))
        .and_then(|row| row.get(form_state.composite_col))
        .map(|v| v.len())
        .unwrap_or(0)
}

pub(super) fn insert_composite_char_in(
    form_state: &mut FormState,
    field_name: &str,
    sub_fields: &[SubFieldConfig],
    c: char,
) {
    if let Some(sub) = sub_fields.get(form_state.composite_col) {
        if sub.field_type == SubFieldType::StaticSelect {
            return;
        }
        if sub.field_type == SubFieldType::Number && !c.is_ascii_digit() && c != '.' && c != '-' {
            return;
        }
    }

    let r = form_state.composite_row;
    let col = form_state.composite_col;
    if let Some(rows) = form_state.composite_values.get_mut(field_name)
        && let Some(row) = rows.get_mut(r)
        && let Some(cell) = row.get_mut(col)
    {
        let pos = form_state.cursor_position.min(cell.len());
        cell.insert(pos, c);
        form_state.cursor_position = pos + 1;
    }
}

pub(super) fn cycle_composite_select_in(
    form_state: &mut FormState,
    field_name: &str,
    sub: &SubFieldConfig,
    delta: i32,
) {
    let options = match &sub.options {
        Some(opts) if !opts.is_empty() => opts,
        _ => return,
    };

    let r = form_state.composite_row;
    let c = form_state.composite_col;
    if let Some(rows) = form_state.composite_values.get_mut(field_name)
        && let Some(row) = rows.get_mut(r)
        && let Some(cell) = row.get_mut(c)
    {
        let current_idx = options.iter().position(|o| o == cell);
        let new_idx = match current_idx {
            Some(idx) => {
                let len = options.len() as i32;
                ((idx as i32 + delta).rem_euclid(len)) as usize
            }
            None => 0,
        };
        if let Some(new_value) = options.get(new_idx) {
            *cell = new_value.clone();
        }
    }
}
