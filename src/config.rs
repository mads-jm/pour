// LINTOK: oversized: pending Slice 3 + Slice 8 decomposition (config types/validation split)

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

/// Top-level configuration, deserialized from `config.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub vault: VaultConfig,
    pub modules: HashMap<String, ModuleConfig>,
    #[serde(default)]
    pub templates: Option<HashMap<String, TemplateConfig>>,
    /// Optional ordering for dashboard display. Modules not listed appear
    /// alphabetically after the listed ones.
    pub module_order: Option<Vec<String>>,
    /// Semver string identifying the config schema version. Absent in configs
    /// predating versioning; treated as `"0.1.0"` for backward compatibility.
    #[serde(default = "default_config_version")]
    pub config_version: Option<String>,
}

/// Vault connection settings.
// The vault name is not persisted here; it is implicit in the section name
// and is resolved at runtime from the config path.
#[derive(Debug, Clone, Deserialize)]
pub struct VaultConfig {
    pub base_path: String,
    #[serde(default = "default_api_port")]
    pub api_port: Option<u16>,
    pub api_key: Option<String>,
    /// strftime format string used to render a date-based filename when the
    /// browser picks a directory for an append-mode path. Defaults to `%Y%m%d`.
    pub date_format: Option<String>,
}

fn default_api_port() -> Option<u16> {
    Some(27124)
}

fn default_config_version() -> Option<String> {
    Some("0.1.0".to_string())
}

/// A single module definition (e.g. `[modules.coffee]`).
#[derive(Debug, Clone, Deserialize)]
pub struct ModuleConfig {
    pub mode: WriteMode,
    pub path: String,
    pub append_under_header: Option<String>,
    pub append_template: Option<String>,
    pub fields: Vec<FieldConfig>,
    pub display_name: Option<String>,
    /// Obsidian callout type used for `{{callout}}` in templates.
    pub callout_type: Option<String>,
    /// Optional icon displayed in the TUI dashboard and written to frontmatter
    /// in create-mode output. Typically a Unicode emoji (e.g. "☕").
    pub icon: Option<String>,
    /// When `true`, create-mode output includes a `daily` frontmatter key
    /// linking to today's daily note (e.g. `daily: "[[20260405]]"`).
    #[serde(default)]
    pub daily_link: Option<bool>,
    /// When `true`, append-mode insertion treats any subsequent heading as a
    /// section boundary, not just equal-or-higher level. Useful when the
    /// target heading should not absorb sub-headings (e.g. `### Tasks` with
    /// `#### Completed` sub-headings).
    #[serde(default)]
    pub append_shallow: Option<bool>,
    /// When `false`, this module is hidden from the mobile PWA (`/api/v1/config`
    /// omits it entirely). Defaults to `true` — all modules are mobile-visible
    /// unless explicitly opted out.
    #[serde(default)]
    pub mobile_visible: Option<bool>,
    /// Ordered list of field names used as drilldown axes in the preset picker.
    /// Empty/absent → no picker; the legacy Left/Right cycler stays active.
    #[serde(default)]
    pub preset_axes: Vec<String>,
}

impl ModuleConfig {
    /// Returns whether this module should be included in the mobile PWA config response.
    ///
    /// `None` and `Some(true)` both mean visible; only `Some(false)` hides it.
    pub fn is_mobile_visible(&self) -> bool {
        self.mobile_visible.unwrap_or(true)
    }
}

/// Whether a module appends to an existing note or creates a new one.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    Append,
    Create,
}

/// Conditional visibility rule for a field.
///
/// Exactly one of `equals` or `one_of` must be set.
#[derive(Debug, Clone, Deserialize)]
pub struct ShowWhen {
    /// The name of the field whose value controls visibility.
    pub field: String,
    /// Show this field when the controlling field's value equals this string.
    pub equals: Option<String>,
    /// Show this field when the controlling field's value is any of these strings.
    pub one_of: Option<Vec<String>>,
}

/// A single field within a module form.
#[derive(Debug, Clone, Deserialize)]
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
    /// Column definitions for `composite_array` fields.
    pub sub_fields: Option<Vec<SubFieldConfig>>,
    /// Obsidian callout type to wrap this field's body output in (e.g. "note", "tip").
    pub callout: Option<String>,
    /// Title rendered on the callout line: `> [!type] {callout_title}`. Only
    /// used when `callout` is set; if empty or absent, no title is emitted.
    #[serde(default)]
    pub callout_title: Option<String>,
    /// When `true`, allows the user to create new entries inline during selection.
    /// Only valid on `dynamic_select` fields.
    #[serde(default)]
    pub allow_create: Option<bool>,
    /// When `true`, wraps the output value in Obsidian wikilink syntax: `[[value]]`.
    /// Applies to `text`, `static_select`, and `dynamic_select` field types.
    /// No-ops if the value is already wrapped. Defaults to `false`.
    #[serde(default)]
    pub wikilink: Option<bool>,
    /// Template name (from `[templates]`) used to create a new note when `allow_create` fires.
    #[serde(default)]
    pub create_template: Option<String>,
    /// Obsidian command URI to execute after inline note creation (e.g. `"templater:run"` ).
    #[serde(default)]
    pub post_create_command: Option<String>,
    /// Optional conditional visibility rule. When set, this field is only shown
    /// if the referenced field's current value matches the condition.
    #[serde(default)]
    pub show_when: Option<ShowWhen>,
    /// Optional icon displayed next to the field prompt in the TUI form.
    /// Purely cosmetic — not written to output.
    #[serde(default)]
    pub icon: Option<String>,
    /// When `true`, this field is excluded from preset capture and application.
    /// Useful for notes/textarea fields that should not be part of a saved preset.
    #[serde(default)]
    pub preset_exclude: Option<bool>,
    /// When `true`, comma-separated values (e.g. `"a, b, c"`) are split and
    /// emitted as a YAML sequence in frontmatter, and each item gets its own
    /// wikilink when `wikilink = true`. Defaults to `false` (value treated as a
    /// literal string — not split).
    #[serde(default)]
    pub list: bool,
}

/// The kind of input widget for a field.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Textarea,
    Number,
    StaticSelect,
    DynamicSelect,
    CompositeArray,
}

/// Allowed sub-field types within a `composite_array` field.
///
/// Restricted to simple input types — no nesting or dynamic data.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubFieldType {
    Text,
    Number,
    StaticSelect,
}

/// A single column definition within a `composite_array` field.
#[derive(Debug, Clone, Deserialize)]
pub struct SubFieldConfig {
    pub name: String,
    pub field_type: SubFieldType,
    pub prompt: String,
    /// Valid only for `static_select` sub-fields.
    pub options: Option<Vec<String>>,
}

/// Allowed field types within a template definition. Restricted to simple input types.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TemplateFieldType {
    Text,
    Number,
    StaticSelect,
}

/// A single field definition within a template.
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateFieldConfig {
    pub name: String,
    pub field_type: TemplateFieldType,
    pub prompt: String,
    /// Valid only for `static_select`.
    pub options: Option<Vec<String>>,
    pub default: Option<String>,
    /// For `static_select` only: allow typing a novel value which is appended
    /// to the field's `options` on disk.
    pub allow_create: Option<bool>,
}

/// A template definition for auto-created notes.
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateConfig {
    pub path: String,
    pub fields: Vec<TemplateFieldConfig>,
}

/// Controls whether a field value goes into YAML frontmatter or the Markdown body.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldTarget {
    Frontmatter,
    Body,
}

/// Partial updates to apply to an existing module in the config file.
///
/// Each field is an `Option`; `None` means "leave unchanged".
/// For optional string fields (`display_name`, `append_under_header`),
/// `Some(None)` means "remove the key from the config".
pub struct ModuleUpdates {
    /// New vault-relative path for the module's output file.
    pub path: Option<String>,
    /// New display name. `Some(None)` removes the key.
    pub display_name: Option<Option<String>>,
    /// New write mode (`append` or `create`).
    pub mode: Option<WriteMode>,
    /// New append-under-header value. `Some(None)` removes the key.
    pub append_under_header: Option<Option<String>>,
    /// New callout type. `Some(None)` removes the key.
    pub callout_type: Option<Option<String>>,
    /// New icon. `Some(None)` removes the key.
    pub icon: Option<Option<String>>,
    /// New daily_link value. `Some(None)` removes the key.
    pub daily_link: Option<Option<bool>>,
    /// New append_shallow value. `Some(None)` removes the key.
    pub append_shallow: Option<Option<bool>>,
    /// New mobile_visible value. `Some(None)` removes the key (treated as true).
    pub mobile_visible: Option<Option<bool>>,
}

/// Partial updates to apply to the vault section of the config file.
///
/// Each field is an `Option`; `None` means "leave unchanged".
/// For optional fields (`api_port`, `api_key`, `date_format`), `Some(None)` means "remove the key".
pub struct VaultUpdates {
    pub base_path: Option<String>,
    /// `Some(None)` removes the key; `Some(Some(port))` sets it.
    pub api_port: Option<Option<u16>>,
    /// `Some(None)` removes the key; `Some(Some(key))` sets it.
    pub api_key: Option<Option<String>>,
    /// `Some(None)` removes the key; `Some(Some(fmt))` sets it.
    pub date_format: Option<Option<String>>,
}

