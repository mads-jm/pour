/// Wire-only DTO types for `GET /api/v1/config` and error envelopes.
///
/// These are intentionally separate from the engine types in `src/config.rs`.
/// The wire format is contract-bound; engine refactors must not silently break
/// the response shape. Each optional field serializes to JSON `null` when None
/// (no `skip_serializing_if` — the contract requires explicit null).
use std::collections::HashMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error envelope (§5.2)
// ---------------------------------------------------------------------------

/// Standard error codes from §5.2 of the API contract.
///
/// These are string constants (not an enum variant) so they serialize as
/// bare strings in the JSON body, exactly as the contract specifies.
pub mod error_codes {
    pub const UNAUTHORIZED: &str = "unauthorized";
    pub const NOT_FOUND: &str = "not_found";
    pub const METHOD_NOT_ALLOWED: &str = "method_not_allowed";
    pub const VALIDATION_FAILED: &str = "validation_failed";
    pub const UNSUPPORTED_MEDIA_TYPE: &str = "unsupported_media_type";
    pub const PAYLOAD_TOO_LARGE: &str = "payload_too_large";
    pub const TRANSPORT_ERROR: &str = "transport_error";
    pub const WRITE_ERROR: &str = "write_error";
    pub const INTERNAL_ERROR: &str = "internal_error";
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: ErrorDetail,
}

/// Build a JSON error envelope response for any non-2xx status.
///
/// Usage: `error_response(StatusCode::UNAUTHORIZED, error_codes::UNAUTHORIZED, "...")`
pub fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: ErrorDetail {
                code: code.to_string(),
                message: message.to_string(),
            },
        }),
    )
        .into_response()
}

use crate::config::{
    Config, FieldConfig, FieldTarget, FieldType, ModuleConfig, ShowWhen, SubFieldConfig,
    SubFieldType, TemplateConfig, TemplateFieldConfig, TemplateFieldType,
};

// ---------------------------------------------------------------------------
// Top-level response
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ConfigResponse {
    pub modules: Vec<ModuleDto>,
    pub module_order: Vec<String>,
    pub templates: HashMap<String, TemplateDto>,
    pub vault: VaultDto,
    pub config_version: String,
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ModuleDto {
    pub key: String,
    pub display_name: Option<String>,
    pub icon: Option<String>,
    pub mode: &'static str,
    pub fields: Vec<FieldDto>,
    pub callout_type: Option<String>,
    pub append_under_header: Option<String>,
    pub append_template: Option<String>,
    pub append_shallow: bool,
    pub daily_link: bool,
}

// ---------------------------------------------------------------------------
// Field
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct FieldDto {
    pub name: String,
    pub field_type: &'static str,
    pub prompt: String,
    pub required: bool,
    pub default: Option<String>,
    pub options: Option<Vec<String>>,
    pub source: Option<String>,
    pub target: Option<&'static str>,
    pub callout: Option<String>,
    pub callout_title: Option<String>,
    pub allow_create: Option<bool>,
    pub wikilink: Option<bool>,
    pub create_template: Option<String>,
    pub post_create_command: Option<String>,
    pub show_when: Option<ShowWhenDto>,
    pub icon: Option<String>,
    pub preset_exclude: bool,
    pub list: bool,
    pub sub_fields: Option<Vec<SubFieldDto>>,
}

// ---------------------------------------------------------------------------
// show_when
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ShowWhenDto {
    pub field: String,
    pub equals: Option<String>,
    pub one_of: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Sub-field (composite_array column)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SubFieldDto {
    pub name: String,
    pub field_type: &'static str,
    pub prompt: String,
    pub options: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Template
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct TemplateDto {
    pub path: String,
    pub fields: Vec<TemplateFieldDto>,
}

#[derive(Serialize)]
pub struct TemplateFieldDto {
    pub name: String,
    pub field_type: &'static str,
    pub prompt: String,
    pub options: Option<Vec<String>>,
    pub default: Option<String>,
    pub allow_create: bool,
}

// ---------------------------------------------------------------------------
// Vault sub-object
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct VaultDto {
    pub date_format: String,
    pub transport_mode: &'static str,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn field_type_wire(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::Text => "text",
        FieldType::Textarea => "textarea",
        FieldType::Number => "number",
        FieldType::StaticSelect => "static_select",
        FieldType::DynamicSelect => "dynamic_select",
        FieldType::CompositeArray => "composite_array",
    }
}

fn sub_field_type_wire(ft: &SubFieldType) -> &'static str {
    match ft {
        SubFieldType::Text => "text",
        SubFieldType::Number => "number",
        SubFieldType::StaticSelect => "static_select",
    }
}

fn template_field_type_wire(ft: &TemplateFieldType) -> &'static str {
    match ft {
        TemplateFieldType::Text => "text",
        TemplateFieldType::Number => "number",
        TemplateFieldType::StaticSelect => "static_select",
    }
}

fn target_wire(t: &FieldTarget) -> &'static str {
    match t {
        FieldTarget::Frontmatter => "frontmatter",
        FieldTarget::Body => "body",
    }
}

fn write_mode_wire(mode: &crate::config::WriteMode) -> &'static str {
    match mode {
        crate::config::WriteMode::Append => "append",
        crate::config::WriteMode::Create => "create",
    }
}

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

// ---------------------------------------------------------------------------
// Error envelope with structured `details` (§5.2)
// ---------------------------------------------------------------------------

