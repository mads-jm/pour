use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, FormState};
use crate::tui::form::FormAction;

/// Shared navigation context computed once per `handle_key` call.
pub(in crate::tui::form) struct NavCtx {
    pub visible_count: usize,
    pub navigable_count: usize,
    pub visible_indices: Vec<usize>,
}

// ── Preset row ──────────────────────────────────────────────────────────────

/// Handle all keys when `active_field == 0` (preset row).
pub(super) fn handle_preset_row(
    app: &mut App,
    module_key: &str,
    key: KeyEvent,
    ctx: NavCtx,
) -> FormAction {
    let preset_count = app
        .form_state
        .as_ref()
        .map(|fs| fs.preset_names.len())
        .unwrap_or(0);
    let total = preset_count + 1;
    let axes_empty = app
        .config
        .modules
        .get(module_key)
        .map(|m| m.preset_axes.is_empty())
        .unwrap_or(true)
        || app
            .form_state
            .as_ref()
            .map(|fs| !fs.axis_warnings.is_empty())
            .unwrap_or(false);

    let current_idx = app
        .form_state
        .as_ref()
        .and_then(|fs| {
            fs.selected_preset_name
                .as_ref()
                .and_then(|n| fs.preset_names.iter().position(|p| p == n))
                .map(|i| i + 1)
        })
        .unwrap_or(0);

    match key.code {
        KeyCode::Left => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let name = app
                    .form_state
                    .as_ref()
                    .and_then(|fs| fs.selected_preset_name.clone());
                if let Some(name) = name {
                    return FormAction::ReorderPreset {
                        name,
                        direction: -1,
                    };
                }
            } else if axes_empty && total > 0 {
                let new_idx = (current_idx + total - 1) % total;
                apply_preset_by_idx(app, module_key, new_idx);
            }
            FormAction::None
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let name = app
                    .form_state
                    .as_ref()
                    .and_then(|fs| fs.selected_preset_name.clone());
                if let Some(name) = name {
                    return FormAction::ReorderPreset { name, direction: 1 };
                }
            } else if axes_empty && total > 0 {
                let new_idx = (current_idx + 1) % total;
                apply_preset_by_idx(app, module_key, new_idx);
            }
            FormAction::None
        }
        KeyCode::Up => {
            if let Some(fs) = app.form_state.as_mut() {
                fs.active_field = ctx.visible_count + 1;
                fs.active_config_idx = None;
                fs.cursor_position = 0;
            }
            FormAction::None
        }
        KeyCode::Down | KeyCode::Tab => {
            let active_field = if ctx.visible_count > 0 {
                1
            } else {
                ctx.visible_count + 1
            };
            let first_vi = ctx.visible_indices.first().copied();
            // Compute cursor_len before taking mutable borrow
            let val_len = app
                .form_state
                .as_ref()
                .and_then(|fs| {
                    app.config
                        .modules
                        .get(module_key)
                        .and_then(|m| super::current_value_len(fs, &m.fields).into())
                })
                .unwrap_or(0);
            if let Some(fs) = app.form_state.as_mut() {
                fs.active_field = active_field;
                fs.active_config_idx = first_vi;
                fs.cursor_position = val_len;
            }
            FormAction::None
        }
        KeyCode::BackTab => {
            if let Some(fs) = app.form_state.as_mut() {
                fs.active_field = ctx.visible_count + 1;
                fs.active_config_idx = None;
                fs.cursor_position = 0;
            }
            FormAction::None
        }
        KeyCode::Esc => FormAction::Cancel,
        _ => FormAction::None,
    }
}

fn apply_preset_by_idx(app: &mut App, module_key: &str, new_idx: usize) {
    let new_name = if new_idx > 0 {
        app.form_state
            .as_ref()
            .and_then(|fs| fs.preset_names.get(new_idx - 1).cloned())
    } else {
        None
    };
    let preset_entry = new_name.as_ref().and_then(|n| {
        app.presets
            .get(module_key)
            .into_iter()
            .find(|p| p.name == *n)
    });
    let fields = app
        .config
        .modules
        .get(module_key)
        .map(|m| m.fields.clone())
        .unwrap_or_default();
    if let Some(fs) = app.form_state.as_mut() {
        fs.selected_preset_name = new_name;
        App::apply_preset(fs, &fields, preset_entry.as_ref());
        fs.active_field = 0;
        fs.active_config_idx = None;
    }
}

// ── Field navigation (not on preset row, no mode overlay) ───────────────────

/// Tab — advance one field, close overlays.
pub(super) fn handle_tab(
    app: &mut App,
    module_key: &str,
    active_field_name: Option<&str>,
    ctx: &NavCtx,
) -> FormAction {
    // Compute cursor_len before taking mutable borrow
    let val_len = compute_new_cursor_len(app, module_key, ctx, /* forward */ true);
    let form_state = match app.form_state.as_mut() {
        Some(fs) => fs,
        None => return FormAction::None,
    };
    if let Some(name) = active_field_name {
        form_state.search_buffers.remove(name);
    }
    close_mode_overlays(form_state);
    let new_af = (form_state.active_field + 1) % ctx.navigable_count;
    set_active_field_with_len(form_state, new_af, ctx, val_len);
    FormAction::None
}

