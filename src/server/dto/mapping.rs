/// Config→DTO translation layer.
///
/// All `From<&ConfigType>` impls and `build_config_response` live here,
/// isolated from the HTTP layer so they can be tested without a running server.
use std::collections::HashMap;

use crate::config::{
    Config, FieldConfig, FieldTarget, FieldType, ModuleConfig, ShowWhen, SubFieldConfig,
    SubFieldType, TemplateConfig, TemplateFieldConfig, TemplateFieldType,
};

use super::response::{
    ConfigResponse, FieldDto, ModuleDto, ShowWhenDto, SubFieldDto, TemplateDto, TemplateFieldDto,
    VaultDto,
};

// ---------------------------------------------------------------------------
// Wire-string helpers (Config enum → &'static str)
// ---------------------------------------------------------------------------

pub fn field_type_wire(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Text => "text",
        FieldType::Textarea => "textarea",
        FieldType::Number => "number",
        FieldType::StaticSelect => "static_select",
        FieldType::DynamicSelect => "dynamic_select",
        FieldType::CompositeArray => "composite_array",
    }
}

pub fn sub_field_type_wire(ft: &SubFieldType) -> &'static str {
    match ft {
        SubFieldType::Text => "text",
        SubFieldType::Number => "number",
        SubFieldType::StaticSelect => "static_select",
    }
}

pub fn template_field_type_wire(ft: &TemplateFieldType) -> &'static str {
    match ft {
        TemplateFieldType::Text => "text",
        TemplateFieldType::Number => "number",
        TemplateFieldType::StaticSelect => "static_select",
    }
}

pub fn target_wire(t: &FieldTarget) -> &'static str {
    match t {
        FieldTarget::Frontmatter => "frontmatter",
        FieldTarget::Body => "body",
    }
}

pub fn write_mode_wire(mode: &crate::config::WriteMode) -> &'static str {
    match mode {
        crate::config::WriteMode::Append => "append",
        crate::config::WriteMode::Create => "create",
    }
}

// ---------------------------------------------------------------------------
// From impls for Config → DTO
// ---------------------------------------------------------------------------

impl From<&ShowWhen> for ShowWhenDto {
    fn from(sw: &ShowWhen) -> Self {
        ShowWhenDto {
            field: sw.field.clone(),
            equals: sw.equals.clone(),
            one_of: sw.one_of.clone(),
        }
    }
}

impl From<&SubFieldConfig> for SubFieldDto {
    fn from(sf: &SubFieldConfig) -> Self {
        SubFieldDto {
            name: sf.name.clone(),
            field_type: sub_field_type_wire(&sf.field_type),
            prompt: sf.prompt.clone(),
            options: sf.options.clone(),
        }
    }
}

impl From<&FieldConfig> for FieldDto {
    fn from(f: &FieldConfig) -> Self {
        FieldDto {
            name: f.name.clone(),
            field_type: field_type_wire(&f.field_type),
            prompt: f.prompt.clone(),
            required: f.required.unwrap_or(false),
            default: f.default.clone(),
            options: f.options.clone(),
            source: f.source.clone(),
            target: f.target.as_ref().map(target_wire),
            callout: f.callout.clone(),
            callout_title: f.callout_title.clone(),
            allow_create: f.allow_create,
            wikilink: f.wikilink,
            create_template: f.create_template.clone(),
            post_create_command: f.post_create_command.clone(),
            show_when: f.show_when.as_ref().map(ShowWhenDto::from),
            icon: f.icon.clone(),
            preset_exclude: f.preset_exclude.unwrap_or(false),
            list: f.list,
            sub_fields: f
                .sub_fields
                .as_ref()
                .map(|sfs| sfs.iter().map(SubFieldDto::from).collect()),
        }
    }
}

impl ModuleDto {
    pub fn from_module_with_key(key: &str, m: &ModuleConfig) -> Self {
        ModuleDto {
            key: key.to_string(),
            display_name: m.display_name.clone(),
            icon: m.icon.clone(),
            mode: write_mode_wire(&m.mode),
            fields: m.fields.iter().map(FieldDto::from).collect(),
            callout_type: m.callout_type.clone(),
            append_under_header: m.append_under_header.clone(),
            append_template: m.append_template.clone(),
            append_shallow: m.append_shallow.unwrap_or(false),
            daily_link: m.daily_link.unwrap_or(false),
        }
    }
}

