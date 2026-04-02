use crate::config::{Config, FieldType, ModuleConfig, SubFieldType, WriteMode};
use crate::data::history::History;
use crate::transport::{Transport, TransportMode, VaultEntry};
use std::collections::HashMap;

/// Which screen the TUI is currently displaying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Form,
    Summary,
    Configure,
}

/// State for the module entry form.
#[derive(Debug)]
pub struct FormState {
    /// Current value for each field, keyed by field name.
    pub field_values: HashMap<String, String>,
    /// Available options for select fields, keyed by field name.
    pub field_options: HashMap<String, Vec<String>>,
    /// Index of the currently active (focused) field.
    pub active_field: usize,
    /// Validation error messages, populated on submit attempt.
    pub validation_errors: Vec<String>,
    /// Cursor position within the active text/number input.
    pub cursor_position: usize,
    /// Whether the dropdown for the current select field is open.
    pub dropdown_open: bool,
    /// Whether the textarea editor overlay is open.
    pub textarea_open: bool,
    /// Horizontal scroll offset for the textarea editor (chars).
    pub textarea_scroll_offset: usize,
    /// Row data for composite_array fields, keyed by field name.
    /// Each row is a Vec of cell values (one per sub-field column).
    pub composite_values: HashMap<String, Vec<Vec<String>>>,
    /// Whether the composite_array editor overlay is open.
    pub composite_open: bool,
    /// Currently selected row in the composite overlay.
    pub composite_row: usize,
    /// Currently selected column in the composite overlay.
    pub composite_col: usize,
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
    pub loading: bool,
}

/// A pending destructive action awaiting user confirmation.
#[derive(Debug, Clone)]
pub enum PendingConfirm {
    /// Delete the field at the given index (into module.fields).
    DeleteField { field_index: usize, field_name: String },
    /// Delete the entire module.
    DeleteModule { module_key: String },
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
}

impl App {
    /// Create a new App with the given config and transport.
    ///
    /// Starts on the Dashboard screen with the first module selected.
    /// Module keys are ordered by `module_order` from config if present,
    /// with any unlisted modules appended alphabetically.
    pub fn new(config: Config, transport: Transport, history: History) -> Self {
        let module_keys = match &config.module_order {
            Some(order) => {
                let mut keys: Vec<String> = order
                    .iter()
                    .filter(|k| config.modules.contains_key(k.as_str()))
                    .cloned()
                    .collect();
                let mut rest: Vec<String> = config
                    .modules
                    .keys()
                    .filter(|k| !order.contains(k))
                    .cloned()
                    .collect();
                rest.sort();
                keys.extend(rest);
                keys
            }
            None => {
                let mut keys: Vec<String> = config.modules.keys().cloned().collect();
                keys.sort();
                keys
            }
        };

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
        }

