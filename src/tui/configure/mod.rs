mod autosave;
pub mod init;
mod key;
mod render;

use crate::app::ConfigureState;

const SCROLL_MARGIN: usize = 2;

/// Actions the configure screen can signal to the wiring layer.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigureAction {
    None,
    Cancel,
    Save,
    /// Request a directory listing for the given vault-relative path.
    BrowseDirectory(String),
    /// Add a new default field to the current module.
    AddField,
    /// Remove the field at the given index (confirmed by user).
    RemoveField(usize),
    /// Swap the two field indices in the current module.
    ReorderFields(usize, usize),
    /// Delete the current module (confirmed by user).
    DeleteModule,
    /// Save the new module being configured to disk (Phase 4c stub).
    SaveNewModule,
    /// Add a new default sub-field to the current composite_array field.
    AddSubField(usize),
    /// Remove the sub-field at the given indices (field_index, sub_field_index).
    RemoveSubField(usize, usize),
    /// Swap two sub-field indices (field_index, a, b).
    ReorderSubFields(usize, usize, usize),
}

/// Adjust `state.scroll_offset` so the cursor stays visible in the edit viewport.
///
/// `term_cols` is the full terminal width in columns. We reconstruct avail from
/// the active setting's label length and kind_hint the same way render does.
fn sync_scroll_offset(state: &mut ConfigureState, term_cols: u16) {
    use crate::app::SettingKind;
    use unicode_width::UnicodeWidthStr;

    let setting = match state.settings.get(state.active_field) {
        Some(s) => s,
        None => return,
    };
    let kind_hint_len = match &setting.kind {
        SettingKind::Path => 9,      // " [Browse]"
        SettingKind::Toggle(_) => 8, // " [toggle]"
        SettingKind::Text => 0,
        SettingKind::NavLink => 2,     // " >"
        SettingKind::ListEditor => 10, // " [Edit list]"
        SettingKind::Identifier => 0,
        SettingKind::QuickSelect(_) => 8, // " [select]"
    };
    let prefix_len = 2 + UnicodeWidthStr::width(setting.label.as_str()) + 3;
    let avail = (term_cols as usize).saturating_sub(prefix_len + kind_hint_len);

    if avail == 0 {
        return;
    }

    let cursor = state.cursor_position;

    // Scroll right: cursor too far right
    let scroll_right_edge = state.scroll_offset + avail.saturating_sub(SCROLL_MARGIN + 1);
    if cursor >= scroll_right_edge {
        state.scroll_offset = cursor.saturating_sub(avail.saturating_sub(SCROLL_MARGIN + 1));
    }

    // Scroll left: cursor too far left
    if cursor < state.scroll_offset + SCROLL_MARGIN && state.scroll_offset > 0 {
        state.scroll_offset = cursor.saturating_sub(SCROLL_MARGIN);
    }

    // Never scroll past start
    if state.scroll_offset > cursor {
        state.scroll_offset = 0;
    }
}

// Public API re-exports

/// Render the configure screen.
pub use render::render;

/// Handle a key event on the configure screen.
pub use key::handle_key;

/// Build `FieldUpdates` from configure settings (backward-compatibility shim).
pub use autosave::build_field_updates_from_settings;

/// Init helpers re-exported for convenience.
pub use init::{
    build_field_settings, build_sub_field_settings, init_new_module_configure, init_vault_configure,
};
