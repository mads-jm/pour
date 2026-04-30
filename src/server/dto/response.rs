/// Response-side wire DTOs — shapes that the server sends to clients.
///
/// These types are intentionally free of `crate::config::*` imports.
/// All Config→DTO translation lives in `mapping.rs`.
use std::collections::HashMap;

use serde::Serialize;

// ---------------------------------------------------------------------------
// Config response (§6.1)
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

/// Response body for `PUT /api/v1/presets/{module}/{name}` (§6.8).
#[derive(Serialize)]
pub struct PresetUpsertResponse {
    pub preset: PresetDto,
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