        Some(FormState {
            field_values,
            field_options,
            active_field: 0,
            validation_errors: Vec::new(),
            cursor_position: 0,
            dropdown_open: false,
            textarea_open: false,
            textarea_scroll_offset: 0,
            composite_values,
            composite_open: false,
            composite_row: 0,
            composite_col: 0,
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

        // Navigation link to the field list
        let field_count = module.fields.len();
        settings.push(ConfigSetting {
            label: "Fields".to_string(),
            key: "fields".to_string(),
            value: format!("{field_count} field{}", if field_count == 1 { "" } else { "s" }),
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
        })
    }

    /// Build settings list for editing a specific field's properties.
    ///
    /// Replaces the current `settings` in `ConfigureState` with settings
    /// derived from the field at `field_index`. Type-conditional settings
    /// (options, source) are included based on the field's current type.
    pub fn build_field_settings(field: &crate::config::FieldConfig) -> Vec<ConfigSetting> {
        let type_str = match field.field_type {
            FieldType::Text => "text",
            FieldType::Textarea => "textarea",
            FieldType::Number => "number",
            FieldType::StaticSelect => "static_select",
            FieldType::DynamicSelect => "dynamic_select",
            FieldType::CompositeArray => "composite_array",
        };

        let target_str = match &field.target {
            Some(crate::config::FieldTarget::Frontmatter) => "frontmatter",
            Some(crate::config::FieldTarget::Body) => "body",
            None => "",
        };

        let mut settings = vec![
            ConfigSetting {
                label: "Name".to_string(),
                key: "name".to_string(),
                value: field.name.clone(),
                kind: SettingKind::Text,
            },
            ConfigSetting {
                label: "Prompt".to_string(),
                key: "prompt".to_string(),
                value: field.prompt.clone(),
                kind: SettingKind::Text,
            },
            ConfigSetting {
                label: "Type".to_string(),
                key: "field_type".to_string(),
                value: type_str.to_string(),
                kind: SettingKind::Toggle(vec![
                    "text".to_string(),
                    "textarea".to_string(),
                    "number".to_string(),
                    "static_select".to_string(),
                    "dynamic_select".to_string(),
                    "composite_array".to_string(),
                ]),
            },
            ConfigSetting {
                label: "Required".to_string(),
                key: "required".to_string(),
                value: if field.required.unwrap_or(false) {
                    "true".to_string()
                } else {
                    "false".to_string()
                },
                kind: SettingKind::Toggle(vec!["false".to_string(), "true".to_string()]),
            },
            ConfigSetting {
                label: "Default".to_string(),
                key: "default".to_string(),
                value: field.default.clone().unwrap_or_default(),
                kind: SettingKind::Text,
            },
            ConfigSetting {
                label: "Target".to_string(),
                key: "target".to_string(),
                value: target_str.to_string(),
                kind: SettingKind::Toggle(vec![
                    String::new(),
                    "frontmatter".to_string(),
                    "body".to_string(),
                ]),
            },
        ];

        // Type-conditional settings
        if field.field_type == FieldType::StaticSelect {
            let opts_display = field
                .options
                .as_ref()
                .map(|o| o.join("\n"))
                .unwrap_or_default();
            settings.push(ConfigSetting {
                label: "Options".to_string(),
                key: "options".to_string(),
                value: opts_display,
                kind: SettingKind::ListEditor,
            });
        }

        if field.field_type == FieldType::DynamicSelect {
            settings.push(ConfigSetting {
                label: "Source".to_string(),
                key: "source".to_string(),
                value: field.source.clone().unwrap_or_default(),
                kind: SettingKind::Path,
            });
        }

        settings
    }

    /// Build settings list for editing vault-level configuration.
    ///
    /// Returns a `ConfigureState` ready to be used with `ConfigureLevel::VaultSettings`.
    /// The `module_key` is set to `"__vault__"` (not a real module).
    pub fn init_vault_configure(&self) -> ConfigureState {
        let vault = &self.config.vault;

        // For api_key, use the config-file value rather than the in-memory
        // value which may include POUR_API_KEY env var override. This prevents
        // leaking env var secrets into the config file on save.
        let api_key_from_file = if std::env::var("POUR_API_KEY").is_ok() {
            // Env var is set — read the raw file value instead of the override.
            std::fs::read_to_string(Config::default_config_path())
                .ok()
                .and_then(|content| {
                    let doc = content.parse::<toml_edit::DocumentMut>().ok()?;
                    let key = doc.get("vault")?.get("api_key")?.as_str()?;
                    Some(key.to_string())
                })
                .unwrap_or_default()
        } else {
            vault.api_key.clone().unwrap_or_default()
        };

        let settings = vec![
            ConfigSetting {
                label: "Base Path".to_string(),
                key: "base_path".to_string(),
                value: vault.base_path.clone(),
                kind: SettingKind::Text,
            },
            ConfigSetting {
                label: "API Port".to_string(),
                key: "api_port".to_string(),
                value: vault
                    .api_port
                    .map(|p| p.to_string())
                    .unwrap_or_default(),
                kind: SettingKind::Text,
            },
            ConfigSetting {
                label: "API Key".to_string(),
                key: "api_key".to_string(),
                value: api_key_from_file,
                kind: SettingKind::Text,
            },
            ConfigSetting {
                label: "Date Format".to_string(),
                key: "date_format".to_string(),
                value: vault.date_format.clone().unwrap_or_else(|| "%Y%m%d".to_string()),
                kind: SettingKind::Text,
            },
        ];

        ConfigureState {
            module_key: "__vault__".to_string(),
            level: ConfigureLevel::VaultSettings,
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
        }
    }

    /// Initialize configure state for creating a new module.
    ///
    /// The returned `ConfigureState` has an empty `module_key` — it will be set
    /// by the user via the "Module Key" setting.
    pub fn init_new_module_configure(&self) -> ConfigureState {
        let settings = vec![
            ConfigSetting {
                label: "Module Key".to_string(),
                key: "module_key".to_string(),
                value: String::new(),
                kind: SettingKind::Identifier,
            },
            ConfigSetting {
                label: "Display Name".to_string(),
                key: "display_name".to_string(),
                value: String::new(),
                kind: SettingKind::Text,
            },
            ConfigSetting {
                label: "Mode".to_string(),
                key: "mode".to_string(),
                value: "create".to_string(),
                kind: SettingKind::Toggle(vec!["append".to_string(), "create".to_string()]),
            },
            ConfigSetting {
                label: "Path".to_string(),
                key: "path".to_string(),
                value: String::new(),
                kind: SettingKind::Path,
            },
        ];

        ConfigureState {
            module_key: String::new(),
            level: ConfigureLevel::NewModule,
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
        }
    }

    /// Validate form state against the module's field requirements.
    ///
    /// Returns a list of error messages. An empty list means validation passed.
    pub fn validate_form(module: &ModuleConfig, form_state: &FormState) -> Vec<String> {
        let mut errors = Vec::new();

        for field in &module.fields {
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
