use crate::config::{Config, FieldType, ModuleConfig};
use crate::transport::{Transport, TransportMode};
use std::collections::HashMap;

/// Which screen the TUI is currently displaying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Form,
    Summary,
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
    /// Sorted module keys for deterministic ordering in the dashboard.
    pub module_keys: Vec<String>,
}

impl App {
    /// Create a new App with the given config and transport.
    ///
    /// Starts on the Dashboard screen with the first module selected.
    /// Module keys are sorted alphabetically for stable, predictable ordering.
    pub fn new(config: Config, transport: Transport) -> Self {
        let mut module_keys: Vec<String> = config.modules.keys().cloned().collect();
        module_keys.sort();

        App {
            config,
            transport,
            screen: Screen::Dashboard,
            selected_module: 0,
            form_state: None,
            summary_state: None,
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
