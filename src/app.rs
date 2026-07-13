// LINTOK: oversized: state container + factory at v1.0.0; further App split deferred to v1.1
use crate::config::{
    Config, FieldType, ModuleConfig, SubFieldType, TemplateConfig, TemplateFieldType, WriteMode,
};
use crate::data::field_presets::FieldPresets;
use crate::data::history::History;
use crate::data::presets::Presets;
use crate::transport::{Transport, TransportMode, VaultEntry};
use crate::visibility::visible_field_indices;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Default duration for status-bar toast messages.
pub const STATUS_TOAST_DURATION: Duration = Duration::from_secs(5);

/// An ephemeral warning displayed at the bottom of all TUI screens.
#[derive(Debug)]
pub struct StatusMessage {
    pub text: String,
    pub expires_at: Instant,
}

/// Which screen the TUI is currently displaying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Form,
    Summary,
    Configure,
}

/// Which field of the preset-save overlay currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetDialogFocus {
    Name,
    Description,
}

/// What kind of preset the save overlay is naming.
///
/// `Module` — saves a flat field-value map for the current module
/// (default; preset row at `active_field == 0`).
/// `CompositeField` — saves the current rows of the named composite_array
/// field (opened from inside the composite overlay).
#[derive(Debug, Clone)]
pub enum PresetDialogTarget {
    Module,
    CompositeField { field_name: String },
}

/// Dialog state for naming a preset during a save operation.
#[derive(Debug)]
pub struct PresetSaveDialog {
    /// Text typed by the user as the preset name.
    pub name_buffer: String,
    /// Cursor position within `name_buffer`.
    pub cursor_position: usize,
    /// Optional human-readable description for the preset.
    pub description_buffer: String,
    /// Cursor position within `description_buffer`.
    pub description_cursor: usize,
    /// Which input row currently has focus.
    pub focus: PresetDialogFocus,
    /// What kind of preset is being saved (module-level vs. composite-field).
    pub target: PresetDialogTarget,
    /// `true` when the user has manually edited the name field (suppresses auto-suggest re-fill).
    pub name_was_user_edited: bool,
    /// `true` after first Enter on a colliding name — next Enter will overwrite.
    pub awaiting_overwrite_confirm: bool,
}

/// State for the hierarchical preset drilldown picker overlay.
///
/// Active when the user opens the picker via `p` on the preset row or `Ctrl+P`.
#[derive(Debug, Clone)]
pub struct PresetPickerState {
    /// The built tree for the current module.
    pub tree: crate::data::preset_tree::PresetTree,
    /// Breadcrumb path: indices into the current branch level's children.
    /// `path[0]` is an index into `tree.roots`, `path[1]` into that branch's children, etc.
    pub path: Vec<usize>,
    /// Currently highlighted index within the *current level* (roots or a branch's children).
    pub selected: usize,
    /// Scroll offset for the visible list window.
    pub viewport_offset: usize,
}

/// State for the in-overlay picker that lists saved presets for a single
/// composite_array field. Shown when the user presses `l` inside the
/// composite editor; loads on Enter, deletes on Ctrl+D.
#[derive(Debug, Clone)]
pub struct FieldPresetPickerState {
    /// Field name the picker is bound to.
    pub field_name: String,
    /// Ordered preset names (mirrors saved order in `field_presets.json`).
    pub names: Vec<String>,
    /// Optional descriptions parallel to `names`.
    pub descriptions: Vec<Option<String>>,
    /// Currently highlighted index into `names`.
    pub selected: usize,
}