/// Partial updates to apply to a single field in a module's config.
///
/// Each field is an `Option`; `None` means "leave unchanged".
/// For optional keys (`required`, `default`, `options`, `source`, `target`),
/// `Some(None)` means "remove the key from the config".
pub struct FieldUpdates {
    pub name: Option<String>,
    pub field_type: Option<FieldType>,
    pub prompt: Option<String>,
    pub required: Option<Option<bool>>,
    pub default: Option<Option<String>>,
    pub options: Option<Option<Vec<String>>>,
    pub source: Option<Option<String>>,
    pub target: Option<Option<FieldTarget>>,
    /// Obsidian callout type. `Some(None)` removes the key.
    pub callout: Option<Option<String>>,
    /// Title rendered after the callout type. `Some(None)` removes the key.
    pub callout_title: Option<Option<String>>,
    /// Conditional visibility rule. `Some(None)` removes the key.
    pub show_when: Option<Option<ShowWhen>>,
    /// Wrap output in wikilink syntax. `Some(None)` removes the key.
    pub wikilink: Option<Option<bool>>,
    /// Allow inline note creation. `Some(None)` removes the key.
    pub allow_create: Option<Option<bool>>,
    /// Template name for inline note creation. `Some(None)` removes the key.
    pub create_template: Option<Option<String>>,
    /// Obsidian command URI after inline creation. `Some(None)` removes the key.
    pub post_create_command: Option<Option<String>>,
    /// Icon displayed next to prompt in TUI. `Some(None)` removes the key.
    pub icon: Option<Option<String>>,
    /// Exclude from preset capture/apply. `Some(None)` removes the key.
    pub preset_exclude: Option<Option<bool>>,
}