/// BackTab — go back one field, close overlays.
pub(super) fn handle_backtab(
    app: &mut App,
    module_key: &str,
    active_field_name: Option<&str>,
    ctx: &NavCtx,
) -> FormAction {
    let cur_af = app
        .form_state
        .as_ref()
        .map(|fs| fs.active_field)
        .unwrap_or(0);
    let new_af = if cur_af == 0 {
        ctx.navigable_count - 1
    } else {
        cur_af - 1
    };
    let val_len = compute_cursor_len_for_af(app, module_key, new_af, ctx);
    let form_state = match app.form_state.as_mut() {
        Some(fs) => fs,
        None => return FormAction::None,
    };
    if let Some(name) = active_field_name {
        form_state.search_buffers.remove(name);
    }
    close_mode_overlays(form_state);
    set_active_field_with_len(form_state, new_af, ctx, val_len);
    FormAction::None
}

/// Up arrow with no overlay — move to previous field.
pub(super) fn move_up(app: &mut App, module_key: &str, ctx: &NavCtx) -> FormAction {
    let cur_af = app
        .form_state
        .as_ref()
        .map(|fs| fs.active_field)
        .unwrap_or(0);
    let new_af = if cur_af == 0 {
        ctx.navigable_count - 1
    } else {
        cur_af - 1
    };
    let val_len = compute_cursor_len_for_af(app, module_key, new_af, ctx);
    let form_state = match app.form_state.as_mut() {
        Some(fs) => fs,
        None => return FormAction::None,
    };
    close_mode_overlays(form_state);
    set_active_field_with_len(form_state, new_af, ctx, val_len);
    FormAction::None
}

/// Down arrow with no overlay — move to next field.
pub(super) fn move_down(app: &mut App, module_key: &str, ctx: &NavCtx) -> FormAction {
    let cur_af = app
        .form_state
        .as_ref()
        .map(|fs| fs.active_field)
        .unwrap_or(0);
    let new_af = (cur_af + 1) % ctx.navigable_count;
    let val_len = compute_cursor_len_for_af(app, module_key, new_af, ctx);
    let form_state = match app.form_state.as_mut() {
        Some(fs) => fs,
        None => return FormAction::None,
    };
    close_mode_overlays(form_state);
    set_active_field_with_len(form_state, new_af, ctx, val_len);
    FormAction::None
}

// ── Esc (not on preset row, no mode overlay) ─────────────────────────────────

pub(super) fn handle_esc(
    form_state: &mut FormState,
    active_field_name: Option<&str>,
    is_select_allow_create: bool,
) -> FormAction {
    if is_select_allow_create
        && let Some(fname) = active_field_name
        && form_state
            .search_buffers
            .get(fname)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    {
        form_state
            .search_buffers
            .insert(fname.to_string(), String::new());
        return FormAction::None;
    }
    if form_state.dropdown_open {
        form_state.dropdown_open = false;
        return FormAction::None;
    }
    if form_state.textarea_open {
        form_state.textarea_open = false;
        form_state.textarea_scroll_offset = 0;
        return FormAction::None;
    }
    if form_state.composite_open {
        form_state.composite_open = false;
        return FormAction::None;
    }
    if let Some(fname) = active_field_name {
        let value = form_state
            .field_values
            .entry(fname.to_string())
            .or_default();
        if !value.is_empty() {
            value.clear();
            form_state.cursor_position = 0;
            return FormAction::None;
        }
    }
    FormAction::Cancel
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn close_mode_overlays(form_state: &mut FormState) {
    form_state.dropdown_open = false;
    form_state.textarea_open = false;
    form_state.textarea_scroll_offset = 0;
    form_state.composite_open = false;
}

fn set_active_field_with_len(
    form_state: &mut FormState,
    new_af: usize,
    ctx: &NavCtx,
    val_len: usize,
) {
    form_state.active_field = new_af;
    form_state.active_config_idx = if new_af > 0 && new_af <= ctx.visible_count {
        ctx.visible_indices.get(new_af - 1).copied()
    } else {
        None
    };
    form_state.cursor_position = val_len;
}

pub(super) fn compute_cursor_len_for_af_pub(
    app: &App,
    module_key: &str,
    new_af: usize,
    ctx: &NavCtx,
) -> usize {
    compute_cursor_len_for_af(app, module_key, new_af, ctx)
}

fn compute_cursor_len_for_af(app: &App, module_key: &str, new_af: usize, ctx: &NavCtx) -> usize {
    // Determine which field new_af resolves to, then get its value length
    if new_af == 0 || new_af > ctx.visible_count {
        return 0; // preset row or submit button
    }
    let vi = new_af - 1;
    let ci = match ctx.visible_indices.get(vi) {
        Some(&ci) => ci,
        None => return 0,
    };
    let fields = match app.config.modules.get(module_key) {
        Some(m) => &m.fields,
        None => return 0,
    };
    let field = match fields.get(ci) {
        Some(f) => f,
        None => return 0,
    };
    let fs = match app.form_state.as_ref() {
        Some(fs) => fs,
        None => return 0,
    };
    fs.field_values
        .get(&field.name)
        .map(|v| v.len())
        .unwrap_or(0)
}

/// Compute new cursor length after a Tab (forward) or BackTab (backward) move.
fn compute_new_cursor_len(app: &App, module_key: &str, ctx: &NavCtx, forward: bool) -> usize {
    let cur_af = app
        .form_state
        .as_ref()
        .map(|fs| fs.active_field)
        .unwrap_or(0);
    let new_af = if forward {
        (cur_af + 1) % ctx.navigable_count
    } else if cur_af == 0 {
        ctx.navigable_count - 1
    } else {
        cur_af - 1
    };
    compute_cursor_len_for_af(app, module_key, new_af, ctx)
}