/// State for the module entry form.
#[derive(Debug)]
pub struct FormState {
    /// Current value for each field, keyed by field name.
    pub field_values: HashMap<String, String>,
    /// Available options for select fields, keyed by field name.
    pub field_options: HashMap<String, Vec<String>>,
    /// Index of the currently active (focused) field within the **visible** field set.
    /// Ranges from 0..visible_count (inclusive); `visible_count` means the submit button.
    pub active_field: usize,
    /// Config-level index of the currently active field (`module.fields[i]`).
    /// `None` when the submit button is focused.
    /// Kept in sync with `active_field` so that visibility recomputation can detect
    /// when the focused field has become hidden and move focus appropriately.
    pub active_config_idx: Option<usize>,
    /// Validation error messages, populated on submit attempt.
    pub validation_errors: Vec<String>,
    /// Warning messages produced by `validate_axes` at form open.
    /// Non-empty → preset_axes are invalid; legacy cycler is active, picker is disabled.
    pub axis_warnings: Vec<String>,
    /// Cursor position within the active text/number input, measured as a
    /// **char-index** (number of Unicode scalar values before the cursor).
    /// Always snaps to a character boundary; never a raw byte offset.
    pub cursor_position: usize,
    /// Whether the dropdown for the current select field is open.
    pub dropdown_open: bool,
    /// Whether the textarea editor overlay is open.
    pub textarea_open: bool,
    /// Horizontal scroll offset for the textarea editor (chars).
    pub textarea_scroll_offset: usize,
    /// Runtime callout type overrides, keyed by field name.
    /// Initialized from config defaults; cyclable via Left/Right in the form.
    pub callout_overrides: HashMap<String, String>,
    /// Runtime callout title overrides, keyed by field name.
    /// Initialized from config `callout_title`; editable via Ctrl+T on a
    /// textarea field with an active callout.
    pub callout_titles: HashMap<String, String>,
    /// Active inline callout-title edit buffer.
    /// `Some((field_name, cursor))` while the user is typing in the title
    /// prompt overlay; `None` otherwise.
    pub callout_title_edit: Option<CalloutTitleEdit>,
    /// Row data for composite_array fields, keyed by field name.
    /// Each row is a Vec of cell values (one per sub-field column).
    pub composite_values: HashMap<String, Vec<Vec<String>>>,
    /// Whether the composite_array editor overlay is open.
    pub composite_open: bool,
    /// Currently selected row in the composite overlay.
    pub composite_row: usize,
    /// Currently selected column in the composite overlay.
    pub composite_col: usize,
    /// Typed search/filter text for `dynamic_select` fields with `allow_create = true`.
    /// Keyed by field name. Non-empty means the user is filtering or typing a novel value.
    pub search_buffers: HashMap<String, String>,
    /// Active sub-form overlay for template-driven inline note creation.
    pub sub_form: Option<SubFormState>,
    /// Ordered list of preset names for the current module.
    pub preset_names: Vec<String>,
    /// Parallel to `preset_names`: optional description per preset.
    /// Rendered as a dim subtitle under the preset row when `Some`.
    pub preset_descriptions: Vec<Option<String>>,
    /// Name of the currently selected preset, or `None` for `<none>`.
    pub selected_preset_name: Option<String>,
    /// Hierarchical preset picker overlay. `Some` while the drilldown picker is open.
    pub preset_picker: Option<PresetPickerState>,
    /// Open preset-save dialog, if the user is naming a new preset.
    pub preset_overlay: Option<PresetSaveDialog>,
    /// Whether the delete-preset confirmation prompt is shown.
    pub confirm_delete_preset: bool,
    /// Active per-field preset picker overlay (composite_array fields).
    /// `Some` while the user is browsing saved row-sets inside the composite
    /// editor; `None` otherwise.
    pub field_preset_picker: Option<FieldPresetPickerState>,
    /// Last preset applied per composite field, keyed by field name.
    /// Drives the `preset: <name>` subtitle in the composite overlay.
    pub last_applied_field_preset: HashMap<String, String>,
    /// Transient status message shown in the composite overlay (e.g. after a
    /// preset save or a schema-adjusted apply). Cleared on next user action.
    pub composite_status: Option<String>,
    /// Resolved read-only priors panel (§ pour-review-priors). `None` until the
    /// first resolve completes or when the matched tier is empty (empty state).
    pub priors_panel: Option<crate::priors::ResolvedPanel>,
    /// When `true`, the priors panel is collapsed to a one-line summary hint
    /// (toggled with `Ctrl+R`). Read-only; never affects submit.
    pub priors_collapsed: bool,
}

/// Active callout-title edit session on a textarea field.
#[derive(Debug, Clone)]
pub struct CalloutTitleEdit {
    /// Name of the field whose callout title is being edited.
    pub field_name: String,
    /// Current buffer contents.
    pub buffer: String,
    /// Cursor position (char index) into `buffer`.
    pub cursor: usize,
}

/// State for the template-driven sub-form overlay.
///
/// When a user enters a novel value on a `dynamic_select` field with
/// `create_template` configured, a sub-form opens to collect template fields
/// before creating the note.
#[derive(Debug)]
pub struct SubFormState {
    /// Name of the template being used (key in `config.templates`).
    pub template_name: String,
    /// The raw value the user typed that triggered creation.
    pub note_name: String,
    /// Current values for each template field, keyed by field name.
    pub field_values: HashMap<String, String>,
    /// Available options for `static_select` template fields, keyed by field name.
    pub field_options: HashMap<String, Vec<String>>,
    /// Index of the currently active template field.
    pub active_field: usize,
    /// Cursor position within the active text/number input.
    pub cursor_position: usize,
    /// Whether the dropdown for the current `static_select` field is open.
    pub dropdown_open: bool,
    /// Name of the parent field that triggered this sub-form.
    pub parent_field_name: String,
    /// Error message to display in the sub-form overlay (e.g. write failure).
    pub error_message: Option<String>,
}

