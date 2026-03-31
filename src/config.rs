use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// Top-level configuration, deserialized from `config.toml`.
#[derive(Debug, Deserialize)]
pub struct Config {
    pub vault: VaultConfig,
    pub modules: HashMap<String, ModuleConfig>,
}

/// Vault connection settings.
/// TODO : need to persist vault name here?
#[derive(Debug, Deserialize)]
pub struct VaultConfig {
    pub base_path: String,
    #[serde(default = "default_api_port")]
    pub api_port: Option<u16>,
    pub api_key: Option<String>,
}

fn default_api_port() -> Option<u16> {
    Some(27124)
}

/// A single module definition (e.g. `[modules.coffee]`).
#[derive(Debug, Deserialize)]
pub struct ModuleConfig {
    pub mode: WriteMode,
    pub path: String,
    pub append_under_header: Option<String>,
    pub append_template: Option<String>,
    pub fields: Vec<FieldConfig>,
    pub display_name: Option<String>,
}

/// Whether a module appends to an existing note or creates a new one.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    Append,
    Create,
}

/// A single field within a module form.
#[derive(Debug, Deserialize)]
pub struct FieldConfig {
    pub name: String,
    pub field_type: FieldType,
    pub prompt: String,
    pub required: Option<bool>,
    pub default: Option<String>,
    /// Valid only for `static_select`.
    pub options: Option<Vec<String>>,
    /// Directory path for `dynamic_select` data source.
    pub source: Option<String>,
    /// Where this field's value is written. Defaults depend on `field_type`.
    pub target: Option<FieldTarget>,
}

/// The kind of input widget for a field.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Textarea,
    Number,
    StaticSelect,
    DynamicSelect,
}

/// Controls whether a field value goes into YAML frontmatter or the Markdown body.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldTarget {
    Frontmatter,
    Body,
}

/// Errors that can occur when loading or validating configuration.
#[derive(Debug)]
pub enum ConfigError {
    /// The config file was not found at the expected path.
    NotFound(PathBuf),
    /// Failed to read the config file.
    ReadError(std::io::Error),
    /// Failed to parse the TOML content.
    ParseError(toml::de::Error),
    /// One or more validation rules were violated.
    ValidationError(Vec<String>),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NotFound(path) => {
                write!(f, "config file not found: {}", path.display())
            }
            ConfigError::ReadError(err) => write!(f, "failed to read config: {err}"),
            ConfigError::ParseError(err) => write!(f, "failed to parse config: {err}"),
            ConfigError::ValidationError(errors) => {
                writeln!(f, "config validation failed:")?;
                for e in errors {
                    writeln!(f, "  - {e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Load and validate the configuration.
    ///
    /// Resolution order for config file path:
    /// 1. `POUR_CONFIG` environment variable (if set)
    /// 2. `~/.config/pour/config.toml` (via `dirs::config_dir()`)
    ///
    /// The `api_key` is resolved from `POUR_API_KEY` env var first, falling
    /// back to whatever is in the config file.
    pub fn load() -> Result<Config, ConfigError> {
        let path = Self::resolve_config_path()?;

        let content = std::fs::read_to_string(&path).map_err(ConfigError::ReadError)?;

        Self::from_toml(&content)
    }

    /// Parse and validate a config from a TOML string.
    /// Also applies `POUR_API_KEY` env var override.
    pub fn from_toml(toml_content: &str) -> Result<Config, ConfigError> {
        let mut config: Config = toml::from_str(toml_content).map_err(ConfigError::ParseError)?;

        // Resolve api_key: env var takes precedence over config file
        if let Ok(env_key) = std::env::var("POUR_API_KEY")
            && !env_key.is_empty()
        {
            config.vault.api_key = Some(env_key);
        }

        config.validate()?;

        Ok(config)
    }

    /// Determine the config file path, checking `POUR_CONFIG` env var first.
    fn resolve_config_path() -> Result<PathBuf, ConfigError> {
        if let Ok(env_path) = std::env::var("POUR_CONFIG") {
            let path = PathBuf::from(env_path);
            if path.exists() {
                return Ok(path);
            }
            return Err(ConfigError::NotFound(path));
        }

        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::NotFound(PathBuf::from("~/.config/pour/config.toml")))?;

        let path = config_dir.join("pour").join("config.toml");
        if path.exists() {
            Ok(path)
        } else {
            Err(ConfigError::NotFound(path))
        }
    }

    /// Validate the parsed config against business rules.
    fn validate(&self) -> Result<(), ConfigError> {
        let mut errors = Vec::new();

        for (name, module) in &self.modules {
            // Every module must have at least one field
            if module.fields.is_empty() {
                errors.push(format!("module '{name}': must have at least one field"));
            }

            // Append mode requires append_under_header
            if module.mode == WriteMode::Append && module.append_under_header.is_none() {
                errors.push(format!(
                    "module '{name}': append mode requires 'append_under_header'"
                ));
            }

            for field in &module.fields {
                // static_select must have non-empty options
                if field.field_type == FieldType::StaticSelect {
                    match &field.options {
                        None => {
                            errors.push(format!(
                                "module '{name}', field '{}': static_select requires 'options'",
                                field.name
                            ));
                        }
                        Some(opts) if opts.is_empty() => {
                            errors.push(format!(
                                "module '{name}', field '{}': static_select 'options' must not be empty",
                                field.name
                            ));
                        }
                        _ => {}
                    }
                }

                // dynamic_select must have source
                if field.field_type == FieldType::DynamicSelect && field.source.is_none() {
                    errors.push(format!(
                        "module '{name}', field '{}': dynamic_select requires 'source'",
                        field.name
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::ValidationError(errors))
        }
    }
}
