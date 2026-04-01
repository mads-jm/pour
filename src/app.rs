use crate::config::{Config, FieldType, ModuleConfig, WriteMode};
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

/// The kind of input widget for a configure setting.
#[derive(Debug, Clone)]
pub enum SettingKind {
    Text,
    Path,
    Toggle(Vec<String>),
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

/// State for the module configure screen.
#[derive(Debug)]
pub struct ConfigureState {
    pub module_key: String,
    pub active_field: usize,
    pub editing: bool,
    pub edit_buffer: String,
    /// Saved value before entering edit mode (used to restore on Esc).
    pub edit_original: String,
    pub cursor_position: usize,
    pub browser_open: bool,
    pub browser_state: Option<BrowserState>,
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
}

impl App {
    /// Create a new App with the given config and transport.
    ///
    /// Starts on the Dashboard screen with the first module selected.
    /// Module keys are ordered by `module_order` from config if present,
    /// with any unlisted modules appended alphabetically.
    pub fn new(config: Config, transport: Transport) -> Self {
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

        for field in &module.fields {
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

        Some(ConfigureState {
            module_key: module_key.to_string(),
            active_field: 0,
            editing: false,
            edit_buffer: String::new(),
            edit_original: String::new(),
            cursor_position: 0,
            browser_open: false,
            browser_state: None,
            dirty: false,
            settings,
            status_message: None,
        })
    }

    /// Validate form state against the module's field requirements.
    ///
    /// Returns a list of error messages. An empty list means validation passed.
    pub fn validate_form(module: &ModuleConfig, form_state: &FormState) -> Vec<String> {
        let mut errors = Vec::new();

        for field in &module.fields {
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
