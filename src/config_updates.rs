//! Canonical implementations for building `*Updates` structs from
//! [`ConfigSetting`] slices.
//!
//! Both `src/main.rs` (save-on-submit) and `src/tui/configure.rs`
//! (auto-save-on-navigate) needed the same conversion logic.  This module is
//! the single source of truth; both call sites import from here.

use crate::app::ConfigSetting;
use crate::config::{
    FieldTarget, FieldType, FieldUpdates, ModuleUpdates, SubFieldType, SubFieldUpdates,
    VaultUpdates, WriteMode,
};

/// Build [`ModuleUpdates`] from a slice of configure settings.
pub fn build_module_updates(settings: &[ConfigSetting]) -> ModuleUpdates {
    let mut path: Option<String> = None;
    let mut display_name: Option<Option<String>> = None;
    let mut mode: Option<WriteMode> = None;
    let mut append_under_header: Option<Option<String>> = None;
    let mut callout_type: Option<Option<String>> = None;
    let mut icon: Option<Option<String>> = None;
    let mut daily_link: Option<Option<bool>> = None;
    let mut append_shallow: Option<Option<bool>> = None;
    let mut mobile_visible: Option<Option<bool>> = None;

    for setting in settings {
        match setting.key.as_str() {
            "path" => path = Some(setting.value.clone()),
            "display_name" => {
                display_name = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "mode" => {
                mode = Some(match setting.value.as_str() {
                    "append" => WriteMode::Append,
                    "update" => WriteMode::Update,
                    _ => WriteMode::Create,
                });
            }
            "append_under_header" => {
                append_under_header = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "callout_type" => {
                callout_type = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "icon" => {
                icon = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "daily_link" => {
                daily_link = Some(if setting.value == "true" {
                    Some(true)
                } else {
                    None
                });
            }
            "append_shallow" => {
                append_shallow = Some(if setting.value == "true" {
                    Some(true)
                } else {
                    None
                });
            }
            // `mobile_visible` uses "false" as the sentinel: storing `Some(false)`
            // means the user explicitly turned it off; anything else means inherit.
            "mobile_visible" => {
                mobile_visible = Some(if setting.value == "false" {
                    Some(false)
                } else {
                    None
                });
            }
            _ => {}
        }
    }

    ModuleUpdates {
        path,
        display_name,
        mode,
        append_under_header,
        callout_type,
        icon,
        daily_link,
        append_shallow,
        mobile_visible,
    }
}

/// Pre-validate vault settings before saving.
///
/// Returns `Err(message)` on the first validation failure so that callers can
/// surface the message before attempting a disk write.  Must be called before
/// [`build_vault_updates`].
pub fn validate_vault_settings(settings: &[ConfigSetting]) -> Result<(), String> {
    for setting in settings {
        match setting.key.as_str() {
            "base_path" if setting.value.trim().is_empty() => {
                return Err("Base Path must not be empty".to_string());
            }
            "api_port" => {
                let trimmed = setting.value.trim();
                if !trimmed.is_empty() && trimmed.parse::<u16>().is_err() {
                    return Err(format!(
                        "API Port must be a number (1-65535), got '{trimmed}'"
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Build [`VaultUpdates`] from a slice of configure settings.
///
/// Callers must run [`validate_vault_settings`] first to ensure values are valid.
pub fn build_vault_updates(settings: &[ConfigSetting]) -> VaultUpdates {
    let mut base_path: Option<String> = None;
    let mut api_port: Option<Option<u16>> = None;
    let mut api_key: Option<Option<String>> = None;
    let mut date_format: Option<Option<String>> = None;

    for setting in settings {
        match setting.key.as_str() {
            "base_path" => {
                base_path = Some(setting.value.clone());
            }
            "api_port" => {
                let trimmed = setting.value.trim();
                api_port = Some(if trimmed.is_empty() {
                    None
                } else {
                    // Pre-validated by validate_vault_settings
                    trimmed.parse::<u16>().ok()
                });
            }
            "api_key" => {
                api_key = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "date_format" => {
                date_format = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            _ => {}
        }
    }

    VaultUpdates {
        base_path,
        api_port,
        api_key,
        date_format,
    }
}

/// Build [`FieldUpdates`] from a slice of configure settings.
pub fn build_field_updates(settings: &[ConfigSetting]) -> FieldUpdates {
    let mut name: Option<String> = None;
    let mut field_type: Option<FieldType> = None;
    let mut prompt: Option<String> = None;
    let mut required: Option<Option<bool>> = None;
    let mut default: Option<Option<String>> = None;
    let mut options: Option<Option<Vec<String>>> = None;
    let mut source: Option<Option<String>> = None;
    let mut target: Option<Option<FieldTarget>> = None;
    let mut callout: Option<Option<String>> = None;
    let mut icon: Option<Option<String>> = None;
    let mut preset_exclude: Option<Option<bool>> = None;

    for setting in settings {
        match setting.key.as_str() {
            "name" => name = Some(setting.value.clone()),
            "prompt" => prompt = Some(setting.value.clone()),
            "field_type" => {
                field_type = Some(match setting.value.as_str() {
                    "text" => FieldType::Text,
                    "textarea" => FieldType::Textarea,
                    "number" => FieldType::Number,
                    "static_select" => FieldType::StaticSelect,
                    "dynamic_select" => FieldType::DynamicSelect,
                    "composite_array" => FieldType::CompositeArray,
                    "toggle" => FieldType::Toggle,
                    "counter" => FieldType::Counter,
                    _ => FieldType::Text,
                });
            }
            "required" => {
                required = Some(if setting.value == "true" {
                    Some(true)
                } else {
                    None
                });
            }
            "default" => {
                default = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "options" => {
                let items: Vec<String> = setting
                    .value
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.to_string())
                    .collect();
                options = Some(if items.is_empty() { None } else { Some(items) });
            }
            "source" => {
                source = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "target" => {
                target = Some(match setting.value.as_str() {
                    "frontmatter" => Some(FieldTarget::Frontmatter),
                    "body" => Some(FieldTarget::Body),
                    _ => None,
                });
            }
            "callout" => {
                callout = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "icon" => {
                icon = Some(if setting.value.is_empty() {
                    None
                } else {
                    Some(setting.value.clone())
                });
            }
            "preset_exclude" => {
                preset_exclude = Some(if setting.value == "true" {
                    Some(true)
                } else {
                    None
                });
            }
            _ => {}
        }
    }

    FieldUpdates {
        name,
        field_type,
        prompt,
        required,
        default,
        options,
        source,
        target,
        callout,
        callout_title: None,
        show_when: None,
        wikilink: None,
        allow_create: None,
        create_template: None,
        post_create_command: None,
        icon,
        preset_exclude,
    }
}

/// Build [`SubFieldUpdates`] from a slice of configure settings.
pub fn build_sub_field_updates(settings: &[ConfigSetting]) -> SubFieldUpdates {
    let mut name: Option<String> = None;
    let mut field_type: Option<SubFieldType> = None;
    let mut prompt: Option<String> = None;
    let mut options: Option<Option<Vec<String>>> = None;

    for setting in settings {
        match setting.key.as_str() {
            "name" => name = Some(setting.value.clone()),
            "prompt" => prompt = Some(setting.value.clone()),
            "field_type" => {
                field_type = Some(match setting.value.as_str() {
                    "number" => SubFieldType::Number,
                    "static_select" => SubFieldType::StaticSelect,
                    _ => SubFieldType::Text,
                });
            }
            "options" => {
                let items: Vec<String> = setting
                    .value
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.to_string())
                    .collect();
                options = Some(if items.is_empty() { None } else { Some(items) });
            }
            _ => {}
        }
    }

    SubFieldUpdates {
        name,
        field_type,
        prompt,
        options,
    }
}
