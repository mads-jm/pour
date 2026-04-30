use crate::app::FormState;
use crate::config::FieldType;
use crate::tui::form::FormAction;

/// Insert a character into a text/textarea/number field.
pub(super) fn handle_char(
    form_state: &mut FormState,
    field_name: &str,
    field_type: Option<&FieldType>,
    c: char,
    is_textarea: bool,
) -> FormAction {
    if field_type == Some(&FieldType::Number) && !c.is_ascii_digit() && c != '.' && c != '-' {
        return FormAction::None;
    }

    let value = form_state
        .field_values
        .entry(field_name.to_string())
        .or_default();
    let pos = form_state.cursor_position.min(value.len());
    value.insert(pos, c);
    form_state.cursor_position = pos + 1;

    if is_textarea && form_state.textarea_open {
        sync_scroll(form_state, field_name);
    }
    FormAction::None
}

/// Delete the character before the cursor.
pub(super) fn handle_backspace(
    form_state: &mut FormState,
    field_name: &str,
    is_textarea: bool,
) -> FormAction {
    let value = form_state
        .field_values
        .entry(field_name.to_string())
        .or_default();
    if form_state.cursor_position > 0 && !value.is_empty() {
        let pos = form_state.cursor_position.min(value.len());
        value.remove(pos - 1);
        form_state.cursor_position = pos - 1;
    }

    if is_textarea && form_state.textarea_open {
        sync_scroll(form_state, field_name);
    }
    FormAction::None
}

/// Move cursor left by one byte.
pub(super) fn handle_left(
    form_state: &mut FormState,
    field_name: &str,
    is_textarea: bool,
) -> FormAction {
    if form_state.cursor_position > 0 {
        form_state.cursor_position -= 1;
    }
    if is_textarea && form_state.textarea_open {
        sync_scroll(form_state, field_name);
    }
    FormAction::None
}

/// Move cursor right by one byte.
pub(super) fn handle_right(
    form_state: &mut FormState,
    field_name: &str,
    is_textarea: bool,
) -> FormAction {
    let len = form_state
        .field_values
        .get(field_name)
        .map(|v| v.len())
        .unwrap_or(0);
    if form_state.cursor_position < len {
        form_state.cursor_position += 1;
    }
    if is_textarea && form_state.textarea_open {
        sync_scroll(form_state, field_name);
    }
    FormAction::None
}

fn sync_scroll(form_state: &mut FormState, field_name: &str) {
    let value_snap = form_state
        .field_values
        .get(field_name)
        .cloned()
        .unwrap_or_default();
    let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
    let avail = term_cols.saturating_sub(8).min(60).saturating_sub(2);
    super::sync_textarea_scroll(form_state, &value_snap, avail);
}