/// Partial updates to apply to a single sub-field within a composite_array field.
///
/// Each field is an `Option`; `None` means "leave unchanged".
/// For `options`, `Some(None)` removes the key.
pub struct SubFieldUpdates {
    pub name: Option<String>,
    pub field_type: Option<SubFieldType>,
    pub prompt: Option<String>,
    pub options: Option<Option<Vec<String>>>,
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
    /// The named module key was not found in the config.
    ModuleNotFound(String),
    /// A module with the given key already exists in the config.
    DuplicateModule(String),
    /// Failed to write the updated config back to disk.
    WriteError(std::io::Error),
    /// The config document could not be parsed by the structure-preserving editor.
    EditParseError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NotFound(path) => {
                write!(
                    f,
                    "config file not found: {}\n      run 'pour init' to create one",
                    path.display()
                )
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
            ConfigError::ModuleNotFound(key) => {
                write!(f, "module '{key}' not found in config")
            }
            ConfigError::DuplicateModule(key) => {
                write!(f, "module '{key}' already exists in config")
            }
            ConfigError::WriteError(err) => write!(f, "failed to write config: {err}"),
            ConfigError::EditParseError(msg) => {
                write!(f, "failed to parse config for editing: {msg}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Build a `toml_edit::InlineTable` from a `ShowWhen` value.
///
/// Produces `{ field = "x", equals = "y" }` or `{ field = "x", one_of = ["a", "b"] }`.
fn build_show_when_inline_table(sw: &ShowWhen) -> toml_edit::InlineTable {
    let mut t = toml_edit::InlineTable::new();
    t.insert("field", toml_edit::Value::from(sw.field.as_str()));
    if let Some(ref eq) = sw.equals {
        t.insert("equals", toml_edit::Value::from(eq.as_str()));
    }
    if let Some(ref one_of) = sw.one_of {
        let mut arr = toml_edit::Array::new();
        for v in one_of {
            arr.push(v.as_str());
        }
        t.insert("one_of", toml_edit::Value::Array(arr));
    }
    t
}

impl Config {
    /// The config schema version this build of Pour understands.
    pub const CURRENT_CONFIG_VERSION: &'static str = "1.0.0";

    /// Atomically write `content` to `path`.
    ///
    /// Writes to a sibling `.toml.tmp` file first, then renames it over `path`.
    /// If the rename fails the orphan temp file is removed before returning the
    /// error — the single sanctioned write path for all config files so that
    /// orphan cleanup is universal rather than per-callsite.
    pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), ConfigError> {
        let tmp_path = path.with_extension("toml.tmp");
        std::fs::write(&tmp_path, content).map_err(ConfigError::WriteError)?;
        if let Err(e) = crate::util::atomic_replace(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(ConfigError::WriteError(e));
        }
        Ok(())
    }

    /// Load and validate the configuration.
    ///
    /// Resolution order for config file path:
    /// 1. `POUR_CONFIG` environment variable (if set)
    /// 2. `~/.pour/config.toml` (or `$POUR_HOME/config.toml` if overridden)
    ///
    /// The `api_key` is resolved via `from_toml` in this order:
    /// env var > secrets.toml > config.toml.
    ///
    /// After a successful parse, a best-effort migration runs: if `api_key`
    /// exists in `config.toml` but not yet in `secrets.toml`, it is moved.
    /// All migration errors are silently ignored.
    pub fn load() -> Result<Config, ConfigError> {
        let path = Self::resolve_config_path()?;

        let content = std::fs::read_to_string(&path).map_err(ConfigError::ReadError)?;

        let mut config = Self::from_toml(&content)?;

        // Apply secrets.toml and env-var overlays.
        Self::apply_api_key_overlay(&mut config);

        // Best-effort migration: move api_key from config.toml to secrets.toml.
        Self::migrate_api_key_to_secrets(&path);

        Ok(config)
    }

    /// Move `api_key` from `config.toml` into `secrets.toml` if it is present
    /// in `config.toml` and absent from `secrets.toml`.
    ///
    /// All errors are swallowed — this is a non-critical background migration.
    /// Re-reads config.toml from disk before writing to avoid clobbering
    /// concurrent modifications.
    fn migrate_api_key_to_secrets(config_path: &Path) {
        // Only migrate if secrets.toml doesn't already have the key.
        if Self::read_secret_api_key().is_some() {
            return;
        }

        // Re-read config.toml from disk to get fresh content for the write-back.
        let fresh_content = match std::fs::read_to_string(config_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut doc = match fresh_content.parse::<DocumentMut>() {
            Ok(d) => d,
            Err(_) => return,
        };

        let key_value = doc
            .get("vault")
            .and_then(|v| v.get("api_key"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let Some(key) = key_value else {
            return;
        };

        // Write to secrets.toml.
        if Self::write_secret_api_key(Some(&key)).is_err() {
            return;
        }

        // Remove from config.toml using the same parsed doc (no second parse needed).
        if let Some(vault) = doc.get_mut("vault").and_then(|v| v.as_table_mut()) {
            vault.remove("api_key");
        }
        let new_content = doc.to_string();
        // Best-effort: errors are intentionally swallowed — this is a
        // non-critical background migration.
        let _ = Self::write_atomic(config_path, &new_content);
    }

    /// Parse and validate a config from a TOML string.
    ///
    /// This is a pure parse+validate step. The caller is responsible for
    /// applying secrets.toml and env-var overlays if needed (see `load()`).
    pub fn from_toml(toml_content: &str) -> Result<Config, ConfigError> {
        let config: Config = toml::from_str(toml_content).map_err(ConfigError::ParseError)?;
        config.validate()?;
        Ok(config)
    }

    /// Apply the `api_key` resolution chain to an already-parsed config:
    ///
    /// 1. `config.toml [vault] api_key` (already present via serde)
    /// 2. `~/.pour/secrets.toml`          (overrides config.toml)
    /// 3. `POUR_API_KEY` env var          (overrides everything)
    fn apply_api_key_overlay(config: &mut Config) {
        // Layer 2: secrets.toml overrides config.toml value.
        if let Some(secret_key) = Self::read_secret_api_key() {
            config.vault.api_key = Some(secret_key);
        }

        // Layer 1: env var overrides everything.
        if let Ok(env_key) = std::env::var("POUR_API_KEY")
            && !env_key.is_empty()
        {
            config.vault.api_key = Some(env_key);
        }
    }

    /// Return the expected config file path without checking if it exists.
    /// Respects `POUR_CONFIG` env var, otherwise resolves under `POUR_HOME`
    /// (defaulting to `~/.pour/config.toml`).
    pub fn default_config_path() -> PathBuf {
        crate::paths::config_path()
    }

    /// Return the path to `secrets.toml`, a sibling of `config.toml`.
    ///
    /// When `POUR_CONFIG` is set the secrets file sits in the same directory
    /// as the overridden config, which provides automatic test isolation.
    pub fn secrets_path() -> PathBuf {
        crate::paths::secrets_path()
    }

    /// Read `api_key` from `secrets.toml`.
    ///
    /// Returns `None` if the file is absent or the key is not present.
    /// All I/O and parse errors are silently swallowed — this is a best-effort
    /// read with env-var and config.toml as fallbacks.
    pub fn read_secret_api_key() -> Option<String> {
        let content = std::fs::read_to_string(Self::secrets_path()).ok()?;
        let doc = content.parse::<DocumentMut>().ok()?;
        let value = doc.get("api_key")?.as_str()?;
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    /// Write or remove `api_key` in `secrets.toml`.
    ///
    /// `Some(key)` sets the value; `None` removes the key (leaving an empty
    /// document so the file persists without dangling keys).
    ///
    /// Creates parent directories and the file itself if they don't exist.
    pub fn write_secret_api_key(key: Option<&str>) -> Result<(), ConfigError> {
        let path = Self::secrets_path();

        // Create parent dir if needed.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::WriteError)?;
        }

        // Load existing doc or start fresh.
        let mut doc: DocumentMut = std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| c.parse::<DocumentMut>().ok())
            .unwrap_or_default();

        match key {
            Some(k) => {
                doc["api_key"] = toml_edit::value(k);
            }
            None => {
                doc.remove("api_key");
            }
        }

        Self::write_atomic(&path, &doc.to_string())?;

        // Restrict secrets.toml to owner-read/write only (0600) on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }

        Ok(())
    }

    /// Read `mobile_token` from `secrets.toml`.
    ///
    /// Returns `None` if the file is absent or the key is not present.
    /// The `POUR_MOBILE_TOKEN` env var overrides this value (mirrors the
    /// `POUR_API_KEY` precedence story).
    pub fn read_mobile_token() -> Option<String> {
        // Env var takes precedence.
        if let Ok(env_tok) = std::env::var("POUR_MOBILE_TOKEN")
            && !env_tok.is_empty()
        {
            return Some(env_tok);
        }

        let content = std::fs::read_to_string(Self::secrets_path()).ok()?;
        let doc = content.parse::<DocumentMut>().ok()?;
        let value = doc.get("mobile_token")?.as_str()?;
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    /// Write `mobile_token` to `secrets.toml`.
    ///
    /// Creates parent directories and the file itself if they don't exist.
    /// Preserves any existing keys in the file (e.g. `api_key`).
    pub fn write_mobile_token(token: &str) -> Result<(), ConfigError> {
        let path = Self::secrets_path();

        // Create parent dir if needed.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::WriteError)?;
        }

        // Load existing doc or start fresh.
        let mut doc: DocumentMut = std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| c.parse::<DocumentMut>().ok())
            .unwrap_or_default();

        doc["mobile_token"] = toml_edit::value(token);

        Self::write_atomic(&path, &doc.to_string())?;

        // Restrict secrets.toml to owner-read/write only (0600) on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }

        Ok(())
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

        let path = crate::paths::config_path();
        if path.exists() {
            Ok(path)
        } else {
            Err(ConfigError::NotFound(path))
        }
    }

    /// Update a module's scalar fields in-place on disk, preserving comments and formatting.
    ///
    /// Uses `toml_edit` so that comments, whitespace, and unrelated keys are
    /// left untouched. After writing, the result is re-parsed via `from_toml`
    /// to validate the new state. If validation fails, the original file
    /// content is restored before returning the error.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` if `module_key` does not exist.
    /// Returns `ConfigError::ValidationError` if the resulting config is invalid
    /// (the original file is restored in this case).
    pub fn update_module_on_disk(
        module_key: &str,
        updates: &ModuleUpdates,
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            // Navigate to doc["modules"][module_key] — error if absent.
            let module = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?;

            if let Some(ref path_val) = updates.path {
                module["path"] = toml_edit::value(path_val.as_str());
            }

            if let Some(ref display_name_update) = updates.display_name {
                match display_name_update {
                    Some(v) => {
                        module["display_name"] = toml_edit::value(v.as_str());
                    }
                    None => {
                        module.remove("display_name");
                    }
                }
            }

            if let Some(ref mode_update) = updates.mode {
                let mode_str = match mode_update {
                    WriteMode::Append => "append",
                    WriteMode::Create => "create",
                };
                module["mode"] = toml_edit::value(mode_str);
            }

            if let Some(ref header_update) = updates.append_under_header {
                match header_update {
                    Some(v) => {
                        module["append_under_header"] = toml_edit::value(v.as_str());
                    }
                    None => {
                        module.remove("append_under_header");
                    }
                }
            }

            if let Some(ref callout_update) = updates.callout_type {
                match callout_update {
                    Some(v) => {
                        module["callout_type"] = toml_edit::value(v.as_str());
                    }
                    None => {
                        module.remove("callout_type");
                    }
                }
            }

            if let Some(ref icon_update) = updates.icon {
                match icon_update {
                    Some(v) => {
                        module["icon"] = toml_edit::value(v.as_str());
                    }
                    None => {
                        module.remove("icon");
                    }
                }
            }

            if let Some(ref daily_link_update) = updates.daily_link {
                match daily_link_update {
                    Some(v) => {
                        module["daily_link"] = toml_edit::value(*v);
                    }
                    None => {
                        module.remove("daily_link");
                    }
                }
            }

            if let Some(ref shallow_update) = updates.append_shallow {
                match shallow_update {
                    Some(v) => {
                        module["append_shallow"] = toml_edit::value(*v);
                    }
                    None => {
                        module.remove("append_shallow");
                    }
                }
            }

            if let Some(ref mobile_visible_update) = updates.mobile_visible {
                match mobile_visible_update {
                    Some(v) => {
                        module["mobile_visible"] = toml_edit::value(*v);
                    }
                    None => {
                        module.remove("mobile_visible");
                    }
                }
            }

            // Validate before writing — never touch the file if the result is invalid.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Apply partial updates to a single field in a module's config file.
    ///
    /// Navigates to `doc["modules"][module_key]["fields"][field_index]` using
    /// `toml_edit` so comments and formatting are preserved. Validates the
    /// result before writing. Uses atomic write (temp file + rename).
    pub fn update_field_on_disk(
        module_key: &str,
        field_index: usize,
        updates: &FieldUpdates,
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            // Navigate to the fields array-of-tables for this module.
            let field = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?
                .get_mut("fields")
                .and_then(|f| f.as_array_of_tables_mut())
                .and_then(|arr| arr.get_mut(field_index))
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field index {field_index} out of range for module '{module_key}'"
                    )])
                })?;

            // Apply each update.
            if let Some(ref name) = updates.name {
                field["name"] = toml_edit::value(name.as_str());
            }

            if let Some(ref ft) = updates.field_type {
                let type_str = match ft {
                    FieldType::Text => "text",
                    FieldType::Textarea => "textarea",
                    FieldType::Number => "number",
                    FieldType::StaticSelect => "static_select",
                    FieldType::DynamicSelect => "dynamic_select",
                    FieldType::CompositeArray => "composite_array",
                };
                field["field_type"] = toml_edit::value(type_str);
            }

            if let Some(ref prompt) = updates.prompt {
                field["prompt"] = toml_edit::value(prompt.as_str());
            }

            if let Some(ref required_update) = updates.required {
                match required_update {
                    Some(v) => field["required"] = toml_edit::value(*v),
                    None => {
                        field.remove("required");
                    }
                }
            }

            if let Some(ref default_update) = updates.default {
                match default_update {
                    Some(v) => field["default"] = toml_edit::value(v.as_str()),
                    None => {
                        field.remove("default");
                    }
                }
            }

            if let Some(ref options_update) = updates.options {
                match options_update {
                    Some(opts) => {
                        let mut arr = toml_edit::Array::new();
                        for opt in opts {
                            arr.push(opt.as_str());
                        }
                        field["options"] = toml_edit::value(arr);
                    }
                    None => {
                        field.remove("options");
                    }
                }
            }

            if let Some(ref source_update) = updates.source {
                match source_update {
                    Some(v) => field["source"] = toml_edit::value(v.as_str()),
                    None => {
                        field.remove("source");
                    }
                }
            }

            if let Some(ref target_update) = updates.target {
                match target_update {
                    Some(t) => {
                        let target_str = match t {
                            FieldTarget::Frontmatter => "frontmatter",
                            FieldTarget::Body => "body",
                        };
                        field["target"] = toml_edit::value(target_str);
                    }
                    None => {
                        field.remove("target");
                    }
                }
            }

            if let Some(ref callout_update) = updates.callout {
                match callout_update {
                    Some(v) => field["callout"] = toml_edit::value(v.as_str()),
                    None => {
                        field.remove("callout");
                    }
                }
            }

            if let Some(ref show_when_update) = updates.show_when {
                match show_when_update {
                    Some(sw) => {
                        field["show_when"] = toml_edit::Item::Value(toml_edit::Value::InlineTable(
                            build_show_when_inline_table(sw),
                        ));
                    }
                    None => {
                        field.remove("show_when");
                    }
                }
            }

            if let Some(ref wikilink_update) = updates.wikilink {
                match wikilink_update {
                    Some(v) => field["wikilink"] = toml_edit::value(*v),
                    None => {
                        field.remove("wikilink");
                    }
                }
            }

            if let Some(ref allow_create_update) = updates.allow_create {
                match allow_create_update {
                    Some(v) => field["allow_create"] = toml_edit::value(*v),
                    None => {
                        field.remove("allow_create");
                    }
                }
            }

            if let Some(ref create_template_update) = updates.create_template {
                match create_template_update {
                    Some(v) => field["create_template"] = toml_edit::value(v.as_str()),
                    None => {
                        field.remove("create_template");
                    }
                }
            }

            if let Some(ref post_create_command_update) = updates.post_create_command {
                match post_create_command_update {
                    Some(v) => field["post_create_command"] = toml_edit::value(v.as_str()),
                    None => {
                        field.remove("post_create_command");
                    }
                }
            }

            if let Some(ref icon_update) = updates.icon {
                match icon_update {
                    Some(v) => field["icon"] = toml_edit::value(v.as_str()),
                    None => {
                        field.remove("icon");
                    }
                }
            }

            if let Some(ref preset_exclude_update) = updates.preset_exclude {
                match preset_exclude_update {
                    Some(v) => field["preset_exclude"] = toml_edit::value(*v),
                    None => {
                        field.remove("preset_exclude");
                    }
                }
            }

            // Validate before writing.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        }) // end Self::edit
    }

