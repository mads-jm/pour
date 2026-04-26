/// Handler for `GET /api/v1/options/{module}/{field}` (§6.3).
///
/// Resolves options for a `dynamic_select` field using a 3-tier fallback:
/// transport → cache → empty. Exposes the tier that served the result so
/// the PWA can show "live" vs "cached" badges.
///
/// Cache strategy: per-request load+save rather than a global Mutex. This
/// works correctly for low-traffic LAN use but races on concurrent requests
/// (last-write-wins) and incurs per-request disk I/O.
/// TODO(step-d-or-later): move to Arc<tokio::sync::Mutex<Cache>> in AppState,
/// constructed once in `pour::server::run`. The options handler would lock,
/// call fetch_options, and persist on shutdown (or in a background task)
/// rather than on every request.
use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use super::super::{AppState, dto::{error_codes, error_response}};
use crate::config::FieldType;
use crate::data::cache::Cache;
use crate::transport::Transport;

#[derive(Serialize)]
pub struct OptionsResponse {
    pub options: Vec<String>,
    pub source_path: String,
    pub tier: &'static str,
}

pub async fn handler(
    State(state): State<AppState>,
    Path((module_key, field_name)): Path<(String, String)>,
) -> Response {
    // Module lookup — 404 if unknown or mobile-invisible (same rule as config).
    let module = match state.config.modules.get(&module_key) {
        Some(m) if m.is_mobile_visible() => m,
        _ => {
            return error_response(
                axum::http::StatusCode::NOT_FOUND,
                error_codes::NOT_FOUND,
                "Unknown module.",
            );
        }
    };

    // Field lookup — 404 if unknown.
    let field = match module.fields.iter().find(|f| f.name == field_name) {
        Some(f) => f,
        None => {
            return error_response(
                axum::http::StatusCode::NOT_FOUND,
                error_codes::NOT_FOUND,
                "Unknown field.",
            );
        }
    };

    // 400 if not a dynamic_select.
    if field.field_type != FieldType::DynamicSelect {
        return error_response(
            axum::http::StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Field is not a dynamic_select.",
        );
    }

    // No source configured → empty tier.
    let source_path = match field.source.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            return Json(OptionsResponse {
                options: vec![],
                source_path: String::new(),
                tier: "empty",
            })
            .into_response();
        }
    };

    // 3-tier fetch with tier tracking.
    let (options, tier) = fetch_with_tier(&state.transport, source_path).await;

    Json(OptionsResponse {
        options,
        source_path: source_path.to_string(),
        tier,
    })
    .into_response()
}

/// Fetch options and report which tier served them.
///
/// Per-request load+save strategy: avoids a global Mutex while still updating
/// the on-disk cache on successful transport fetches.
async fn fetch_with_tier(transport: &Transport, source_path: &str) -> (Vec<String>, &'static str) {
    // Tier 1: transport.
    if let Ok(items) = transport.list_directory(source_path).await
        && !items.is_empty()
    {
        let normalized = normalize(items);
        // Best-effort cache update.
        let mut cache = Cache::load();
        cache.set(source_path, normalized.clone());
        let _ = cache.save();
        return (normalized, "transport");
    }

    // Tier 2: cache.
    let cache = Cache::load();
    if let Some(items) = cache.get(source_path)
        && !items.is_empty()
    {
        return (items, "cache");
    }

    // Tier 3: empty.
    (vec![], "empty")
}

/// Normalize raw directory entries to file stems (strip `.md`, exclude dirs).
fn normalize(items: Vec<String>) -> Vec<String> {
    use std::path::Path;
    items
        .into_iter()
        .filter(|s| !s.ends_with('/'))
        .map(|s| {
            Path::new(&s)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&s)
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
