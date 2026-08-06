pub(super) mod composite;
pub(in crate::tui::form) mod navigation;
pub(super) mod select;
pub(super) mod submit;
pub(super) mod text;

use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, FormState};
use crate::config::FieldType;
use crate::tui::form::FormAction;

pub(in crate::tui::form) use navigation::NavCtx;

// ── Shared helpers exposed to sub-modules ─────────────────────────────────────

/// Cycle the selected value within the filtered subset of options.
pub(super) fn cycle_select_filtered(
    form_state: &mut FormState,
    field_name: &str,
    delta: i32,
    search: &str,
) {
    let all_options = match form_state.field_options.get(field_name) {
        Some(opts) if !opts.is_empty() => opts,
        _ => return,
    };

    let options: Vec<String> = if search.is_empty() {
        all_options.clone()
    } else {
        all_options
            .iter()
            .filter(|o| o.to_lowercase().contains(&search.to_lowercase()))
            .cloned()
            .collect()
    };

    if options.is_empty() {
        return;
    }

    let current = form_state
        .field_values
        .get(field_name)
        .cloned()
        .unwrap_or_default();

    let current_idx = options.iter().position(|o| *o == current);
    let new_idx = match current_idx {
        Some(idx) => {
            let len = options.len() as i32;
            ((idx as i32 + delta).rem_euclid(len)) as usize
        }
        None => 0,
    };

    if let Some(new_value) = options.get(new_idx) {
        form_state
            .field_values
            .insert(field_name.to_string(), new_value.clone());
    }
}

/// Sync textarea horizontal scroll so the cursor stays visible.
///
/// `cursor_position` is a char-index. Line lengths are measured in chars to
/// stay consistent.
pub(super) fn sync_textarea_scroll(form_state: &mut FormState, value: &str, avail_width: u16) {
    if avail_width == 0 {
        return;
    }
    let avail = avail_width as usize;
    const MARGIN: usize = 2;

    // Walk lines in chars to find the cursor column on its line.
    let mut remaining = form_state.cursor_position;
    let mut cursor_col: usize = 0;
    for line in value.split('\n') {
        let line_char_len = line.chars().count();
        if remaining <= line_char_len {
            cursor_col = remaining;
            break;
        }
        remaining -= line_char_len + 1;
    }

    let scroll = form_state.textarea_scroll_offset;

    let right_edge = scroll + avail.saturating_sub(MARGIN + 1);
    if cursor_col >= right_edge {
        form_state.textarea_scroll_offset =
            cursor_col.saturating_sub(avail.saturating_sub(MARGIN + 1));
    }
    if cursor_col < scroll + MARGIN && scroll > 0 {
        form_state.textarea_scroll_offset = cursor_col.saturating_sub(MARGIN);
    }
    if form_state.textarea_scroll_offset > cursor_col {
        form_state.textarea_scroll_offset = 0;
    }
}

/// Move a flat char-index cursor up or down by one line within multiline text.
///
/// `cursor` is a **char-index** (number of Unicode scalar values from the start
/// of the string). All line-length arithmetic uses `chars().count()` so that
/// multi-byte characters never cause a mismatch between the cursor and line
/// boundaries. Returns the new char-index after the move.
pub(super) fn move_cursor_vertically(text: &str, cursor: usize, delta: i32) -> usize {
    // Walk lines counting chars to find which line the cursor is on and its
    // column within that line.
    let mut line_start_chars: usize = 0;
    let mut current_line = 0;
    let mut col = cursor;
    for (i, line) in text.split('\n').enumerate() {
        let line_char_len = line.chars().count();
        if cursor <= line_start_chars + line_char_len {
            current_line = i;
            col = cursor - line_start_chars;
            break;
        }
        // +1 for the '\n' which counts as one char.
        line_start_chars += line_char_len + 1;
    }

    let target_line = (current_line as i32 + delta).max(0) as usize;

    let mut pos: usize = 0;
    for (i, line) in text.split('\n').enumerate() {
        let line_char_len = line.chars().count();
        if i == target_line {
            return pos + col.min(line_char_len);
        }
        pos += line_char_len + 1;
    }
    // Cursor past all lines → end of text (in chars).
    text.chars().count()
}