impl SubFormState {
    /// Create a new sub-form state from a template definition.
    ///
    /// Pre-fills `field_values` with defaults and populates `field_options`
    /// for `StaticSelect` fields.
    pub fn new(
        template_name: String,
        note_name: String,
        parent_field_name: String,
        template: &TemplateConfig,
    ) -> Self {
        let mut field_values = HashMap::new();
        let mut field_options = HashMap::new();

        for field in &template.fields {
            field_values.insert(
                field.name.clone(),
                field.default.clone().unwrap_or_default(),
            );

            if field.field_type == TemplateFieldType::StaticSelect
                && let Some(opts) = &field.options
            {
                field_options.insert(field.name.clone(), opts.clone());
            }
        }

        Self {
            template_name,
            note_name,
            field_values,
            field_options,
            active_field: 0,
            cursor_position: 0,
            dropdown_open: false,
            parent_field_name,
            error_message: None,
        }
    }
}

/// State for the post-write summary screen.
#[derive(Debug)]
pub struct SummaryState {
    /// Human-readable success or error message.
    pub message: String,
    /// Vault-relative path of the written file, if successful.
    pub file_path: Option<String>,
    /// Which transport backend was used for the write.
    pub transport_mode: TransportMode,
    /// Notes that were auto-created for novel dynamic_select values.
    pub auto_created_notes: Vec<crate::autocreate::AutoCreatedNote>,
}

/// Which level of the configure screen is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigureLevel {
    /// Editing module-level scalar settings (path, mode, etc.).
    ModuleSettings,
    /// Browsing the list of fields in the module.
    FieldList,
    /// Editing a specific field's properties (by index into module.fields).
    FieldEditor(usize),
    /// Browsing the list of sub-fields within a composite_array field.
    SubFieldList(usize),
    /// Editing a specific sub-field's properties (field_idx, sub_field_idx).
    SubFieldEditor(usize, usize),
    /// Editing vault-level settings (base_path, api_port, api_key).
    VaultSettings,
    /// Creating a new module (key, display name, mode, path).
    NewModule,
}

/// The kind of input widget for a configure setting.
#[derive(Debug, Clone)]
pub enum SettingKind {
    Text,
    Path,
    Toggle(Vec<String>),
    /// A non-editable row that navigates to a sub-screen on Enter.
    NavLink,
    /// A list of strings, edited via a multiline text overlay (one item per line).
    ListEditor,
    /// A TOML-key-safe identifier: alphanumeric, underscore, hyphen; no spaces or dots.
    Identifier,
    /// A quick-select menu: each option has a hotkey char and label.
    /// The value stored is the label string (e.g. "note", "tip").
    /// An empty value means "none" (no selection).
    QuickSelect(Vec<(char, String)>),
}

/// Obsidian callout types with unique hotkey assignments, ordered by frequency.
pub const CALLOUT_OPTIONS: &[(char, &str)] = &[
    ('n', "note"),
    ('i', "info"),
    ('t', "todo"),
    ('p', "tip"),
    ('w', "warning"),
    ('q', "question"),
    ('e', "example"),
    ('u', "quote"),
    ('s', "success"),
    ('f', "failure"),
    ('b', "bug"),
    ('d', "danger"),
];

/// Build QuickSelect options for callout types.
pub fn callout_quick_select() -> Vec<(char, String)> {
    CALLOUT_OPTIONS
        .iter()
        .map(|&(c, s)| (c, s.to_string()))
        .collect()
}

/// A single editable setting in the configure screen.
#[derive(Debug, Clone)]
pub struct ConfigSetting {
    pub label: String,
    pub key: String,
    pub value: String,
    pub kind: SettingKind,
}

/// State for the vault directory browser popup.
#[derive(Debug)]
pub struct BrowserState {
    pub current_path: String,
    pub entries: Vec<VaultEntry>,
    pub selected: usize,
    /// Error message from the last directory listing attempt, if any.
    pub error: Option<String>,
}

/// A pending destructive action awaiting user confirmation.
#[derive(Debug, Clone)]
pub enum PendingConfirm {
    /// Delete the field at the given index (into module.fields).
    DeleteField {
        field_index: usize,
        field_name: String,
    },
    /// Delete the entire module.
    DeleteModule { module_key: String },
    /// Delete the sub-field at the given index within a composite_array field.
    DeleteSubField {
        field_index: usize,
        sub_field_index: usize,
        sub_field_name: String,
    },
}

