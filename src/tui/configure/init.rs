//! Free-function init helpers for the configure subsystem.
//!
//! These were moved here from `App` (Slice 14) to co-locate initialization
//! logic with the configure screen it initializes.  `App` keeps thin wrappers
//! for backward-compat with test callsites.

use crate::app::{
    ConfigSetting, ConfigureLevel, ConfigureState, SettingKind, callout_quick_select,
};
use crate::config::{Config, FieldConfig, FieldType, SubFieldConfig, SubFieldType};

/// Build settings list for editing a specific field's properties.
///
/// Replaces the current `settings` in `ConfigureState` with settings
/// derived from the given `FieldConfig`. Type-conditional settings
/// (options, source) are included based on the field's current type.
pub fn build_field_settings(field: &FieldConfig) -> Vec<ConfigSetting> {
    let type_str = match field.field_type {
        FieldType::Text => "text",
        FieldType::Textarea => "textarea",
        FieldType::Number => "number",
        FieldType::StaticSelect => "static_select",
        FieldType::DynamicSelect => "dynamic_select",
        FieldType::CompositeArray => "composite_array",
        FieldType::Toggle => "toggle",
        FieldType::Counter => "counter",
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
                "toggle".to_string(),
                "counter".to_string(),
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

    if field.field_type == FieldType::CompositeArray {
        let sub_count = field.sub_fields.as_ref().map(|s| s.len()).unwrap_or(0);
        settings.push(ConfigSetting {
            label: "Sub-fields".to_string(),
            key: "sub_fields".to_string(),
            value: format!(
                "{sub_count} column{}",
                if sub_count == 1 { "" } else { "s" }
            ),
            kind: SettingKind::NavLink,
        });
    }

    // Callout wrapping — only for textarea fields targeting body
    if field.field_type == FieldType::Textarea {
        settings.push(ConfigSetting {
            label: "Callout".to_string(),
            key: "callout".to_string(),
            value: field.callout.clone().unwrap_or_default(),
            kind: SettingKind::QuickSelect(callout_quick_select()),
        });
    }

    settings.push(ConfigSetting {
        label: "Icon".to_string(),
        key: "icon".to_string(),
        value: field.icon.clone().unwrap_or_default(),
        kind: SettingKind::Text,
    });

    settings.push(ConfigSetting {
        label: "Preset Exclude".to_string(),
        key: "preset_exclude".to_string(),
        value: if field.preset_exclude.unwrap_or(false) {
            "true".to_string()
        } else {
            "false".to_string()
        },
        kind: SettingKind::Toggle(vec!["false".to_string(), "true".to_string()]),
    });

    settings
}

/// Build settings list for editing a specific sub-field's properties.
pub fn build_sub_field_settings(sub_field: &SubFieldConfig) -> Vec<ConfigSetting> {
    let type_str = match sub_field.field_type {
        SubFieldType::Text => "text",
        SubFieldType::Number => "number",
        SubFieldType::StaticSelect => "static_select",
    };

    let mut settings = vec![
        ConfigSetting {
            label: "Name".to_string(),
            key: "name".to_string(),
            value: sub_field.name.clone(),
            kind: SettingKind::Text,
        },
        ConfigSetting {
            label: "Prompt".to_string(),
            key: "prompt".to_string(),
            value: sub_field.prompt.clone(),
            kind: SettingKind::Text,
        },
        ConfigSetting {
            label: "Type".to_string(),
            key: "field_type".to_string(),
            value: type_str.to_string(),
            kind: SettingKind::Toggle(vec![
                "text".to_string(),
                "number".to_string(),
                "static_select".to_string(),
            ]),
        },
    ];

    if sub_field.field_type == SubFieldType::StaticSelect {
        let opts_display = sub_field
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

    settings
}

/// Build `ConfigureState` for vault-level configuration.
///
/// Returns a `ConfigureState` ready to be used with `ConfigureLevel::VaultSettings`.
/// The `module_key` is set to `"__vault__"` (not a real module).
pub fn init_vault_configure(config: &Config) -> ConfigureState {
    let vault = &config.vault;

    // Always show the persisted value, never the env-var override.
    // secrets.toml is authoritative; config.toml is the legacy fallback.
    let api_key_from_file = Config::read_secret_api_key()
        .or_else(|| {
            std::fs::read_to_string(Config::default_config_path())
                .ok()
                .and_then(|content| {
                    let doc = content.parse::<toml_edit::DocumentMut>().ok()?;
                    doc.get("vault")?.get("api_key")?.as_str().map(String::from)
                })
        })
        .unwrap_or_default();

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
            value: vault.api_port.map(|p| p.to_string()).unwrap_or_default(),
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
            value: vault
                .date_format
                .clone()
                .unwrap_or_else(|| "%Y%m%d".to_string()),
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
        help_overlay_open: false,
        quick_select_open: false,
    }
}

/// Initialize configure state for creating a new module.
///
/// The returned `ConfigureState` has an empty `module_key` — it will be set
/// by the user via the "Module Key" setting.
pub fn init_new_module_configure(_config: &Config) -> ConfigureState {
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
            kind: SettingKind::Toggle(vec![
                "append".to_string(),
                "create".to_string(),
                "update".to_string(),
            ]),
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
        help_overlay_open: false,
        quick_select_open: false,
    }
}