/// Get the char count of the current field value for cursor positioning.
///
/// Returns the number of Unicode scalar values (chars) in the current field's
/// value. This matches `cursor_position` semantics (char-index).
///
/// Takes `form_state` and `module_fields` as separate borrows, avoiding
/// the need to hold a borrow on all of `app`.
pub(super) fn current_value_len(
    form_state: &FormState,
    module_fields: &[crate::config::FieldConfig],
) -> usize {
    super::active_field_config_fields(form_state, module_fields)
        .and_then(|f| form_state.field_values.get(&f.name))
        .map(|v| v.chars().count())
        .unwrap_or(0)
}

// ── Top-level dispatcher ──────────────────────────────────────────────────────

/// Dispatch key events for the main form (not overlay-intercepted).
///
/// Takes `&mut App` exclusively. All mode flags and active-field identity are
/// passed as owned/cloned values so no borrowed references into `app` survive
/// into this function.
#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch(
    app: &mut App,
    module_key: &str,
    key: KeyEvent,
    ctx: NavCtx,
    on_submit_button: bool,
    on_preset_row: bool,
    is_select: bool,
    is_textarea: bool,
    is_composite: bool,
    is_dynamic_allow_create: bool,
    is_static_allow_create: bool,
    active_field_name: Option<String>,
    active_field_type: Option<FieldType>,
    active_config_idx: Option<usize>,
) -> FormAction {
    // Preset row: pass &mut App so navigation can call App::apply_preset
    if on_preset_row {
        return navigation::handle_preset_row(app, module_key, key, ctx);
    }

    match key.code {
        // ── Universal navigation ─────────────────────────────────────────────
        KeyCode::Esc => {
            let form_state = match app.form_state.as_mut() {
                Some(fs) => fs,
                None => return FormAction::None,
            };
            navigation::handle_esc(
                form_state,
                active_field_name.as_deref(),
                is_dynamic_allow_create || is_static_allow_create,
            )
        }

        KeyCode::Tab => navigation::handle_tab(app, module_key, active_field_name.as_deref(), &ctx),

        KeyCode::BackTab => {
            navigation::handle_backtab(app, module_key, active_field_name.as_deref(), &ctx)
        }

        // ── Up arrow ─────────────────────────────────────────────────────────
        KeyCode::Up => {
            let dropdown_open = app
                .form_state
                .as_ref()
                .map(|fs| fs.dropdown_open)
                .unwrap_or(false);
            let textarea_open = app
                .form_state
                .as_ref()
                .map(|fs| fs.textarea_open)
                .unwrap_or(false);

            if is_select && dropdown_open {
                let form_state = app.form_state.as_mut().unwrap();
                if let Some(ref fname) = active_field_name {
                    let search = if is_dynamic_allow_create || is_static_allow_create {
                        form_state
                            .search_buffers
                            .get(fname)
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    cycle_select_filtered(form_state, fname, -1, &search);
                }
                FormAction::None
            } else if is_textarea && textarea_open {
                let form_state = app.form_state.as_mut().unwrap();
                if let Some(ref fname) = active_field_name {
                    let value = form_state
                        .field_values
                        .get(fname)
                        .cloned()
                        .unwrap_or_default();
                    let cursor = form_state.cursor_position;
                    form_state.cursor_position = move_cursor_vertically(&value, cursor, -1);
                }
                FormAction::None
            } else {
                navigation::move_up(app, module_key, &ctx)
            }
        }

        // ── Down arrow ───────────────────────────────────────────────────────
        KeyCode::Down => {
            let dropdown_open = app
                .form_state
                .as_ref()
                .map(|fs| fs.dropdown_open)
                .unwrap_or(false);
            let textarea_open = app
                .form_state
                .as_ref()
                .map(|fs| fs.textarea_open)
                .unwrap_or(false);

            if is_select && dropdown_open {
                let form_state = app.form_state.as_mut().unwrap();
                if let Some(ref fname) = active_field_name {
                    let search = if is_dynamic_allow_create || is_static_allow_create {
                        form_state
                            .search_buffers
                            .get(fname)
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    cycle_select_filtered(form_state, fname, 1, &search);
                }
                FormAction::None
            } else if is_textarea && textarea_open {
                let form_state = app.form_state.as_mut().unwrap();
                if let Some(ref fname) = active_field_name {
                    let value = form_state
                        .field_values
                        .get(fname)
                        .cloned()
                        .unwrap_or_default();
                    let cursor = form_state.cursor_position;
                    form_state.cursor_position = move_cursor_vertically(&value, cursor, 1);
                }
                FormAction::None
            } else {
                navigation::move_down(app, module_key, &ctx)
            }
        }

        // ── Enter ────────────────────────────────────────────────────────────
        KeyCode::Enter => {
            if on_submit_button {
                return FormAction::Submit;
            }
            if is_select {
                return select::handle_select_enter(
                    app,
                    module_key,
                    active_field_name.as_deref().unwrap_or(""),
                    is_dynamic_allow_create || is_static_allow_create,
                    is_static_allow_create,
                    active_config_idx,
                );
            }
            if is_composite {
                let form_state = app.form_state.as_mut().unwrap();
                form_state.composite_open = true;
                form_state.composite_row = 0;
                form_state.composite_col = 0;
                return FormAction::None;
            }
            if is_textarea {
                let fname = active_field_name.as_deref().unwrap_or("").to_string();
                return submit::handle_enter_textarea(app, module_key, &fname);
            }
            // text / number: advance to next field
            submit::handle_enter_advance(app, module_key, &ctx)
        }

        // ── Char ─────────────────────────────────────────────────────────────
        KeyCode::Char(c) => {
            // A toggle has no text buffer: space flips it, everything else is
            // inert. Handled before the search/text branches so a toggle can
            // never accumulate typed characters.
            if active_field_type == Some(FieldType::Toggle) {
                if c == ' '
                    && let Some(ref fname) = active_field_name
                    && let Some(form_state) = app.form_state.as_mut()
                {
                    flip_toggle(form_state, fname);
                }
                return FormAction::None;
            }
            if (is_dynamic_allow_create || is_static_allow_create)
                && let Some(ref fname) = active_field_name
            {
                let form_state = app.form_state.as_mut().unwrap();
                let buf = form_state.search_buffers.entry(fname.clone()).or_default();
                if buf.len() < 100 {
                    buf.push(c);
                }
                form_state.dropdown_open = true;
                return FormAction::None;
            }
            if on_submit_button
                || is_select
                || is_composite
                || (is_textarea
                    && !app
                        .form_state
                        .as_ref()
                        .map(|fs| fs.textarea_open)
                        .unwrap_or(false))
            {
                return FormAction::None;
            }
            if let Some(ref fname) = active_field_name {
                let form_state = app.form_state.as_mut().unwrap();
                text::handle_char(
                    form_state,
                    fname,
                    active_field_type.as_ref(),
                    c,
                    is_textarea,
                )
            } else {
                FormAction::None
            }
        }

        // ── Backspace ────────────────────────────────────────────────────────
        KeyCode::Backspace => {
            if (is_dynamic_allow_create || is_static_allow_create)
                && let Some(ref fname) = active_field_name
            {
                let form_state = app.form_state.as_mut().unwrap();
                let buf = form_state.search_buffers.entry(fname.clone()).or_default();
                buf.pop();
                return FormAction::None;
            }
            if on_submit_button
                || is_select
                || is_composite
                || (is_textarea
                    && !app
                        .form_state
                        .as_ref()
                        .map(|fs| fs.textarea_open)
                        .unwrap_or(false))
            {
                return FormAction::None;
            }
            if let Some(ref fname) = active_field_name {
                let form_state = app.form_state.as_mut().unwrap();
                text::handle_backspace(form_state, fname, is_textarea)
            } else {
                FormAction::None
            }
        }

        // ── Left ─────────────────────────────────────────────────────────────
        KeyCode::Left => {
            let textarea_open = app
                .form_state
                .as_ref()
                .map(|fs| fs.textarea_open)
                .unwrap_or(false);
            if is_textarea
                && !textarea_open
                && let Some(ref fname) = active_field_name
            {
                let form_state = app.form_state.as_mut().unwrap();
                if cycle_callout(form_state, fname, -1) {
                    return FormAction::None;
                }
            }
            if is_select
                && !app
                    .form_state
                    .as_ref()
                    .map(|fs| fs.dropdown_open)
                    .unwrap_or(false)
            {
                if let Some(ref fname) = active_field_name {
                    let form_state = app.form_state.as_mut().unwrap();
                    select::inline_cycle(form_state, fname, -1);
                }
                return FormAction::None;
            }
            if let Some(ref fname) = active_field_name
                && !on_submit_button
                && !is_composite
            {
                let form_state = app.form_state.as_mut().unwrap();
                return text::handle_left(form_state, fname, is_textarea);
            }
            FormAction::None
        }

        // ── Right ────────────────────────────────────────────────────────────
        KeyCode::Right => {
            let textarea_open = app
                .form_state
                .as_ref()
                .map(|fs| fs.textarea_open)
                .unwrap_or(false);
            if is_textarea
                && !textarea_open
                && let Some(ref fname) = active_field_name
            {
                let form_state = app.form_state.as_mut().unwrap();
                if cycle_callout(form_state, fname, 1) {
                    return FormAction::None;
                }
            }
            if is_select
                && !app
                    .form_state
                    .as_ref()
                    .map(|fs| fs.dropdown_open)
                    .unwrap_or(false)
            {
                if let Some(ref fname) = active_field_name {
                    let form_state = app.form_state.as_mut().unwrap();
                    select::inline_cycle(form_state, fname, 1);
                }
                return FormAction::None;
            }
            if let Some(ref fname) = active_field_name
                && !on_submit_button
                && !is_composite
            {
                let form_state = app.form_state.as_mut().unwrap();
                return text::handle_right(form_state, fname, is_textarea);
            }
            FormAction::None
        }

        _ => FormAction::None,
    }
}

// ── Toggle helper ─────────────────────────────────────────────────────────────

/// Flip a `toggle` field between `"true"` and `"false"`.
///
/// Anything that is not exactly `"true"` counts as false, so a note whose
/// property holds junk flips to a clean `true` rather than staying stuck.
pub(super) fn flip_toggle(form_state: &mut FormState, field_name: &str) {
    let entry = form_state
        .field_values
        .entry(field_name.to_string())
        .or_default();
    *entry = if entry.trim().eq_ignore_ascii_case("true") {
        "false".to_string()
    } else {
        "true".to_string()
    };
}

// ── Callout cycling helper ────────────────────────────────────────────────────

fn cycle_callout(form_state: &mut FormState, field_name: &str, delta: i32) -> bool {
    let options = crate::app::CALLOUT_OPTIONS;

    if form_state.callout_overrides.contains_key(field_name) {
        let current = form_state.callout_overrides[field_name].clone();
        let new_idx = if delta < 0 {
            match options.iter().position(|(_, s)| *s == current) {
                Some(0) => options.len() - 1,
                Some(idx) => idx - 1,
                None => options.len() - 1,
            }
        } else {
            match options.iter().position(|(_, s)| *s == current) {
                Some(idx) => (idx + 1) % options.len(),
                None => 0,
            }
        };
        form_state
            .callout_overrides
            .insert(field_name.to_string(), options[new_idx].1.to_string());
        return true;
    }

    if form_state.callout_overrides.contains_key("_callout_type")
        && !form_state.callout_overrides.contains_key(field_name)
    {
        let current = form_state.callout_overrides["_callout_type"].clone();
        let new_idx = if delta < 0 {
            match options.iter().position(|(_, s)| *s == current) {
                Some(0) => options.len() - 1,
                Some(idx) => idx - 1,
                None => options.len() - 1,
            }
        } else {
            match options.iter().position(|(_, s)| *s == current) {
                Some(idx) => (idx + 1) % options.len(),
                None => 0,
            }
        };
        form_state
            .callout_overrides
            .insert("_callout_type".to_string(), options[new_idx].1.to_string());
        return true;
    }

    false
}