/// State for the module configure screen.
#[derive(Debug)]
pub struct ConfigureState {
    pub module_key: String,
    /// Which level of the configure hierarchy is active.
    pub level: ConfigureLevel,
    pub active_field: usize,
    pub editing: bool,
    pub edit_buffer: String,
    /// Saved value before entering edit mode (used to restore on Esc).
    pub edit_original: String,
    pub cursor_position: usize,
    pub browser_open: bool,
    pub browser_state: Option<BrowserState>,
    /// Horizontal scroll offset for the inline edit buffer (chars).
    pub scroll_offset: usize,
    /// Whether the list editor overlay is open (for ListEditor fields).
    pub list_editor_open: bool,
    /// Multi-line buffer for the list editor (one item per line).
    pub list_editor_buffer: String,
    /// Cursor line in the list editor.
    pub list_editor_cursor_line: usize,
    /// Cursor column in the list editor.
    pub list_editor_cursor_col: usize,
    /// A destructive action awaiting y/n confirmation.
    pub confirm: Option<PendingConfirm>,
    pub dirty: bool,
    pub settings: Vec<ConfigSetting>,
    /// Non-fatal status message to show in the footer (e.g. save errors).
    pub status_message: Option<String>,
    /// Whether the path placeholder help overlay is visible.
    pub help_overlay_open: bool,
    /// Whether the quick-select overlay is open (for QuickSelect fields).
    pub quick_select_open: bool,
}

/// Central application state, holding config, transport, and all screen state.
pub struct App {
    pub config: Config,
    pub transport: Transport,
    /// Which screen is currently displayed.
    pub screen: Screen,
    /// Index into `module_keys` for the currently selected module.
    pub selected_module: usize,
    /// Form state, present when `screen == Screen::Form`.
    pub form_state: Option<FormState>,
    /// Summary state, present when `screen == Screen::Summary`.
    pub summary_state: Option<SummaryState>,
    /// Configure state, present when `screen == Screen::Configure`.
    pub configure_state: Option<ConfigureState>,
    /// Sorted module keys for deterministic ordering in the dashboard.
    pub module_keys: Vec<String>,
    /// Path validation warnings collected at startup; shown as a dashboard overlay until dismissed.
    pub startup_warnings: Vec<String>,
    /// Capture history for ambient dashboard stats.
    pub history: History,
    /// Whether the dashboard help overlay is visible.
    pub help_open: bool,
    /// Saved presets for all modules.
    pub presets: Presets,
    /// Saved per-field presets for `composite_array` fields, keyed by
    /// `"<module>.<field>"`.
    pub field_presets: FieldPresets,
    /// Messages deferred from async tasks (e.g. autocreate) that must not be
    /// printed while TUI raw mode is active. Drained and written to stderr by
    /// main after `ratatui::restore()`.
    pub deferred_stderr: Vec<String>,
    /// Ephemeral warning toast displayed at the bottom of every TUI screen.
    /// Cleared automatically when `expires_at` has passed.
    pub status_message: Option<StatusMessage>,
}

impl App {
    /// Create a new App with the given config and transport.
    ///
    /// Starts on the Dashboard screen with the first module selected.
    /// Module keys are ordered by `module_order` from config if present,
    /// with any unlisted modules appended alphabetically.
    pub fn new(
        config: Config,
        transport: Transport,
        history: History,
        presets: Presets,
        field_presets: FieldPresets,
    ) -> Self {
        let module_keys = crate::init::module_order(&config);

        App {
            config,
            transport,
            screen: Screen::Dashboard,
            selected_module: 0,
            form_state: None,
            summary_state: None,
            configure_state: None,
            module_keys,
            startup_warnings: Vec::new(),
            history,
            help_open: false,
            presets,
            field_presets,
            deferred_stderr: Vec::new(),
            status_message: None,
        }
    }

