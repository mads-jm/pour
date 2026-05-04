mod key;
pub(super) mod overlays;
pub mod render;

use ratatui::Frame;

use crate::app::{
    App, CalloutTitleEdit, FormState, PresetDialogFocus, PresetDialogTarget, PresetPickerState,
    PresetSaveDialog,
};
use crate::config::FieldType;
use crate::visibility::visible_field_indices;

/// Render the form view for the currently selected module.
pub fn render(app: &App, frame: &mut Frame) {
    render::render(app, frame);
}

/// Recompute which field `active_field` should point at after a potential
/// visibility change.
///
/// Two-phase logic:
///
/// **Phase 1 — in-range check**: If `active_field <= submit_idx` (i.e., within
/// the current visible set or exactly on the submit button), check whether the
/// config field it resolves to is the same one recorded in `active_config_idx`.
/// - If they agree (or `active_field == submit_idx` and `active_config_idx`
///   is None): no action needed; just sync `active_config_idx` to match and return.
/// - If they disagree (stale `active_config_idx`, e.g. from a direct test
///   assignment): `active_field` wins — sync `active_config_idx` to the config
///   field at the current visible position and return.
///
/// **Phase 2 — out-of-range recovery**: If `active_field > submit_idx`, the
/// visible set has shrunk. Use `active_config_idx` to locate the intended field:
/// - If `active_config_idx` is `None` (was on submit), land on the new submit.
/// - If `active_config_idx` is `Some(ci)` and `ci` is still visible, move to
///   its new visible position.
/// - If `ci` is no longer visible, prefer next visible field (higher config
///   index), then previous, then submit.
fn clamp_active_to_visible(form_state: &mut FormState, fields: &[crate::config::FieldConfig]) {
    let visible = visible_field_indices(fields, &form_state.field_values);
    let visible_count = visible.len();
    // active_field layout: 0=preset, 1..=visible_count=fields, visible_count+1=submit
    let submit_idx = visible_count + 1;

    if form_state.active_field == 0 {
        // On preset row — always valid, no config_idx to sync.
        form_state.active_config_idx = None;
        return;
    }

    if form_state.active_field <= submit_idx {
        // Phase 1: active_field is in a valid position.
        // Map real-field slot (1..=visible_count) to visible index (0..visible_count).
        let current_ci = if form_state.active_field <= visible_count {
            visible.get(form_state.active_field - 1).copied()
        } else {
            None // submit button
        };
        form_state.active_config_idx = current_ci;
        return;
    }

    // Phase 2: active_field is out of range — visible set shrank.
    let prev_ci = match form_state.active_config_idx {
        None => {
            // Was on submit or preset — keep on submit (new boundary).
            form_state.active_field = submit_idx;
            return;
        }
        Some(ci) => ci,
    };

    if let Some(new_vi) = visible.iter().position(|&ci| ci == prev_ci) {
        form_state.active_field = new_vi + 1; // +1 for preset row offset
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else if let Some(new_vi) = visible.iter().position(|&ci| ci > prev_ci) {
        form_state.active_field = new_vi + 1;
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else if let Some(new_vi) = visible.iter().rposition(|&ci| ci < prev_ci) {
        form_state.active_field = new_vi + 1;
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else {
        form_state.active_field = submit_idx;
        form_state.active_config_idx = None;
    }
}

/// Resolve the active `FieldConfig` given a fields slice directly.
///
/// Used by `key/` submodules that have a fields slice but not a full `ModuleConfig`.
pub(super) fn active_field_config_fields<'a>(
    form_state: &FormState,
    fields: &'a [crate::config::FieldConfig],
) -> Option<&'a crate::config::FieldConfig> {
    if form_state.active_field == 0 {
        return None;
    }
    let visible = visible_field_indices(fields, &form_state.field_values);
    let vi = form_state.active_field - 1;
    visible.get(vi).and_then(|&ci| fields.get(ci))
}

/// Handle a key event while in Form view.
///
/// Returns a `FormAction` signalling what the wiring layer should do next.
pub fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) -> FormAction {
    use crossterm::event::KeyCode;

    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k.clone(),
        None => return FormAction::None,
    };

    // ── Compute mode flags from a scoped borrow of module + form_state ────────
    // All references into `app` must be dropped before calling `key::dispatch`
    // (which takes `&mut App`). We clone/own the data we need.
    let (
        visible_count,
        navigable_count,
        visible_indices,
        on_preset_row,
        on_submit_button,
        is_select,
        is_textarea,
        is_composite,
        is_dynamic_allow_create,
        is_static_allow_create,
        active_field_name,
        active_field_type,
        active_config_idx_pre,
        // Hotkey data extracted while module is still in scope
        preset_axes_empty,
        preset_axes_clone,
        callout_data, // Option<(name, has_callout, prefill, callout_title_clone)>
        composite_field_data, // Option<(FieldConfig clone)> for composite overlay
    ) = {
        let module = match app.config.modules.get(&module_key) {
            Some(m) => m,
            None => return FormAction::None,
        };
        let form_state = match &mut app.form_state {
            Some(fs) => fs,
            None => return FormAction::None,
        };

        clamp_active_to_visible(form_state, &module.fields);

        let visible_indices = visible_field_indices(&module.fields, &form_state.field_values);
        let visible_count = visible_indices.len();
        let navigable_count = visible_count + 2;
        let on_preset_row = form_state.active_field == 0;
        let on_submit_button = form_state.active_field == visible_count + 1;

        let active_field = if form_state.active_field > 0 && !on_submit_button {
            let vi = form_state.active_field - 1;
            visible_indices
                .get(vi)
                .and_then(|&ci| module.fields.get(ci))
        } else {
            None
        };

        let is_select = active_field
            .map(|f| {
                matches!(
                    f.field_type,
                    FieldType::StaticSelect | FieldType::DynamicSelect
                )
            })
            .unwrap_or(false);
        let is_textarea = active_field
            .map(|f| f.field_type == FieldType::Textarea)
            .unwrap_or(false);
        let is_composite = active_field
            .map(|f| f.field_type == FieldType::CompositeArray)
            .unwrap_or(false);
        let is_dynamic_allow_create = active_field
            .map(|f| f.field_type == FieldType::DynamicSelect && f.allow_create.unwrap_or(false))
            .unwrap_or(false);
        let is_static_allow_create = active_field
            .map(|f| f.field_type == FieldType::StaticSelect && f.allow_create.unwrap_or(false))
            .unwrap_or(false);

        let active_field_name = active_field.map(|f| f.name.clone());
        let active_field_type = active_field.map(|f| f.field_type.clone());
        let active_config_idx_pre = form_state.active_config_idx;

        // Callout-title trigger data
        let callout_data = if matches!(key.code, KeyCode::Char('t') | KeyCode::Char('T'))
            && !form_state.textarea_open
            && let Some(field) = active_field
            && field.field_type == FieldType::Textarea
        {
            let has_callout = form_state.callout_overrides.contains_key(&field.name)
                || (field.callout.is_some()
                    && !form_state
                        .callout_overrides
                        .get(&field.name)
                        .is_some_and(|s| s.is_empty()))
                || form_state.callout_overrides.contains_key("_callout_type");
            if has_callout {
                let prefill = form_state
                    .callout_titles
                    .get(&field.name)
                    .cloned()
                    .or_else(|| field.callout_title.clone())
                    .unwrap_or_default();
                Some((
                    field.name.clone(),
                    true,
                    prefill,
                    field.callout_title.clone(),
                ))
            } else {
                None
            }
        } else {
            None
        };

        // Composite field clone for composite overlay
        let composite_field_data = if is_composite && form_state.composite_open {
            active_field.cloned()
        } else {
            None
        };

        // Preset axes data for 's' and 'p' hotkeys
        let preset_axes_empty = module.preset_axes.is_empty();
        let preset_axes_clone = module.preset_axes.clone();

        (
            visible_count,
            navigable_count,
            visible_indices,
            on_preset_row,
            on_submit_button,
            is_select,
            is_textarea,
            is_composite,
            is_dynamic_allow_create,
            is_static_allow_create,
            active_field_name,
            active_field_type,
            active_config_idx_pre,
            preset_axes_empty,
            preset_axes_clone,
            callout_data,
            composite_field_data,
        )
    };
    // All borrows of `app` through `module`/`form_state` are now released.

    let _is_select_allow_create = is_dynamic_allow_create || is_static_allow_create;

    // ── Overlay intercepts (highest priority) ────────────────────────────────

    {
        let form_state = app.form_state.as_mut().unwrap();

        if form_state.callout_title_edit.is_some() {
            return overlays::small::handle_callout_title_key(form_state, key);
        }
    }

    if app
        .form_state
        .as_ref()
        .map(|fs| fs.preset_overlay.is_some())
        .unwrap_or(false)
    {
        let module = app.config.modules.get(&module_key).unwrap();
        let (form_state, module, presets) =
            (app.form_state.as_mut().unwrap(), module, &app.presets);
        let _ = presets; // accessed through module_key below
        return overlays::small::handle_preset_save_key(form_state, &module_key, module, key);
    }

    if app
        .form_state
        .as_ref()
        .map(|fs| fs.preset_picker.is_some())
        .unwrap_or(false)
    {
        let module = app.config.modules.get(&module_key).unwrap();
        let form_state = app.form_state.as_mut().unwrap();
        let presets = &app.presets;
        return overlays::preset_picker::handle_key(form_state, &module_key, module, presets, key);
    }

    if app
        .form_state
        .as_ref()
        .map(|fs| fs.confirm_delete_preset)
        .unwrap_or(false)
    {
        let form_state = app.form_state.as_mut().unwrap();
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let name = form_state.selected_preset_name.clone().unwrap_or_default();
                form_state.confirm_delete_preset = false;
                return FormAction::DeletePreset { name };
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                form_state.confirm_delete_preset = false;
                return FormAction::None;
            }
            _ => return FormAction::None,
        }
    }

    if app
        .form_state
        .as_ref()
        .map(|fs| fs.sub_form.is_some())
        .unwrap_or(false)
    {
        let config = &app.config;
        let form_state = app.form_state.as_mut().unwrap();
        return overlays::sub_form::handle_key(form_state, config, key);
    }

    // Composite overlay
    if let Some(field) = composite_field_data {
        let form_state = app.form_state.as_mut().unwrap();
        return key::composite::handle_composite_key(
            form_state,
            &field,
            &app.field_presets,
            &module_key,
            key,
        );
    }

    // ── Hotkey triggers ───────────────────────────────────────────────────────

    // `t` / `T`: open callout-title editor
    if let Some((field_name, _has_callout, prefill, _callout_title)) = callout_data {
        let cursor = prefill.chars().count();
        let form_state = app.form_state.as_mut().unwrap();
        form_state.callout_title_edit = Some(CalloutTitleEdit {
            field_name,
            buffer: prefill,
            cursor,
        });
        return FormAction::None;
    }

    // `s` / Ctrl+S: open preset save dialog
    if key.code == KeyCode::Char('s')
        && app
            .form_state
            .as_ref()
            .map(|fs| !fs.textarea_open && !fs.composite_open)
            .unwrap_or(false)
        && ((on_preset_row || on_submit_button)
            || key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL))
    {
        let module = app.config.modules.get(&module_key).unwrap();
        let form_state = app.form_state.as_mut().unwrap();
        let (prefill_name, prefill_desc, name_was_user_edited) =
            if let Some(ref sel_name) = form_state.selected_preset_name.clone() {
                let idx = form_state.preset_names.iter().position(|n| n == sel_name);
                let desc = idx
                    .and_then(|i| form_state.preset_descriptions.get(i))
                    .and_then(|d| d.clone())
                    .unwrap_or_default();
                (sel_name.clone(), desc, true)
            } else {
                let suggested = if !preset_axes_empty {
                    crate::data::preset_tree::suggest_preset_name(
                        &form_state.field_values,
                        &module.preset_axes,
                    )
                } else {
                    String::new()
                };
                (suggested, String::new(), false)
            };
        let cursor_position = prefill_name.chars().count();
        let description_cursor = prefill_desc.chars().count();
        form_state.preset_overlay = Some(PresetSaveDialog {
            name_buffer: prefill_name,
            cursor_position,
            description_buffer: prefill_desc,
            description_cursor,
            focus: PresetDialogFocus::Name,
            target: PresetDialogTarget::Module,
            name_was_user_edited,
            awaiting_overwrite_confirm: false,
        });
        return FormAction::None;
    }

    // `d`: delete preset
    if key.code == KeyCode::Char('d')
        && on_preset_row
        && app
            .form_state
            .as_ref()
            .map(|fs| fs.selected_preset_name.is_some())
            .unwrap_or(false)
    {
        if let Some(fs) = app.form_state.as_mut() {
            fs.confirm_delete_preset = true;
        }
        return FormAction::None;
    }

    // `p` / Ctrl+P: open preset picker
    let picker_trigger = !preset_axes_empty
        && app
            .form_state
            .as_ref()
            .map(|fs| fs.axis_warnings.is_empty() && !fs.textarea_open && !fs.composite_open)
            .unwrap_or(false)
        && (key.code == KeyCode::Char('p')
            && (on_preset_row
                || key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)));
    if picker_trigger {
        let presets = app.presets.get(&module_key);
        let tree = crate::data::preset_tree::build(&presets, &preset_axes_clone);
        if let Some(fs) = app.form_state.as_mut() {
            fs.preset_picker = Some(PresetPickerState {
                tree,
                path: Vec::new(),
                selected: 0,
                viewport_offset: 0,
            });
        }
        return FormAction::None;
    }

    // ── Mode dispatch ─────────────────────────────────────────────────────────

    let ctx = key::NavCtx {
        visible_count,
        navigable_count,
        visible_indices,
    };

    key::dispatch(
        app,
        &module_key,
        key,
        ctx,
        on_submit_button,
        on_preset_row,
        is_select,
        is_textarea,
        is_composite,
        is_dynamic_allow_create,
        is_static_allow_create,
        active_field_name,
        active_field_type,
        active_config_idx_pre,
    )
}

