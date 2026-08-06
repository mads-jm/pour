use crate::app::FormState;
use crate::config::FieldType;
use crate::tui::form::FormAction;

/// Convert a char-index into a byte offset within `s`.
///
/// If `char_idx` is beyond the last char, returns `s.len()` (end of string).
fn char_idx_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Insert a character into a text/textarea/number field.
///
/// `cursor_position` is a **char-index** (number of Unicode scalar values before
/// the cursor). All mutations go through `char_idx_to_byte` before touching the
/// `String`, so multi-byte characters (emoji, CJK, accented letters, …) are
/// handled without panicking on non-char boundaries.
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

    // A counter takes the same numeric characters plus a leading `=`, which is
    // the "set, don't add" token from spec §3.2.
    if field_type == Some(&FieldType::Counter)
        && !c.is_ascii_digit()
        && c != '.'
        && c != '-'
        && c != '='
    {
        return FormAction::None;
    }

    let value = form_state
        .field_values
        .entry(field_name.to_string())
        .or_default();
    // Clamp char-index to the number of chars actually present.
    let char_count = value.chars().count();
    let char_idx = form_state.cursor_position.min(char_count);
    let byte_pos = char_idx_to_byte(value, char_idx);
    value.insert(byte_pos, c);
    form_state.cursor_position = char_idx + 1;

    if is_textarea && form_state.textarea_open {
        sync_scroll(form_state, field_name);
    }
    FormAction::None
}

/// Delete the character before the cursor (one Unicode scalar value).
///
/// `cursor_position` is a char-index. Uses `char_idx_to_byte` to locate the
/// exact byte offset so that multi-byte characters are removed atomically.
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
        // Clamp char-index, then step back one char.
        let char_count = value.chars().count();
        let char_idx = form_state.cursor_position.min(char_count);
        // The char to delete is at char_idx - 1.
        let byte_pos = char_idx_to_byte(value, char_idx - 1);
        value.remove(byte_pos);
        form_state.cursor_position = char_idx - 1;
    }

    if is_textarea && form_state.textarea_open {
        sync_scroll(form_state, field_name);
    }
    FormAction::None
}

/// Move cursor left by one char (Unicode scalar value).
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

/// Move cursor right by one char (Unicode scalar value).
pub(super) fn handle_right(
    form_state: &mut FormState,
    field_name: &str,
    is_textarea: bool,
) -> FormAction {
    let char_count = form_state
        .field_values
        .get(field_name)
        .map(|v| v.chars().count())
        .unwrap_or(0);
    if form_state.cursor_position < char_count {
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