    /// Set an ephemeral warning toast that auto-expires after [`STATUS_TOAST_DURATION`].
    pub fn set_status_warning(&mut self, msg: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            text: msg.into(),
            expires_at: Instant::now() + STATUS_TOAST_DURATION,
        });
    }

    /// Clear `status_message` if it has expired. Called each event-loop tick.
    pub fn tick_status(&mut self) {
        if let Some(ref msg) = self.status_message
            && Instant::now() >= msg.expires_at
        {
            self.status_message = None;
        }
    }

    /// Initialize form state for the given module key.
    ///
    /// Populates default values from field config and pre-fills options
    /// for `static_select` fields. Returns `None` if the module key is
    /// not found in config.
    pub fn init_form(&self, module_key: &str) -> Option<FormState> {
        let module = self.config.modules.get(module_key)?;

        let mut field_values = HashMap::new();
        let mut field_options = HashMap::new();
        let mut composite_values = HashMap::new();
        let mut callout_overrides = HashMap::new();
        let mut callout_titles = HashMap::new();

        for field in &module.fields {
            if field.field_type == FieldType::CompositeArray {
                // Composite fields store data in composite_values, not field_values
                composite_values.insert(field.name.clone(), Vec::new());
                continue;
            }

            // Set default value if configured
            let default_val = field.default.clone().unwrap_or_default();
            field_values.insert(field.name.clone(), default_val);

            // Pre-populate options for static_select fields
            if field.field_type == FieldType::StaticSelect
                && let Some(opts) = &field.options
            {
                field_options.insert(field.name.clone(), opts.clone());
            }

            // Seed callout overrides from config defaults
            if let Some(ref callout) = field.callout {
                callout_overrides.insert(field.name.clone(), callout.clone());
            }
            if let Some(ref title) = field.callout_title {
                callout_titles.insert(field.name.clone(), title.clone());
            }
        }

        if let Some(ref callout) = module.callout_type {
            callout_overrides.insert("_callout_type".to_string(), callout.clone());
        }

        // Determine the config index for active_field=0 given initial (default) values.
        let initial_visible =
            crate::visibility::visible_field_indices(&module.fields, &field_values);
        let initial_config_idx = initial_visible.first().copied();

        // Populate preset names (and descriptions) from saved presets for this module.
        let saved_presets = self.presets.get(module_key);
        let preset_names: Vec<String> = saved_presets.iter().map(|p| p.name.clone()).collect();
        let preset_descriptions: Vec<Option<String>> = saved_presets
            .iter()
            .map(|p| p.description.clone())
            .collect();

        // Start on the first real field (active_field 1), not the preset row (0).
        // The preset row is always visible at position 0 but is not the default focus.
        let start_field = if initial_config_idx.is_some() { 1 } else { 0 };

        // Validate preset_axes at form-open time. Invalid axes produce warnings and
        // disable the picker (legacy cycler resumes).
        let axis_warnings: Vec<String> = if module.preset_axes.is_empty() {
            Vec::new()
        } else {
            match crate::data::preset_tree::validate_axes(&module.preset_axes, &module.fields) {
                Ok(()) => Vec::new(),
                Err(errors) => errors
                    .iter()
                    .map(|e| match e {
                        crate::data::preset_tree::AxisError::UnknownField(n) => {
                            format!("unknown field \"{n}\"")
                        }
                        crate::data::preset_tree::AxisError::CompositeArrayField(n) => {
                            format!("composite_array field \"{n}\" cannot be an axis")
                        }
                        crate::data::preset_tree::AxisError::PresetExcludedField(n) => {
                            format!("preset_exclude field \"{n}\" cannot be an axis")
                        }
                    })
                    .collect(),
            }
        };

        Some(FormState {
            field_values,
            field_options,
            active_field: start_field,
            active_config_idx: initial_config_idx,
            validation_errors: Vec::new(),
            axis_warnings,
            cursor_position: 0,
            dropdown_open: false,
            textarea_open: false,
            textarea_scroll_offset: 0,
            callout_overrides,
            callout_titles,
            callout_title_edit: None,
            composite_values,
            composite_open: false,
            composite_row: 0,
            composite_col: 0,
            search_buffers: HashMap::new(),
            sub_form: None,
            preset_names,
            preset_descriptions,
            selected_preset_name: None,
            preset_picker: None,
            preset_overlay: None,
            confirm_delete_preset: false,
            field_preset_picker: None,
            last_applied_field_preset: HashMap::new(),
            composite_status: None,
            priors_panel: None,
            priors_collapsed: false,
        })
    }

    /// Initialize configure state for the given module key.
    ///
    /// Builds a settings list from the module's current config values.
    /// Returns `None` if the module key is not found in config.
    pub fn init_configure(&self, module_key: &str) -> Option<ConfigureState> {
        let module = self.config.modules.get(module_key)?;

        let mode_str = match module.mode {
            WriteMode::Append => "append".to_string(),
            WriteMode::Create => "create".to_string(),
        };

        let mut settings = vec![
            ConfigSetting {
                label: "Path".to_string(),
                key: "path".to_string(),
                value: module.path.clone(),
                kind: SettingKind::Path,
            },
            ConfigSetting {
                label: "Display Name".to_string(),
                key: "display_name".to_string(),
                value: module.display_name.clone().unwrap_or_default(),
                kind: SettingKind::Text,
            },
            ConfigSetting {
                label: "Mode".to_string(),
                key: "mode".to_string(),
                value: mode_str.clone(),
                kind: SettingKind::Toggle(vec!["append".to_string(), "create".to_string()]),
            },
        ];

        // Only show append_under_header when mode is append
        if mode_str == "append" {
            settings.push(ConfigSetting {
                label: "Append Header".to_string(),
                key: "append_under_header".to_string(),
                value: module.append_under_header.clone().unwrap_or_default(),
                kind: SettingKind::Text,
            });
        }

        // Callout type — useful for both modes (append template or field-level)
        settings.push(ConfigSetting {
            label: "Callout Type".to_string(),
            key: "callout_type".to_string(),
            value: module.callout_type.clone().unwrap_or_default(),
            kind: SettingKind::QuickSelect(callout_quick_select()),
        });

        settings.push(ConfigSetting {
            label: "Icon".to_string(),
            key: "icon".to_string(),
            value: module.icon.clone().unwrap_or_default(),
            kind: SettingKind::Text,
        });

        settings.push(ConfigSetting {
            label: "Daily Link".to_string(),
            key: "daily_link".to_string(),
            value: if module.daily_link == Some(true) {
                "true".to_string()
            } else {
                String::new()
            },
            kind: SettingKind::Toggle(vec![String::new(), "true".to_string()]),
        });

        // Only show append_shallow when mode is append
        if mode_str == "append" {
            settings.push(ConfigSetting {
                label: "Shallow Append".to_string(),
                key: "append_shallow".to_string(),
                value: if module.append_shallow == Some(true) {
                    "true".to_string()
                } else {
                    String::new()
                },
                kind: SettingKind::Toggle(vec![String::new(), "true".to_string()]),
            });
        }

        settings.push(ConfigSetting {
            label: "Mobile Visible".to_string(),
            key: "mobile_visible".to_string(),
            // Default is true (visible). Show "false" only when explicitly set false.
            value: if module.mobile_visible == Some(false) {
                "false".to_string()
            } else {
                String::new()
            },
            // Cycles: "" (default / visible) ◂ ▸ "false" (hidden)
            kind: SettingKind::Toggle(vec![String::new(), "false".to_string()]),
        });

        // Navigation link to the field list
        let field_count = module.fields.len();
        settings.push(ConfigSetting {
            label: "Fields".to_string(),
            key: "fields".to_string(),
            value: format!(
                "{field_count} field{}",
                if field_count == 1 { "" } else { "s" }
            ),
            kind: SettingKind::NavLink,
        });

        Some(ConfigureState {
            module_key: module_key.to_string(),
            level: ConfigureLevel::ModuleSettings,
            active_field: 0,
            editing: false,
            edit_buffer: String::new(),
            edit_original: String::new(),
            cursor_position: 0,
            browser_open: false,
            browser_state: None,
            scroll_offset: 0,
            list_editor_open: false,
            list_editor_buffer: String::new(),
            list_editor_cursor_line: 0,
            list_editor_cursor_col: 0,
            confirm: None,
            dirty: false,
            settings,
            status_message: None,
            help_overlay_open: false,
            quick_select_open: false,
        })
    }

    /// Build settings list for editing a specific field's properties.
    ///
    /// Thin wrapper — delegates to `crate::tui::configure::init::build_field_settings`.
    /// Kept on `App` for backward-compat with test callsites.
    pub fn build_field_settings(field: &crate::config::FieldConfig) -> Vec<ConfigSetting> {
        crate::tui::configure::init::build_field_settings(field)
    }

    /// Build settings list for editing a specific sub-field's properties.
    ///
    /// Thin wrapper — delegates to `crate::tui::configure::init::build_sub_field_settings`.
    pub fn build_sub_field_settings(
        sub_field: &crate::config::SubFieldConfig,
    ) -> Vec<ConfigSetting> {
        crate::tui::configure::init::build_sub_field_settings(sub_field)
    }

    /// Build `ConfigureState` for vault-level configuration.
    ///
    /// Thin wrapper — delegates to `crate::tui::configure::init::init_vault_configure`.
    pub fn init_vault_configure(&self) -> ConfigureState {
        crate::tui::configure::init::init_vault_configure(&self.config)
    }

    /// Initialize configure state for creating a new module.
    ///
    /// Thin wrapper — delegates to `crate::tui::configure::init::init_new_module_configure`.
    pub fn init_new_module_configure(&self) -> ConfigureState {
        crate::tui::configure::init::init_new_module_configure(&self.config)
    }

    /// Apply a preset (or `None` for `<none>`) to the given form state.
    ///
    /// Rules:
    /// - Fields with `preset_exclude = true` are never touched.
    /// - `CompositeArray` fields are never touched (no scalar representation).
    /// - When `preset` is `Some`, all eligible fields are set: fields present in the preset
    ///   receive the preset value, fields absent from the preset are reset to their config
    ///   default (or empty string). This makes preset application deterministic — the result
    ///   does not depend on previous form state.
    /// - When `preset` is `None`, all eligible fields are reset to their config default
    ///   (or empty string if no default is configured).
    /// - After applying, `show_when` visibility is re-evaluated: `active_config_idx` is updated
    ///   to stay within the newly visible set (moves to first visible field when the current
    ///   focused field becomes hidden).
    /// - UI state (`cursor_position`, `dropdown_open`, `textarea_open`, `search_buffers`) is
    ///   reset to avoid stale state from the previous form values.
    pub fn apply_preset(
        form_state: &mut FormState,
        fields: &[crate::config::FieldConfig],
        preset: Option<&crate::data::presets::PresetEntry>,
    ) {
        for field in fields {
            // Never touch excluded or composite fields.
            if field.preset_exclude == Some(true) {
                continue;
            }
            if field.field_type == FieldType::CompositeArray {
                continue;
            }

            let new_val = match preset {
                Some(p) => p
                    .values
                    .get(&field.name)
                    .cloned()
                    .unwrap_or_else(|| field.default.clone().unwrap_or_default()),
                None => field.default.clone().unwrap_or_default(),
            };
            form_state.field_values.insert(field.name.clone(), new_val);
        }

        // Reset UI state to avoid stale cursor/overlay positions.
        form_state.cursor_position = 0;
        form_state.dropdown_open = false;
        form_state.textarea_open = false;
        form_state.search_buffers.clear();

        // Re-evaluate visibility and fix up focus if the active field became hidden.
        let visible = crate::visibility::visible_field_indices(fields, &form_state.field_values);

        // Map the current active_config_idx back to a visible position.
        let current_visible_pos = form_state
            .active_config_idx
            .and_then(|cfg_idx| visible.iter().position(|&v| v == cfg_idx));

        if let Some(pos) = current_visible_pos {
            // Field is still visible — keep focus where it is.
            form_state.active_field = pos;
        } else {
            // Field became hidden — move to first visible field.
            form_state.active_field = 0;
            form_state.active_config_idx = visible.first().copied();
        }
    }

    /// Save a per-field preset (composite_array rows) for the active module.
    ///
    /// Upserts by name into `field_presets` (in-memory update always happens),
    /// then attempts to persist to disk. Returns the disk error if it fails so
    /// the caller can surface it via the status toast; in-memory state is
    /// already updated regardless of the disk result.
    pub fn save_field_preset(
        &mut self,
        field_name: &str,
        name: &str,
        description: Option<String>,
        rows: Vec<Vec<String>>,
    ) -> anyhow::Result<()> {
        let module_key = match self.module_keys.get(self.selected_module) {
            Some(k) => k.clone(),
            None => return Ok(()),
        };
        let key = crate::data::field_presets::preset_key(&module_key, field_name);

        let entry = crate::data::field_presets::FieldPresetEntry {
            name: name.to_string(),
            description,
            rows,
        };
        // In-memory update always succeeds.
        self.field_presets.set(&key, entry);

        // Update form state subtitle unconditionally.
        if let Some(ref mut fs) = self.form_state {
            fs.last_applied_field_preset
                .insert(field_name.to_string(), name.to_string());
            fs.composite_status = Some(format!("saved preset \u{201c}{name}\u{201d}"));
        }

        // Disk persist — errors returned to caller for toast display.
        self.field_presets.save()
    }

    /// Apply a saved per-field preset to the named composite_array field.
    ///
    /// Replaces the rows of `field_name` silently (no confirm). Reconciles
    /// the saved row shape against the current sub_field count, padding or
    /// truncating cells as needed; sets a status message when adjusted.
    /// No-op if the preset is not found or the form isn't open.
    pub fn apply_field_preset(&mut self, field_name: &str, preset_name: &str) {
        let module_key = match self.module_keys.get(self.selected_module) {
            Some(k) => k.clone(),
            None => return,
        };
        let key = crate::data::field_presets::preset_key(&module_key, field_name);
        let entry = match self
            .field_presets
            .get(&key)
            .into_iter()
            .find(|p| p.name == preset_name)
        {
            Some(e) => e,
            None => return,
        };

        let sub_field_count = self
            .config
            .modules
            .get(&module_key)
            .and_then(|m| m.fields.iter().find(|f| f.name == field_name))
            .and_then(|f| f.sub_fields.as_ref())
            .map(|s| s.len())
            .unwrap_or(0);

        let (rows, adjusted) =
            crate::data::field_presets::reconcile_rows(entry.rows.clone(), sub_field_count);

        if let Some(ref mut fs) = self.form_state {
            fs.composite_values.insert(field_name.to_string(), rows);
            fs.composite_row = 0;
            fs.composite_col = 0;
            fs.cursor_position = 0;
            fs.last_applied_field_preset
                .insert(field_name.to_string(), preset_name.to_string());
            fs.composite_status = if adjusted {
                Some("preset shape adjusted to current schema".to_string())
            } else {
                None
            };
        }
    }

    /// Delete a saved per-field preset by name. If the picker is open it is
    /// re-populated; if the deleted preset was the last-applied one, that
    /// marker is cleared so the subtitle stops referencing it. In-memory state
    /// is always updated; returns a disk error if persistence fails so the
    /// caller can surface it via the status toast.
    pub fn delete_field_preset(
        &mut self,
        field_name: &str,
        preset_name: &str,
    ) -> anyhow::Result<()> {
        let module_key = match self.module_keys.get(self.selected_module) {
            Some(k) => k.clone(),
            None => return Ok(()),
        };
        let key = crate::data::field_presets::preset_key(&module_key, field_name);
        // In-memory update always happens.
        self.field_presets.delete(&key, preset_name);

        let entries = self.field_presets.get(&key);
        if let Some(ref mut fs) = self.form_state {
            if fs
                .last_applied_field_preset
                .get(field_name)
                .map(|n| n == preset_name)
                .unwrap_or(false)
            {
                fs.last_applied_field_preset.remove(field_name);
            }
            if let Some(picker) = &mut fs.field_preset_picker {
                picker.names = entries.iter().map(|p| p.name.clone()).collect();
                picker.descriptions = entries.iter().map(|p| p.description.clone()).collect();
                if picker.names.is_empty() {
                    fs.field_preset_picker = None;
                    fs.composite_status = Some("preset deleted".to_string());
                } else if picker.selected >= picker.names.len() {
                    picker.selected = picker.names.len() - 1;
                }
            }
        }

        // Disk persist — errors returned to caller for toast display.
        self.field_presets.save()
    }

    /// Validate form state against the module's field requirements.
    ///
    /// Returns a list of error messages. An empty list means validation passed.
    /// Only visible fields (per `show_when` rules) are validated — hidden required fields
    /// do not block submission.
    pub fn validate_form(module: &ModuleConfig, form_state: &FormState) -> Vec<String> {
        let mut errors = Vec::new();

        let visible_indices = visible_field_indices(&module.fields, &form_state.field_values);
        let visible_names: HashSet<&str> = visible_indices
            .iter()
            .map(|&i| module.fields[i].name.as_str())
            .collect();

        for field in &module.fields {
            // Skip fields that are currently hidden
            if !visible_names.contains(field.name.as_str()) {
                continue;
            }
            // Composite array fields have their own validation path
            if field.field_type == FieldType::CompositeArray {
                let rows = form_state
                    .composite_values
                    .get(&field.name)
                    .cloned()
                    .unwrap_or_default();

                // Strip empty rows (all cells blank)
                let non_empty: Vec<&Vec<String>> = rows
                    .iter()
                    .filter(|row| row.iter().any(|cell| !cell.trim().is_empty()))
                    .collect();

                let is_required = field.required.unwrap_or(false);
                if is_required && non_empty.is_empty() {
                    errors.push(format!("'{}' requires at least one row", field.prompt));
                    continue;
                }

                // Validate number sub-fields per row
                if let Some(subs) = &field.sub_fields {
                    for (row_idx, row) in non_empty.iter().enumerate() {
                        for (col_idx, sub) in subs.iter().enumerate() {
                            if sub.field_type == SubFieldType::Number {
                                let cell = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                                if !cell.trim().is_empty() && cell.trim().parse::<f64>().is_err() {
                                    errors.push(format!(
                                        "'{}' row {}: '{}' must be a valid number",
                                        field.prompt,
                                        row_idx + 1,
                                        sub.prompt
                                    ));
                                }
                            }
                        }
                    }
                }

                continue;
            }

            let value = form_state
                .field_values
                .get(&field.name)
                .map(|s| s.as_str())
                .unwrap_or("");

            // Check required fields
            let is_required = field.required.unwrap_or(false);
            if is_required && value.trim().is_empty() {
                errors.push(format!("'{}' is required", field.prompt));
                continue;
            }

            // Check number fields parse correctly (skip empty optional fields)
            if field.field_type == FieldType::Number
                && !value.trim().is_empty()
                && value.trim().parse::<f64>().is_err()
            {
                errors.push(format!("'{}' must be a valid number", field.prompt));
            }
        }

        errors
    }
}