/// Actions that the form handler can signal to the wiring layer.
#[derive(Debug, PartialEq, Eq)]
pub enum FormAction {
    None,
    Submit,
    Cancel,
    /// User submitted the sub-form overlay for template-driven note creation.
    CreateFromTemplate {
        field_name: String,
        template_name: String,
        note_name: String,
        field_values: std::collections::HashMap<String, String>,
    },
    /// Save a preset with the given name and field values for the current module.
    SavePreset {
        name: String,
        description: Option<String>,
        values: std::collections::HashMap<String, String>,
    },
    /// Delete the preset with the given name for the current module.
    DeletePreset {
        name: String,
    },
    /// Reorder the preset with the given name by `direction` (+1 or -1).
    ReorderPreset {
        name: String,
        direction: i32,
    },
    /// Append a novel option to a static_select field's `options` list,
    /// persisting the change to config.toml. The field value has already
    /// been set in-memory; this action only handles persistence.
    AppendStaticOption {
        field_index: usize,
        value: String,
    },
    /// Save the current rows of a composite_array field as a named preset.
    SaveFieldPreset {
        field_name: String,
        name: String,
        description: Option<String>,
        rows: Vec<Vec<String>>,
    },
    /// Apply (replace rows with) a saved preset for a composite_array field.
    ApplyFieldPreset {
        field_name: String,
        preset_name: String,
    },
    /// Quick-cycle to the next/previous saved preset for a composite_array field.
    /// Direction is +1 (next) or -1 (previous). No-op if no presets are saved.
    CycleFieldPreset {
        field_name: String,
        direction: i32,
    },
    /// Delete a saved preset for a composite_array field by name.
    DeleteFieldPreset {
        field_name: String,
        preset_name: String,
    },
}
