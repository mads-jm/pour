use crate::app::App;

/// Auto-save dirty module-level settings to disk and reload the config.
///
/// Called before navigating away from `ModuleSettings` into a sub-screen
/// (field list, field editor) so that edits to path, mode, etc. are not
/// silently lost.  Returns `true` on success, `false` on error (in which
/// case a status message is set on the configure state).
pub(super) fn auto_save_module_settings(app: &mut App) -> bool {
    let state = match &app.configure_state {
        Some(s) if s.dirty => s,
        _ => return true, // nothing to save
    };

    let module_key = state.module_key.clone();

    let updates = crate::config_updates::build_module_updates(&state.settings);

    match crate::config::Config::update_module_on_disk(&module_key, &updates) {
        Ok(()) => match crate::config::Config::load() {
            Ok(new_config) => {
                app.config = new_config;
                if let Some(ref mut s) = app.configure_state {
                    s.dirty = false;
                    s.status_message = None;
                }
                true
            }
            Err(e) => {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(format!("Auto-save reload failed: {e}"));
                }
                false
            }
        },
        Err(e) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Auto-save failed: {e}"));
            }
            false
        }
    }
}

/// Auto-save dirty field-level settings to disk and reload the config.
///
/// Called before navigating from `FieldEditor` into a sub-screen (e.g.
/// sub-field list) so that field edits are not silently lost.
pub(super) fn auto_save_field_settings(app: &mut App, field_idx: usize) {
    let state = match &app.configure_state {
        Some(s) if s.dirty => s,
        _ => return,
    };
    let module_key = state.module_key.clone();

    let updates = build_field_updates_from_settings(&state.settings);

    match crate::config::Config::update_field_on_disk(&module_key, field_idx, &updates) {
        Ok(()) => match crate::config::Config::load() {
            Ok(new_config) => {
                app.config = new_config;
                if let Some(ref mut s) = app.configure_state {
                    s.dirty = false;
                    s.status_message = None;
                }
            }
            Err(e) => {
                if let Some(ref mut s) = app.configure_state {
                    s.status_message = Some(format!("Auto-save reload failed: {e}"));
                }
            }
        },
        Err(e) => {
            if let Some(ref mut s) = app.configure_state {
                s.status_message = Some(format!("Auto-save failed: {e}"));
            }
        }
    }
}

/// Build `FieldUpdates` from the current configure settings.
///
/// Thin wrapper kept for backward compatibility; delegates to the canonical
/// implementation in [`crate::config_updates`].
pub fn build_field_updates_from_settings(
    settings: &[crate::app::ConfigSetting],
) -> crate::config::FieldUpdates {
    crate::config_updates::build_field_updates(settings)
}