    /// Append a new option string to a static_select module field's `options`
    /// array on disk. No-op if the option is already present (case-sensitive
    /// match). Preserves comments and formatting via `toml_edit`. Atomic write.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` / range error if navigation fails,
    /// `ConfigError::ValidationError` if the result is not a valid config.
    pub fn append_option_to_field_on_disk(
        module_key: &str,
        field_index: usize,
        new_option: &str,
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            let field = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?
                .get_mut("fields")
                .and_then(|f| f.as_array_of_tables_mut())
                .and_then(|arr| arr.get_mut(field_index))
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field index {field_index} out of range for module '{module_key}'"
                    )])
                })?;

            // Read existing options (if any), skip if already present, then append.
            let already_present = field
                .get("options")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str())
                        .any(|existing| existing == new_option)
                })
                .unwrap_or(false);

            if already_present {
                return Ok(());
            }

            match field.get_mut("options").and_then(|v| v.as_array_mut()) {
                Some(arr) => {
                    arr.push(new_option);
                }
                None => {
                    let mut arr = toml_edit::Array::new();
                    arr.push(new_option);
                    field["options"] = toml_edit::value(arr);
                }
            }

            // Validate before writing.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Append a new option string to a template static_select field's
    /// `options` array on disk. No-op if the option is already present
    /// (case-sensitive match). Preserves comments and formatting via
    /// `toml_edit`. Atomic write.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ValidationError` if the template or field cannot
    /// be located, or if the resulting config fails validation.
    pub fn append_option_to_template_field_on_disk(
        template_name: &str,
        field_index: usize,
        new_option: &str,
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            let field = draft
                .doc
                .get_mut("templates")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(template_name))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "template '{template_name}' not found"
                    )])
                })?
                .get_mut("fields")
                .and_then(|f| f.as_array_of_tables_mut())
                .and_then(|arr| arr.get_mut(field_index))
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field index {field_index} out of range for template '{template_name}'"
                    )])
                })?;

            let already_present = field
                .get("options")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str())
                        .any(|existing| existing == new_option)
                })
                .unwrap_or(false);

            if already_present {
                return Ok(());
            }

            match field.get_mut("options").and_then(|v| v.as_array_mut()) {
                Some(arr) => {
                    arr.push(new_option);
                }
                None => {
                    let mut arr = toml_edit::Array::new();
                    arr.push(new_option);
                    field["options"] = toml_edit::value(arr);
                }
            }

            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Append a new field to a module's fields array on disk.
    ///
    /// Uses `toml_edit` to preserve comments and formatting. Validates the
    /// result before writing. Uses atomic write (temp file + rename).
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` if `module_key` does not exist.
    /// Returns `ConfigError::ValidationError` if the resulting config is invalid
    /// (e.g. `static_select` without `options`).
    pub fn add_field_on_disk(module_key: &str, field: &FieldConfig) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            // Verify the module exists before touching anything.
            draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?;

            // Build the new table entry.
            let mut new_table = toml_edit::Table::new();

            let type_str = match field.field_type {
                FieldType::Text => "text",
                FieldType::Textarea => "textarea",
                FieldType::Number => "number",
                FieldType::StaticSelect => "static_select",
                FieldType::DynamicSelect => "dynamic_select",
                FieldType::CompositeArray => "composite_array",
            };

            new_table["name"] = toml_edit::value(field.name.as_str());
            new_table["field_type"] = toml_edit::value(type_str);
            new_table["prompt"] = toml_edit::value(field.prompt.as_str());

            if let Some(required) = field.required {
                new_table["required"] = toml_edit::value(required);
            }

            if let Some(ref default) = field.default {
                new_table["default"] = toml_edit::value(default.as_str());
            }

            if let Some(ref opts) = field.options {
                let mut arr = toml_edit::Array::new();
                for opt in opts {
                    arr.push(opt.as_str());
                }
                new_table["options"] = toml_edit::value(arr);
            }

            if let Some(ref source) = field.source {
                new_table["source"] = toml_edit::value(source.as_str());
            }

            if let Some(ref target) = field.target {
                let target_str = match target {
                    FieldTarget::Frontmatter => "frontmatter",
                    FieldTarget::Body => "body",
                };
                new_table["target"] = toml_edit::value(target_str);
            }

            if let Some(ref subs) = field.sub_fields {
                let mut arr = toml_edit::ArrayOfTables::new();
                for sf in subs {
                    let mut t = toml_edit::Table::new();
                    let sf_type_str = match sf.field_type {
                        SubFieldType::Text => "text",
                        SubFieldType::Number => "number",
                        SubFieldType::StaticSelect => "static_select",
                    };
                    t["name"] = toml_edit::value(sf.name.as_str());
                    t["field_type"] = toml_edit::value(sf_type_str);
                    t["prompt"] = toml_edit::value(sf.prompt.as_str());
                    if let Some(ref opts) = sf.options {
                        let mut a = toml_edit::Array::new();
                        for opt in opts {
                            a.push(opt.as_str());
                        }
                        t["options"] = toml_edit::value(a);
                    }
                    arr.push(t);
                }
                new_table["sub_fields"] = toml_edit::Item::ArrayOfTables(arr);
            }

            if let Some(ref callout) = field.callout {
                new_table["callout"] = toml_edit::value(callout.as_str());
            }

            if let Some(wikilink) = field.wikilink {
                new_table["wikilink"] = toml_edit::value(wikilink);
            }

            if let Some(allow_create) = field.allow_create {
                new_table["allow_create"] = toml_edit::value(allow_create);
            }

            if let Some(ref create_template) = field.create_template {
                new_table["create_template"] = toml_edit::value(create_template.as_str());
            }

            if let Some(ref post_create_command) = field.post_create_command {
                new_table["post_create_command"] = toml_edit::value(post_create_command.as_str());
            }

            if let Some(ref sw) = field.show_when {
                new_table["show_when"] = toml_edit::Item::Value(toml_edit::Value::InlineTable(
                    build_show_when_inline_table(sw),
                ));
            }

            if let Some(ref icon) = field.icon {
                new_table["icon"] = toml_edit::value(icon.as_str());
            }

            if let Some(preset_exclude) = field.preset_exclude {
                new_table["preset_exclude"] = toml_edit::value(preset_exclude);
            }

            // Navigate to the fields array-of-tables and push the new entry.
            // If the key doesn't exist yet, create it.
            let module = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?;

            if !module.contains_array_of_tables("fields") {
                module["fields"] = toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
            }

            module["fields"]
                .as_array_of_tables_mut()
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![
                        "fields key is not an array of tables — this is a bug in the config editor"
                            .to_string(),
                    ])
                })?
                .push(new_table);

            // Validate before writing.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        }) // end Self::edit
    }

    /// Remove a field at `field_index` from a module's fields array on disk.
    ///
    /// Validates the result before writing — this catches the "module must have
    /// at least one field" rule when removing the last field. Uses atomic write.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` if `module_key` does not exist.
    /// Returns `ConfigError::ValidationError` if `field_index` is out of range
    /// or if removing the field would leave the module with zero fields.
    pub fn remove_field_on_disk(module_key: &str, field_index: usize) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            // Navigate to the fields array-of-tables.
            let fields = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?
                .get_mut("fields")
                .and_then(|f| f.as_array_of_tables_mut())
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "module '{module_key}': no fields array found"
                    )])
                })?;

            if field_index >= fields.len() {
                return Err(ConfigError::ValidationError(vec![format!(
                    "field index {field_index} out of range for module '{module_key}'"
                )]));
            }

            if fields.len() == 1 {
                return Err(ConfigError::ValidationError(vec![format!(
                    "module '{module_key}': must have at least one field"
                )]));
            }

            fields.remove(field_index);

            // Validate — catches the zero-fields case.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Update vault-level settings in-place on disk, preserving comments and formatting.
    ///
    /// Uses `toml_edit` to navigate to `doc["vault"]`. Validates the result before
    /// writing. Uses atomic write (temp file + rename).
    pub fn update_vault_on_disk(updates: &VaultUpdates) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            let vault = draft
                .doc
                .get_mut("vault")
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| {
                    ConfigError::EditParseError("missing [vault] table in config".to_string())
                })?;

            if let Some(ref base_path) = updates.base_path {
                vault["base_path"] = toml_edit::value(base_path.as_str());
            }

            if let Some(ref port_update) = updates.api_port {
                match port_update {
                    Some(port) => {
                        vault["api_port"] = toml_edit::value(*port as i64);
                    }
                    None => {
                        vault.remove("api_port");
                    }
                }
            }

            if let Some(ref key_update) = updates.api_key {
                // Write api_key to secrets.toml instead of config.toml.
                Self::write_secret_api_key(key_update.as_deref())?;
                // Also remove any legacy api_key from config.toml on save.
                vault.remove("api_key");
            }

            if let Some(ref fmt_update) = updates.date_format {
                match fmt_update {
                    Some(fmt) => {
                        vault["date_format"] = toml_edit::value(fmt.as_str());
                    }
                    None => {
                        vault.remove("date_format");
                    }
                }
            }

            // Validate before writing.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Add a new module to the config file on disk.
    ///
    /// Uses `toml_edit` to preserve comments and formatting. Validates the result
    /// before writing. Uses atomic write (temp file + rename).
    ///
    /// If `doc` contains a top-level `module_order` array, `module_key` is appended
    /// to it automatically.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::DuplicateModule` if `module_key` already exists.
    /// Returns `ConfigError::ValidationError` if the resulting config is invalid.
    pub fn add_module_on_disk(module_key: &str, module: &ModuleConfig) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            // Reject duplicate module key.
            let already_exists = draft
                .doc
                .get("modules")
                .and_then(|m| m.as_table())
                .map(|t| t.contains_key(module_key))
                .unwrap_or(false);

            if already_exists {
                return Err(ConfigError::DuplicateModule(module_key.to_string()));
            }

            // Build the module table.
            let mut module_table = toml_edit::Table::new();

            let mode_str = match module.mode {
                WriteMode::Append => "append",
                WriteMode::Create => "create",
            };
            module_table["mode"] = toml_edit::value(mode_str);
            module_table["path"] = toml_edit::value(module.path.as_str());

            if let Some(ref display_name) = module.display_name {
                module_table["display_name"] = toml_edit::value(display_name.as_str());
            }

            if let Some(ref header) = module.append_under_header {
                module_table["append_under_header"] = toml_edit::value(header.as_str());
            }

            if let Some(ref tmpl) = module.append_template {
                module_table["append_template"] = toml_edit::value(tmpl.as_str());
            }

            if let Some(ref callout) = module.callout_type {
                module_table["callout_type"] = toml_edit::value(callout.as_str());
            }

            if let Some(ref icon) = module.icon {
                module_table["icon"] = toml_edit::value(icon.as_str());
            }

            if module.daily_link == Some(true) {
                module_table["daily_link"] = toml_edit::value(true);
            }

            if module.append_shallow == Some(true) {
                module_table["append_shallow"] = toml_edit::value(true);
            }

            // Build fields as an ArrayOfTables.
            let mut fields_aot = toml_edit::ArrayOfTables::new();

            for field in &module.fields {
                let mut ft = toml_edit::Table::new();

                let type_str = match field.field_type {
                    FieldType::Text => "text",
                    FieldType::Textarea => "textarea",
                    FieldType::Number => "number",
                    FieldType::StaticSelect => "static_select",
                    FieldType::DynamicSelect => "dynamic_select",
                    FieldType::CompositeArray => "composite_array",
                };

                ft["name"] = toml_edit::value(field.name.as_str());
                ft["field_type"] = toml_edit::value(type_str);
                ft["prompt"] = toml_edit::value(field.prompt.as_str());

                if let Some(required) = field.required {
                    ft["required"] = toml_edit::value(required);
                }

                if let Some(ref default) = field.default {
                    ft["default"] = toml_edit::value(default.as_str());
                }

                if let Some(ref opts) = field.options {
                    let mut arr = toml_edit::Array::new();
                    for opt in opts {
                        arr.push(opt.as_str());
                    }
                    ft["options"] = toml_edit::value(arr);
                }

                if let Some(ref source) = field.source {
                    ft["source"] = toml_edit::value(source.as_str());
                }

                if let Some(ref target) = field.target {
                    let target_str = match target {
                        FieldTarget::Frontmatter => "frontmatter",
                        FieldTarget::Body => "body",
                    };
                    ft["target"] = toml_edit::value(target_str);
                }

                if let Some(ref callout) = field.callout {
                    ft["callout"] = toml_edit::value(callout.as_str());
                }

                if let Some(ref icon) = field.icon {
                    ft["icon"] = toml_edit::value(icon.as_str());
                }

                if let Some(ref subs) = field.sub_fields {
                    let mut sub_arr = toml_edit::ArrayOfTables::new();
                    for sf in subs {
                        let mut t = toml_edit::Table::new();
                        let sf_type_str = match sf.field_type {
                            SubFieldType::Text => "text",
                            SubFieldType::Number => "number",
                            SubFieldType::StaticSelect => "static_select",
                        };
                        t["name"] = toml_edit::value(sf.name.as_str());
                        t["field_type"] = toml_edit::value(sf_type_str);
                        t["prompt"] = toml_edit::value(sf.prompt.as_str());
                        if let Some(ref opts) = sf.options {
                            let mut a = toml_edit::Array::new();
                            for opt in opts {
                                a.push(opt.as_str());
                            }
                            t["options"] = toml_edit::value(a);
                        }
                        sub_arr.push(t);
                    }
                    ft["sub_fields"] = toml_edit::Item::ArrayOfTables(sub_arr);
                }

                fields_aot.push(ft);
            }

            module_table["fields"] = toml_edit::Item::ArrayOfTables(fields_aot);

            // Insert into doc["modules"][module_key].
            draft.doc["modules"][module_key] = toml_edit::Item::Table(module_table);

            // If a top-level module_order array exists, append the new key to it.
            if let Some(order_item) = draft.doc.get_mut("module_order")
                && let Some(arr) = order_item.as_array_mut()
            {
                arr.push(module_key);
            }

            // Validate before writing.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Overwrite the top-level `module_order` array on disk.
    ///
    /// Uses `toml_edit` to preserve comments and formatting elsewhere in the file.
    /// Validates the result before writing. Uses atomic write (temp file + rename).
    pub fn update_module_order_on_disk(order: &[String]) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            let mut arr = toml_edit::Array::new();
            for key in order {
                arr.push(key.as_str());
            }

            draft.doc["module_order"] = toml_edit::value(arr);

            // Validate before writing.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Non-blocking path validation against the actual filesystem.
    ///
    /// Unlike `validate()`, which enforces structural rules and blocks loading,
    /// this method checks whether the paths referenced in the config actually
    /// exist on disk. It returns human-readable warnings (not errors) so the
    /// caller can surface them without preventing the app from starting.
    ///
    /// Checks performed:
    /// - For `create` mode modules: the parent directory of `module.path` must exist.
    /// - For `append` mode modules: the file at `module.path` must exist.
    /// - For `dynamic_select` fields with a `source`: the source directory must exist.
    ///
    /// Paths containing `{{` (template variables) are skipped entirely — they
    /// cannot be resolved at config-load time.
    pub fn check_paths(&self, vault_base: &Path) -> Vec<String> {
        let mut warnings = Vec::new();

        for (module_key, module) in &self.modules {
            // Skip paths containing template variables or strftime specifiers —
            // unresolvable at config time.
            if !module.path.contains("{{") && !module.path.contains('%') {
                let full_path = vault_base.join(&module.path);

                match module.mode {
                    WriteMode::Create => {
                        // For create mode, the parent directory must exist.
                        let parent = full_path.parent().unwrap_or(vault_base);
                        if !parent.exists() {
                            warnings.push(format!(
                                "module '{}': path '{}' — parent directory not found",
                                module_key, module.path
                            ));
                        }
                    }
                    WriteMode::Append => {
                        // For append mode, the target file must exist.
                        if !full_path.exists() {
                            warnings.push(format!(
                                "module '{}': path '{}' — file not found",
                                module_key, module.path
                            ));
                        }
                    }
                }
            }

            // Check dynamic_select source directories.
            for field in &module.fields {
                if field.field_type == FieldType::DynamicSelect
                    && let Some(ref source) = field.source
                    && !source.contains("{{")
                {
                    let source_path = vault_base.join(source);
                    if !source_path.exists() {
                        warnings.push(format!(
                            "module '{}', field '{}': source '{}' — directory not found",
                            module_key, field.name, source
                        ));
                    }
                }
            }
        }

        warnings
    }

    /// Delete a module from the config file on disk.
    ///
    /// Uses `toml_edit` to preserve comments and formatting for remaining modules.
    /// Validates the result before writing. Uses atomic write (temp file + rename).
    ///
    /// If `module_order` exists, the deleted module key is removed from it.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` if `module_key` does not exist.
    /// Returns `ConfigError::ValidationError` if this is the last module.
    pub fn delete_module_on_disk(module_key: &str) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            // Navigate to the modules table and verify the key exists.
            let modules = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .ok_or_else(|| {
                    ConfigError::EditParseError("missing [modules] table in config".to_string())
                })?;

            if !modules.contains_key(module_key) {
                return Err(ConfigError::ModuleNotFound(module_key.to_string()));
            }

            // Guard: cannot delete the last module.
            let module_count = modules.iter().count();
            if module_count <= 1 {
                return Err(ConfigError::ValidationError(vec![
                    "cannot delete last module".to_string(),
                ]));
            }

            modules.remove(module_key);

            // Remove from module_order if present.
            if let Some(order_item) = draft.doc.get_mut("module_order")
                && let Some(arr) = order_item.as_array_mut()
            {
                // Find and remove the matching entry by value.
                let pos = arr.iter().position(|v| v.as_str() == Some(module_key));
                if let Some(idx) = pos {
                    arr.remove(idx);
                }
            }

            // Validate before writing.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Reorder the fields of a module on disk using a permutation index slice.
    ///
    /// `new_order` must be a permutation of `0..fields.len()`: same length,
    /// each index appearing exactly once. The fields are rewritten in the
    /// order specified by `new_order[0], new_order[1], ...`.
    ///
    /// Uses `toml_edit` to preserve comments and formatting. Validates the
    /// result before writing. Uses atomic write (temp file + rename).
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` if `module_key` does not exist.
    /// Returns `ConfigError::ValidationError` if `new_order` is not a valid
    /// permutation of the fields indices.
    pub fn reorder_fields_on_disk(
        module_key: &str,
        new_order: &[usize],
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            // Navigate to the module table.
            let module = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?;

            let fields_aot = module
                .get_mut("fields")
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "module '{module_key}': no fields array found"
                    )])
                })?
                .as_array_of_tables_mut()
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "module '{module_key}': fields is not an array of tables"
                    )])
                })?;

            let field_count = fields_aot.len();

            // Validate permutation: correct length.
            if new_order.len() != field_count {
                return Err(ConfigError::ValidationError(vec![format!(
                    "module '{module_key}': new_order length {} does not match field count {}",
                    new_order.len(),
                    field_count
                )]));
            }

            // Validate permutation: each index in range and no duplicates.
            let mut seen = vec![false; field_count];
            for &idx in new_order {
                if idx >= field_count {
                    return Err(ConfigError::ValidationError(vec![format!(
                        "module '{module_key}': index {idx} out of range (field count {field_count})"
                    )]));
                }
                if seen[idx] {
                    return Err(ConfigError::ValidationError(vec![format!(
                        "module '{module_key}': duplicate index {idx} in new_order"
                    )]));
                }
                seen[idx] = true;
            }

            // toml_edit serializes tables in position order. Swapping positions causes
            // the formatter to emit them in the new order while preserving all content.
            //
            // Collect the original doc positions for each field table. Position `i` in
            // new_order means "the table that was at old index new_order[i] should now
            // appear at slot i". So old table new_order[i] gets positions[i].
            let original_positions: Vec<Option<usize>> =
                fields_aot.iter().map(|t| t.position()).collect();

            // Build assignments: (old_index -> new_position).
            // new_order[i] = old_idx  =>  old_idx gets original_positions[i].
            let mut assignments: Vec<(usize, Option<usize>)> = new_order
                .iter()
                .enumerate()
                .map(|(slot, &old_idx)| (old_idx, original_positions[slot]))
                .collect();
            // Sort by old_index so we can look up by index directly.
            assignments.sort_by_key(|&(old_i, _)| old_i);

            for (old_idx, new_pos) in assignments {
                let t = fields_aot.get_mut(old_idx).ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field index {old_idx} out of range during reorder"
                    )])
                })?;
                let pos = new_pos.ok_or_else(|| {
                    ConfigError::EditParseError(
                        "field table has no document position; cannot reorder".to_string(),
                    )
                })?;
                t.set_position(pos);
            }

            // Validate before writing.
            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;

            Ok(())
        })
    }

    /// Add a new sub-field to a composite_array field on disk.
    ///
    /// Navigates to `doc["modules"][module_key]["fields"][field_index]["sub_fields"]`
    /// and appends the new sub-field table. Creates the `sub_fields` ArrayOfTables
    /// if it doesn't exist yet. Uses atomic write.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` if `module_key` does not exist.
    /// Returns `ConfigError::ValidationError` if `field_index` is out of range.
    pub fn add_sub_field_on_disk(
        module_key: &str,
        field_index: usize,
        sub_field: &SubFieldConfig,
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            // Verify field index is in range before building the new table.
            let field_count = draft
                .doc
                .get("modules")
                .and_then(|m| m.as_table())
                .and_then(|t| t.get(module_key))
                .and_then(|v| v.as_table())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?
                .get("fields")
                .and_then(|f| f.as_array_of_tables())
                .map(|arr| arr.len())
                .unwrap_or(0);

            if field_index >= field_count {
                return Err(ConfigError::ValidationError(vec![format!(
                    "field index {field_index} out of range for module '{module_key}'"
                )]));
            }

            let sf_type_str = match sub_field.field_type {
                SubFieldType::Text => "text",
                SubFieldType::Number => "number",
                SubFieldType::StaticSelect => "static_select",
            };

            let mut new_t = toml_edit::Table::new();
            new_t["name"] = toml_edit::value(sub_field.name.as_str());
            new_t["field_type"] = toml_edit::value(sf_type_str);
            new_t["prompt"] = toml_edit::value(sub_field.prompt.as_str());
            if let Some(ref opts) = sub_field.options {
                let mut a = toml_edit::Array::new();
                for opt in opts {
                    a.push(opt.as_str());
                }
                new_t["options"] = toml_edit::value(a);
            }

            // Navigate to the field and push the sub-field.
            let field = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?
                .get_mut("fields")
                .and_then(|f| f.as_array_of_tables_mut())
                .and_then(|arr| arr.get_mut(field_index))
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field index {field_index} out of range for module '{module_key}'"
                    )])
                })?;

            if !field.contains_array_of_tables("sub_fields") {
                field["sub_fields"] =
                    toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
            }

            field["sub_fields"]
                .as_array_of_tables_mut()
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![
                        "sub_fields key is not an array of tables — this is a bug in the config editor"
                            .to_string(),
                    ])
                })?
                .push(new_t);

            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;
            Ok(())
        })
    }

    /// Remove a sub-field at `sub_field_index` from a composite_array field on disk.
    ///
    /// Uses atomic write. Validates the result before writing.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` if `module_key` does not exist.
    /// Returns `ConfigError::ValidationError` if indices are out of range or if
    /// removing the sub-field would leave the field with zero sub-fields.
    pub fn remove_sub_field_on_disk(
        module_key: &str,
        field_index: usize,
        sub_field_index: usize,
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            let field = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?
                .get_mut("fields")
                .and_then(|f| f.as_array_of_tables_mut())
                .and_then(|arr| arr.get_mut(field_index))
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field index {field_index} out of range for module '{module_key}'"
                    )])
                })?;

            let subs = field
                .get_mut("sub_fields")
                .and_then(|sf| sf.as_array_of_tables_mut())
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field {field_index} in module '{module_key}': no sub_fields array found"
                    )])
                })?;

            if sub_field_index >= subs.len() {
                return Err(ConfigError::ValidationError(vec![format!(
                    "sub_field index {sub_field_index} out of range"
                )]));
            }

            if subs.len() == 1 {
                return Err(ConfigError::ValidationError(vec![
                    "composite_array field must have at least one sub-field".to_string(),
                ]));
            }

            subs.remove(sub_field_index);

            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;
            Ok(())
        })
    }

    /// Swap two sub-fields within a composite_array field on disk.
    ///
    /// Uses the same position-swap technique as `reorder_fields_on_disk`.
    /// Uses atomic write.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ValidationError` if either index is out of range.
    pub fn swap_sub_fields_on_disk(
        module_key: &str,
        field_index: usize,
        a: usize,
        b: usize,
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            let field = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?
                .get_mut("fields")
                .and_then(|f| f.as_array_of_tables_mut())
                .and_then(|arr| arr.get_mut(field_index))
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field index {field_index} out of range for module '{module_key}'"
                    )])
                })?;

            let subs = field
                .get_mut("sub_fields")
                .and_then(|sf| sf.as_array_of_tables_mut())
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field {field_index} in module '{module_key}': no sub_fields array found"
                    )])
                })?;

            let sub_count = subs.len();

            if a >= sub_count || b >= sub_count {
                return Err(ConfigError::ValidationError(vec![format!(
                    "sub_field indices {a} or {b} out of range (count {sub_count})"
                )]));
            }

            // Swap positions using the same technique as reorder_fields_on_disk.
            let pos_a = subs.get(a).and_then(|t| t.position()).ok_or_else(|| {
                ConfigError::EditParseError("sub_field has no position".to_string())
            })?;
            let pos_b = subs.get(b).and_then(|t| t.position()).ok_or_else(|| {
                ConfigError::EditParseError("sub_field has no position".to_string())
            })?;

            subs.get_mut(a)
                .ok_or_else(|| {
                    ConfigError::EditParseError("sub_field index a invalid".to_string())
                })?
                .set_position(pos_b);
            subs.get_mut(b)
                .ok_or_else(|| {
                    ConfigError::EditParseError("sub_field index b invalid".to_string())
                })?
                .set_position(pos_a);

            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;
            Ok(())
        })
    }

    /// Apply partial updates to a single sub-field within a composite_array field.
    ///
    /// Navigates to `doc["modules"][module_key]["fields"][field_index]["sub_fields"][sub_field_index]`.
    /// Uses atomic write.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::ModuleNotFound` if `module_key` does not exist.
    /// Returns `ConfigError::ValidationError` if indices are out of range.
    pub fn update_sub_field_on_disk(
        module_key: &str,
        field_index: usize,
        sub_field_index: usize,
        updates: &SubFieldUpdates,
    ) -> Result<(), ConfigError> {
        let path = Self::resolve_config_path()?;
        Self::edit(&path, |draft| {
            let field = draft
                .doc
                .get_mut("modules")
                .and_then(|m| m.as_table_mut())
                .and_then(|t| t.get_mut(module_key))
                .and_then(|v| v.as_table_mut())
                .ok_or_else(|| ConfigError::ModuleNotFound(module_key.to_string()))?
                .get_mut("fields")
                .and_then(|f| f.as_array_of_tables_mut())
                .and_then(|arr| arr.get_mut(field_index))
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "field index {field_index} out of range for module '{module_key}'"
                    )])
                })?;

            let sub = field
                .get_mut("sub_fields")
                .and_then(|sf| sf.as_array_of_tables_mut())
                .and_then(|arr| arr.get_mut(sub_field_index))
                .ok_or_else(|| {
                    ConfigError::ValidationError(vec![format!(
                        "sub_field index {sub_field_index} out of range"
                    )])
                })?;

            if let Some(ref name) = updates.name {
                sub["name"] = toml_edit::value(name.as_str());
            }

            if let Some(ref ft) = updates.field_type {
                let type_str = match ft {
                    SubFieldType::Text => "text",
                    SubFieldType::Number => "number",
                    SubFieldType::StaticSelect => "static_select",
                };
                sub["field_type"] = toml_edit::value(type_str);
            }

            if let Some(ref prompt) = updates.prompt {
                sub["prompt"] = toml_edit::value(prompt.as_str());
            }

            if let Some(ref opts_update) = updates.options {
                match opts_update {
                    Some(opts) => {
                        let mut a = toml_edit::Array::new();
                        for opt in opts {
                            a.push(opt.as_str());
                        }
                        sub["options"] = toml_edit::value(a);
                    }
                    None => {
                        sub.remove("options");
                    }
                }
            }

            let new_content = draft.doc.to_string();
            Self::from_toml(&new_content)?;
            Ok(())
        })
    }

    /// Validate cross-field `show_when` references within a single module.
    ///
    /// Checks:
    /// 1. Unknown field reference
    /// 2. Self-reference
    /// 3. Reference to a `composite_array` field
    /// 4. Circular dependency (A→B→…→A)
    fn validate_show_when_refs(
        module_name: &str,
        fields: &[FieldConfig],
        errors: &mut Vec<String>,
    ) {
        // Build name → FieldConfig index map
        let field_map: HashMap<&str, &FieldConfig> =
            fields.iter().map(|f| (f.name.as_str(), f)).collect();

        // Per-field reference checks (unknown, self, composite_array)
        for field in fields {
            let sw = match &field.show_when {
                Some(sw) => sw,
                None => continue,
            };

            let ref_name = sw.field.as_str();

            // Self-reference
            if ref_name == field.name.as_str() {
                errors.push(format!(
                    "Field '{}' in module '{}' has show_when referencing itself",
                    field.name, module_name
                ));
                continue;
            }

            // Unknown field reference
            let referenced = match field_map.get(ref_name) {
                Some(f) => f,
                None => {
                    errors.push(format!(
                        "Field '{}' in module '{}' has show_when referencing unknown field '{}'",
                        field.name, module_name, ref_name
                    ));
                    continue;
                }
            };

            // Reference to composite_array
            if referenced.field_type == FieldType::CompositeArray {
                errors.push(format!(
                    "show_when on field '{}' in module '{}' cannot reference composite_array field '{}'",
                    field.name, module_name, ref_name
                ));
            }
        }

        // Cycle detection via DFS
        // Build adjacency: field name → name it depends on (if any)
        // Only include edges where the referenced field actually exists (unknown refs already reported above)
        let deps: HashMap<&str, &str> = fields
            .iter()
            .filter_map(|f| {
                f.show_when.as_ref().and_then(|sw| {
                    let ref_name = sw.field.as_str();
                    // Only track edge if the reference is valid (exists and not self)
                    if field_map.contains_key(ref_name) && ref_name != f.name.as_str() {
                        Some((f.name.as_str(), ref_name))
                    } else {
                        None
                    }
                })
            })
            .collect();

        // DFS cycle detection — track globally visited and current path stack separately.
        // `visited` = nodes fully processed (no need to re-walk).
        // `path` = nodes on the current walk's stack (used for cycle extraction).
        let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut reported_cycles: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for start in deps.keys().copied() {
            if visited.contains(start) {
                continue;
            }

            // Walk the chain from `start`
            let mut path: Vec<&str> = Vec::new();
            let mut in_path: std::collections::HashSet<&str> = std::collections::HashSet::new();
            let mut current = start;

            loop {
                if in_path.contains(current) {
                    // Found a cycle — extract the cycle portion
                    // `current` was just found in `in_path`, which is kept in
                    // sync with `path`, so it is always present.
                    // SAFETY: `in_path.contains(current)` is true at this branch
                    // entry, and `path` and `in_path` are kept in strict sync
                    // (every `path.push(x)` is immediately followed by
                    // `in_path.insert(x)`), so `current` is guaranteed to be
                    // present in `path`.
                    let cycle_start = path
                        .iter()
                        .position(|&n| n == current)
                        .expect("current node must be in path when a cycle is detected");
                    let cycle: Vec<&str> = path[cycle_start..].to_vec();

                    // Canonical key: sorted so we don't report the same cycle twice
                    let mut key_parts = cycle.to_vec();
                    key_parts.sort_unstable();
                    let cycle_key = key_parts.join(",");

                    if reported_cycles.insert(cycle_key) {
                        let cycle_list = cycle.join(" → ");
                        errors.push(format!(
                            "Circular show_when dependency detected in module '{}': {}",
                            module_name, cycle_list
                        ));
                    }
                    break;
                }

                // Already processed from a prior walk — no need to re-traverse.
                if visited.contains(current) {
                    break;
                }

                path.push(current);
                in_path.insert(current);

                match deps.get(current) {
                    Some(&next) => current = next,
                    None => break,
                }
            }

            // Mark all nodes on this walk's path as fully explored
            for node in &path {
                visited.insert(node);
            }
        }
    }

    /// Check that a path is vault-relative and safe.
    ///
    /// Rejects absolute paths (Unix or Windows), drive-qualified paths,
    /// UNC paths, and paths containing `..` traversal components.
    fn validate_vault_relative_path(path: &str, label: &str, errors: &mut Vec<String>) {
        let trimmed = path.trim();

        if trimmed.is_empty() {
            errors.push(format!("{label}: path must not be empty"));
            return;
        }

        // Reject Unix absolute paths
        if trimmed.starts_with('/') {
            errors.push(format!(
                "{label}: path must be vault-relative, not absolute"
            ));
            return;
        }

        // Reject Windows drive-qualified paths (e.g. C:, C:\, D:/)
        if trimmed.len() >= 2
            && trimmed.as_bytes()[0].is_ascii_alphabetic()
            && trimmed.as_bytes()[1] == b':'
        {
            errors.push(format!(
                "{label}: path must be vault-relative, not drive-qualified"
            ));
            return;
        }

        // Reject UNC paths (\\server\share or //server/share)
        if trimmed.starts_with("\\\\") || trimmed.starts_with("//") {
            errors.push(format!(
                "{label}: path must be vault-relative, not a UNC path"
            ));
            return;
        }

        // Reject path traversal via '..' in any component
        for component in trimmed.replace('\\', "/").split('/') {
            if component == ".." {
                errors.push(format!("{label}: path must not contain '..' traversal"));
                return;
            }
        }
    }

    /// Validate the parsed config against business rules.
    /// Validate `config_version`: must be a parseable `major.minor.patch` semver string
    /// with a major version this build of Pour supports.
    fn validate_config_version(version: &str, errors: &mut Vec<String>) {
        if version.is_empty() {
            errors.push("config_version must not be an empty string".to_string());
            return;
        }

        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            errors.push(format!(
                "config_version '{version}' is not a valid semver string (expected major.minor.patch)"
            ));
            return;
        }

        // Reject leading zeros in any segment (e.g. "00.01.00")
        for part in &parts {
            if part.len() > 1 && part.starts_with('0') {
                errors.push(format!("config_version segment '{part}' has leading zeros"));
                return;
            }
        }

        let major = match parts[0].parse::<u32>() {
            Ok(n) => n,
            Err(_) => {
                errors.push(format!(
                    "config_version '{version}' is not a valid semver string (expected major.minor.patch)"
                ));
                return;
            }
        };
        // Validate minor and patch are numeric too.
        for segment in &parts[1..] {
            if segment.parse::<u32>().is_err() {
                errors.push(format!(
                    "config_version '{version}' is not a valid semver string (expected major.minor.patch)"
                ));
                return;
            }
        }

        // Parse the current major version from CURRENT_CONFIG_VERSION.
        let current_major = Self::CURRENT_CONFIG_VERSION
            .split('.')
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        if major > current_major {
            errors.push(format!(
                "Config version {version} is not supported by this version of Pour. \
                Please update Pour or downgrade your config."
            ));
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let mut errors = Vec::new();

        // Validate config_version (always Some — serde default guarantees it).
        Self::validate_config_version(
            self.config_version.as_deref().unwrap_or("0.1.0"),
            &mut errors,
        );

        for (name, module) in &self.modules {
            // Every module must have at least one field
            if module.fields.is_empty() {
                errors.push(format!("module '{name}': must have at least one field"));
            }

            // Validate module write path is vault-relative
            Self::validate_vault_relative_path(
                &module.path,
                &format!("module '{name}', path"),
                &mut errors,
            );

            // Append mode requires append_under_header
            if module.mode == WriteMode::Append && module.append_under_header.is_none() {
                errors.push(format!(
                    "module '{name}': append mode requires 'append_under_header'"
                ));
            }

            // Check for duplicate field names within the module
            let mut seen_fields = std::collections::HashSet::new();
            for field in &module.fields {
                if !seen_fields.insert(&field.name) {
                    errors.push(format!(
                        "module '{name}': duplicate field name '{}'",
                        field.name
                    ));
                }
            }

            for field in &module.fields {
                // `list = true` only makes sense for scalar field types where
                // comma-splitting produces meaningful frontmatter lists.
                if field.list
                    && !matches!(
                        field.field_type,
                        FieldType::Text | FieldType::StaticSelect | FieldType::DynamicSelect
                    )
                {
                    errors.push(format!(
                        "module '{name}', field '{}': 'list = true' is only valid on text, static_select, or dynamic_select",
                        field.name
                    ));
                }

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

                // composite_array must have non-empty sub_fields with unique names
                if field.field_type == FieldType::CompositeArray {
                    match &field.sub_fields {
                        None => {
                            errors.push(format!(
                                "module '{name}', field '{}': composite_array requires 'sub_fields'",
                                field.name
                            ));
                        }
                        Some(subs) if subs.is_empty() => {
                            errors.push(format!(
                                "module '{name}', field '{}': composite_array 'sub_fields' must not be empty",
                                field.name
                            ));
                        }
                        Some(subs) => {
                            // Check for duplicate sub-field names
                            let mut seen = std::collections::HashSet::new();
                            for sub in subs {
                                if !seen.insert(&sub.name) {
                                    errors.push(format!(
                                        "module '{name}', field '{}': duplicate sub_field name '{}'",
                                        field.name, sub.name
                                    ));
                                }

                                // static_select sub-fields must have options
                                if sub.field_type == SubFieldType::StaticSelect {
                                    match &sub.options {
                                        None => {
                                            errors.push(format!(
                                                "module '{name}', field '{}', sub_field '{}': static_select requires 'options'",
                                                field.name, sub.name
                                            ));
                                        }
                                        Some(opts) if opts.is_empty() => {
                                            errors.push(format!(
                                                "module '{name}', field '{}', sub_field '{}': static_select 'options' must not be empty",
                                                field.name, sub.name
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }

                // dynamic_select must have source
                if field.field_type == FieldType::DynamicSelect {
                    if field.source.is_none() {
                        errors.push(format!(
                            "module '{name}', field '{}': dynamic_select requires 'source'",
                            field.name
                        ));
                    }

                    // source path must be vault-relative and safe
                    if let Some(source) = &field.source {
                        Self::validate_vault_relative_path(
                            source,
                            &format!("module '{name}', field '{}', source", field.name),
                            &mut errors,
                        );
                    }
                }

                // allow_create is only valid on dynamic_select or static_select
                if field.allow_create.is_some()
                    && field.field_type != FieldType::DynamicSelect
                    && field.field_type != FieldType::StaticSelect
                {
                    errors.push(format!(
                        "module '{name}', field '{}': allow_create is only valid on dynamic_select or static_select fields",
                        field.name
                    ));
                }

                // create_template validation
                if let Some(ref tpl_name) = field.create_template {
                    // Rule 1: only valid on dynamic_select
                    if field.field_type != FieldType::DynamicSelect {
                        errors.push(format!(
                            "module '{name}', field '{}': create_template is only valid on dynamic_select fields",
                            field.name
                        ));
                    }
                    // Rule 2: requires allow_create = true
                    if field.allow_create != Some(true) {
                        errors.push(format!(
                            "module '{name}', field '{}': create_template requires allow_create = true",
                            field.name
                        ));
                    }
                    // Rule 3: referenced template must exist
                    let template_exists = self
                        .templates
                        .as_ref()
                        .and_then(|t| t.get(tpl_name.as_str()))
                        .is_some();
                    if !template_exists {
                        errors.push(format!(
                            "module '{name}', field '{}': create_template references unknown template '{tpl_name}'",
                            field.name
                        ));
                    }
                }

                // post_create_command requires create_template
                if field.post_create_command.is_some() && field.create_template.is_none() {
                    errors.push(format!(
                        "module '{name}', field '{}': post_create_command requires create_template to be set",
                        field.name
                    ));
                }

                // show_when: exactly one of `equals` or `one_of` must be set
                if let Some(ref sw) = field.show_when {
                    match (&sw.equals, &sw.one_of) {
                        (Some(_), Some(_)) => {
                            errors.push(format!(
                                "show_when on field '{}': specify either 'equals' or 'one_of', not both",
                                field.name
                            ));
                        }
                        (None, None) => {
                            errors.push(format!(
                                "show_when on field '{}': must specify 'equals' or 'one_of'",
                                field.name
                            ));
                        }
                        _ => {}
                    }
                    // Reject empty string for `equals`
                    if let Some(ref eq_val) = sw.equals
                        && eq_val.is_empty()
                    {
                        errors.push(format!(
                            "show_when on field '{}': 'equals' must not be empty",
                            field.name
                        ));
                    }
                    // Reject empty vec for `one_of`
                    if let Some(ref one_of_val) = sw.one_of
                        && one_of_val.is_empty()
                    {
                        errors.push(format!(
                            "show_when on field '{}': 'one_of' must not be empty",
                            field.name
                        ));
                    }
                }
            }

            // Cross-field show_when reference validation (per module)
            Self::validate_show_when_refs(name, &module.fields, &mut errors);
        }

        // Validate templates
        if let Some(ref templates) = self.templates {
            for (name, template) in templates {
                // Template path must contain {{name}}
                if !template.path.contains("{{name}}") {
                    errors.push(format!(
                        "template '{name}': path must contain the {{{{name}}}} placeholder"
                    ));
                }

                // Template path must be vault-relative
                Self::validate_vault_relative_path(
                    &template.path,
                    &format!("template '{name}', path"),
                    &mut errors,
                );

                // Template must have at least one field
                if template.fields.is_empty() {
                    errors.push(format!("template '{name}': must have at least one field"));
                }

                // Check for duplicate and reserved field names
                const RESERVED_TEMPLATE_FIELDS: &[&str] = &["date", "name"];
                let mut seen = std::collections::HashSet::new();
                for field in &template.fields {
                    if RESERVED_TEMPLATE_FIELDS.contains(&field.name.as_str()) {
                        errors.push(format!(
                            "template '{name}', field '{}': '{}' is reserved (auto-generated in frontmatter)",
                            field.name, field.name
                        ));
                    }
                    if !seen.insert(&field.name) {
                        errors.push(format!(
                            "template '{name}': duplicate field name '{}'",
                            field.name
                        ));
                    }

                    // static_select template fields must have non-empty options
                    if field.field_type == TemplateFieldType::StaticSelect {
                        match &field.options {
                            None => {
                                errors.push(format!(
                                    "template '{name}', field '{}': static_select requires 'options'",
                                    field.name
                                ));
                            }
                            Some(opts) if opts.is_empty() => {
                                errors.push(format!(
                                    "template '{name}', field '{}': static_select 'options' must not be empty",
                                    field.name
                                ));
                            }
                            _ => {}
                        }
                    }

                    // allow_create is only valid on static_select template fields
                    if field.allow_create.is_some()
                        && field.field_type != TemplateFieldType::StaticSelect
                    {
                        errors.push(format!(
                            "template '{name}', field '{}': allow_create is only valid on static_select template fields",
                            field.name
                        ));
                    }
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
