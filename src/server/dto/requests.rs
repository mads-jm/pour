/// Request-side wire DTOs — shapes that clients send to the server.
use std::collections::HashMap;

use serde::Deserialize;

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
// Preset request DTOs (§6.8, §6.10)
// ---------------------------------------------------------------------------

/// Request body for `PUT /api/v1/presets/{module}/{name}` (§6.8).
#[derive(Deserialize)]
pub struct PresetUpsertRequest {
    pub description: Option<String>,
    #[serde(default)]
    pub values: HashMap<String, String>,
}

/// Request body for `PUT /api/v1/presets/{module}/order` (§6.10).
#[derive(Deserialize)]
pub struct PresetReorderRequest {
    pub names: Vec<String>,
}