/// Error envelope that carries structured `details` alongside the standard
/// `code` + `message` fields.
///
/// `details` is `serde_json::Value` so each endpoint can embed arbitrary
/// structures (field error lists, engine error strings, etc.) without
/// requiring a new Rust type per call site.
pub fn error_response_with_details(
    status: StatusCode,
    code: &str,
    message: &str,
    details: serde_json::Value,
) -> Response {
    #[derive(Serialize)]
    struct Inner {
        code: String,
        message: String,
        details: serde_json::Value,
    }
    #[derive(Serialize)]
    struct Envelope {
        error: Inner,
    }
    (
        status,
        Json(Envelope {
            error: Inner {
                code: code.to_string(),
                message: message.to_string(),
                details,
            },
        }),
    )
        .into_response()
}

// Add a read_error code constant.
pub mod extra_error_codes {
    pub const READ_ERROR: &str = "read_error";
    pub const CAPTURED_AT_OUT_OF_RANGE: &str = "captured_at_out_of_range";
    pub const AUTO_CREATE_INPUT_REQUIRED: &str = "auto_create_input_required";
    pub const IDEMPOTENCY_REPLAY_IN_FLIGHT: &str = "idempotency_replay_in_flight";
}

// ---------------------------------------------------------------------------
// History response DTOs (§6.5)
// ---------------------------------------------------------------------------

/// Entry in the history list. Matches the wire shape in §6.5.
#[derive(Serialize)]
pub struct HistoryEntryDto {
    pub id: String,
    pub module_key: String,
    pub timestamp: String,
    pub vault_path: String,
    pub first_field: Option<String>,
}

/// Summary block in the history response (only on the dashboard call, per §6.5).
#[derive(Serialize)]
pub struct HistorySummaryDto {
    pub version: u32,
    pub last_pour: Option<HistoryEntryDto>,
    pub today_count: usize,
    pub week_count: usize,
    pub streak_days: u64,
    pub per_module_today: HashMap<String, usize>,
}

/// Full response for `GET /api/v1/history` (§6.5).
///
/// `next_cursor` replaces the old `next_until` timestamp cursor (§6.5 amendment
/// 2026-04-26). It is an opaque string (the last entry's `id`) that the client
/// passes as `?cursor=<next_cursor>` for the next page. Cursor-based pagination
/// is exact even when multiple entries share the same millisecond timestamp.
#[derive(Serialize)]
pub struct HistoryResponse {
    pub entries: Vec<HistoryEntryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<HistorySummaryDto>,
    pub has_more: bool,
    /// Opaque pagination cursor. Pass as `?cursor=<next_cursor>` for the next
    /// page. `null` when `has_more` is false.
    pub next_cursor: Option<String>,
}

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
// Presets response DTOs (§6.7–§6.10)
// ---------------------------------------------------------------------------

/// A single preset in the wire format.
#[derive(Serialize)]
pub struct PresetDto {
    pub name: String,
    pub description: Option<String>,
    pub values: HashMap<String, String>,
}

/// Response for `GET /api/v1/presets/{module}` and `PUT /api/v1/presets/{module}/order` (§6.7, §6.10).
#[derive(Serialize)]
pub struct PresetsListResponse {
    pub presets: Vec<PresetDto>,
}

/// Request body for `PUT /api/v1/presets/{module}/{name}` (§6.8).
#[derive(Deserialize)]
pub struct PresetUpsertRequest {
    pub description: Option<String>,
    #[serde(default)]
    pub values: HashMap<String, String>,
}

/// Response body for `PUT /api/v1/presets/{module}/{name}` (§6.8).
#[derive(Serialize)]
pub struct PresetUpsertResponse {
    pub preset: PresetDto,
}

/// Request body for `PUT /api/v1/presets/{module}/order` (§6.10).
#[derive(Deserialize)]
pub struct PresetReorderRequest {
    pub names: Vec<String>,
}

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
// Submit request DTO (§6.4)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    pub field_values: HashMap<String, String>,
    #[serde(default)]
    pub composite_data: HashMap<String, Vec<Vec<String>>>,
    #[serde(default)]
    pub callout_overrides: HashMap<String, String>,
    #[serde(default)]
    pub callout_titles: HashMap<String, String>,
    #[serde(default)]
    pub auto_create_inputs: HashMap<String, HashMap<String, String>>,
    pub captured_at: Option<chrono::DateTime<chrono::Utc>>,
    pub client_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Submit response DTO (§6.4)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct AutoCreatedDto {
    pub field: String,
    pub value: String,
    pub vault_path: String,
    pub templated: bool,
}

#[derive(Serialize)]
pub struct PostCreateCommandDto {
    pub field: String,
    pub command: String,
    pub fired: bool,
}

#[derive(Serialize)]
pub struct SubmitWarningDto {
    pub code: &'static str,
    /// The field name that triggered this warning, if applicable.
    /// `None` (serializes to JSON `null`) for non-field warnings such as
    /// history record failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct SubmitResponse {
    pub vault_path: String,
    pub transport_mode: &'static str,
    pub auto_created: Vec<AutoCreatedDto>,
    pub post_create_commands: Vec<PostCreateCommandDto>,
    pub history_id: String,
    pub captured_at: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<SubmitWarningDto>,
}

// ---------------------------------------------------------------------------
// Captures response DTO (§6.6)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CaptureResponse {
    pub id: String,
    pub module_key: String,
    pub timestamp: String,
    pub vault_path: String,
    pub content: String,
    pub transport_mode: &'static str,
}
