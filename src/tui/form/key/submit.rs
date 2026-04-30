use crate::app::App;
use crate::tui::form::FormAction;

use super::navigation::NavCtx;

/// Handle Enter for a text or number field (no overlay) — advance to next field.
pub(super) fn handle_enter_advance(app: &mut App, module_key: &str, ctx: &NavCtx) -> FormAction {
    let cur_af = app
        .form_state
        .as_ref()
        .map(|fs| fs.active_field)
        .unwrap_or(0);
    let new_af = (cur_af + 1) % ctx.navigable_count;
    let val_len = super::navigation::compute_cursor_len_for_af_pub(app, module_key, new_af, ctx);
    let form_state = match app.form_state.as_mut() {
        Some(fs) => fs,
        None => return FormAction::None,
    };
    form_state.active_field = new_af;
    form_state.active_config_idx = if new_af > 0 && new_af <= ctx.visible_count {
        ctx.visible_indices.get(new_af - 1).copied()
    } else {
        None
    };
    form_state.cursor_position = val_len;
    FormAction::None
}

/// Handle Enter for a textarea field: open editor or insert newline.
pub(super) fn handle_enter_textarea(
    app: &mut App,
    _module_key: &str,
    field_name: &str,
) -> FormAction {
    let textarea_open = app
        .form_state
        .as_ref()
        .map(|fs| fs.textarea_open)
        .unwrap_or(false);

    if textarea_open {
        // Insert newline at cursor
        let form_state = app.form_state.as_mut().unwrap();
        let value = form_state
            .field_values
            .entry(field_name.to_string())
            .or_default();
        let pos = form_state.cursor_position.min(value.len());
        value.insert(pos, '\n');
        form_state.cursor_position = pos + 1;
        form_state.textarea_scroll_offset = 0;
    } else {
        // Open the editor and set cursor to end
        let val_len = app
            .form_state
            .as_ref()
            .and_then(|fs| fs.field_values.get(field_name).map(|v| v.len()))
            .unwrap_or(0);
        let form_state = app.form_state.as_mut().unwrap();
        form_state.textarea_open = true;
        form_state.cursor_position = val_len;
    }
    FormAction::None
}