impl From<&TemplateFieldConfig> for TemplateFieldDto {
    fn from(tf: &TemplateFieldConfig) -> Self {
        TemplateFieldDto {
            name: tf.name.clone(),
            field_type: template_field_type_wire(&tf.field_type),
            prompt: tf.prompt.clone(),
            options: tf.options.clone(),
            default: tf.default.clone(),
            allow_create: tf.allow_create.unwrap_or(false),
        }
    }
}

impl From<&TemplateConfig> for TemplateDto {
    fn from(tc: &TemplateConfig) -> Self {
        TemplateDto {
            path: tc.path.clone(),
            fields: tc.fields.iter().map(TemplateFieldDto::from).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// History entry mapping
// ---------------------------------------------------------------------------

use super::response::HistoryEntryDto;

/// Convert a `HistoryEntry` to its wire DTO form.
/// Entries without an id (legacy) get an empty string id.
impl From<&crate::data::history::HistoryEntry> for HistoryEntryDto {
    fn from(e: &crate::data::history::HistoryEntry) -> Self {
        HistoryEntryDto {
            id: e.id.clone().unwrap_or_default(),
            module_key: e.module_key.clone(),
            timestamp: e.timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            vault_path: e.vault_path.clone(),
            first_field: e.first_field.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Preset entry mapping
// ---------------------------------------------------------------------------

use super::response::PresetDto;

impl From<&crate::data::presets::PresetEntry> for PresetDto {
    fn from(p: &crate::data::presets::PresetEntry) -> Self {
        PresetDto {
            name: p.name.clone(),
            description: p.description.clone(),
            values: p.values.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// build_config_response
// ---------------------------------------------------------------------------

/// Build a `ConfigResponse` from the given `Config` and transport mode string.
///
/// Applies ordering: modules in `module_order` first, then unlisted modules
/// alphabetically. Modules with `mobile_visible = false` are omitted entirely.
pub fn build_config_response(config: &Config, transport_mode: &'static str) -> ConfigResponse {
    // Determine ordered module keys, filtered to mobile-visible only.
    let all_visible_keys: Vec<&str> = config
        .modules
        .keys()
        .filter(|k| config.modules[*k].is_mobile_visible())
        .map(|k| k.as_str())
        .collect();

    // Keys from module_order that are present and visible.
    let ordered_keys: Vec<&str> = config
        .module_order
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|k| k.as_str())
        .filter(|k| all_visible_keys.contains(k))
        .collect();

    // Unlisted visible keys, sorted alphabetically.
    let ordered_set: std::collections::HashSet<&str> = ordered_keys.iter().copied().collect();
    let mut unlisted: Vec<&str> = all_visible_keys
        .iter()
        .copied()
        .filter(|k| !ordered_set.contains(k))
        .collect();
    unlisted.sort_unstable();

    let final_keys: Vec<&str> = ordered_keys.into_iter().chain(unlisted).collect();

    let modules: Vec<ModuleDto> = final_keys
        .iter()
        .filter_map(|k| {
            config
                .modules
                .get(*k)
                .map(|m| ModuleDto::from_module_with_key(k, m))
        })
        .collect();

    let templates: HashMap<String, TemplateDto> = config
        .templates
        .as_ref()
        .map(|ts| {
            ts.iter()
                .map(|(k, v)| (k.clone(), TemplateDto::from(v)))
                .collect()
        })
        .unwrap_or_default();

    let date_format = config
        .vault
        .date_format
        .clone()
        .unwrap_or_else(|| "%Y%m%d".to_string());

    // Build the filtered module_order: only keys that survived the visibility
    // filter, in the same order they appear in the final_keys list.
    let filtered_module_order: Vec<String> = config
        .module_order
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|k| {
            config
                .modules
                .get(k.as_str())
                .map(|m| m.is_mobile_visible())
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    ConfigResponse {
        modules,
        module_order: filtered_module_order,
        templates,
        vault: VaultDto {
            date_format,
            transport_mode,
        },
        config_version: config
            .config_version
            .clone()
            .unwrap_or_else(|| "0.1.0".to_string()),
    }
}
