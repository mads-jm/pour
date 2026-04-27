/// Handler for `GET /api/v1/history` (§6.5).
///
/// Returns paginated capture history with optional filtering by `since`,
/// `until`, `module`, and `limit`. Includes a `summary` block when neither
/// `since` nor `until` are provided (the dashboard call).
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;

use super::super::{
    AppState,
    dto::{
        HistoryEntryDto, HistoryResponse, HistorySummaryDto,
        error_codes, error_response_with_details,
    },
};
use crate::data::history::History;

/// Default limit for history pagination (§6.5).
const DEFAULT_LIMIT: usize = 100;
/// Maximum limit (§6.5).
const MAX_LIMIT: usize = 1000;

/// Raw query parameters for `GET /api/v1/history`.
///
/// All fields are `Option<String>` so we can produce precise validation errors
/// rather than letting serde silently reject bad inputs.
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    since: Option<String>,
    until: Option<String>,
    /// Opaque pagination cursor: the `next_cursor` from the previous response.
    /// When present, the server returns entries with id < cursor (i.e. older).
    /// Takes priority over `until` for pagination (§6.5).
    cursor: Option<String>,
    limit: Option<String>,
    module: Option<String>,
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    // -- Parse `since` -------------------------------------------------------
    let since: Option<DateTime<Utc>> = if let Some(ref s) = query.since {
        match s.parse::<DateTime<Utc>>() {
            Ok(dt) => Some(dt),
            Err(_) => {
                return error_response_with_details(
                    StatusCode::BAD_REQUEST,
                    error_codes::VALIDATION_FAILED,
                    "Invalid 'since' — must be ISO 8601 / RFC 3339.",
                    json!({ "field": "since" }),
                );
            }
        }
    } else {
        None
    };

    // -- Parse `until` -------------------------------------------------------
    let until: Option<DateTime<Utc>> = if let Some(ref u) = query.until {
        match u.parse::<DateTime<Utc>>() {
            Ok(dt) => Some(dt),
            Err(_) => {
                return error_response_with_details(
                    StatusCode::BAD_REQUEST,
                    error_codes::VALIDATION_FAILED,
                    "Invalid 'until' — must be ISO 8601 / RFC 3339.",
                    json!({ "field": "until" }),
                );
            }
        }
    } else {
        None
    };

    // -- Parse `limit` -------------------------------------------------------
    let limit: usize = if let Some(ref s) = query.limit {
        match s.parse::<usize>() {
            Ok(n) if (1..=MAX_LIMIT).contains(&n) => n,
            _ => {
                return error_response_with_details(
                    StatusCode::BAD_REQUEST,
                    error_codes::VALIDATION_FAILED,
                    "Invalid 'limit' — must be an integer between 1 and 1000.",
                    json!({ "field": "limit" }),
                );
            }
        }
    } else {
        DEFAULT_LIMIT
    };

    // -- Validate `module` ---------------------------------------------------
    let module: Option<&str> = match query.module.as_deref() {
        None => None,
        Some(m) => {
            if !state.config.modules.contains_key(m) {
                return error_response_with_details(
                    StatusCode::BAD_REQUEST,
                    error_codes::VALIDATION_FAILED,
                    "Unknown module key.",
                    json!({ "code": "unknown_module", "field": "module" }),
                );
            }
            Some(m)
        }
    };

    // -- Dashboard summary flag ----------------------------------------------
    // `summary` is included only when neither `since` nor `until` are present.
    let include_summary = since.is_none() && until.is_none();

    // -- Load history and filter ---------------------------------------------
    let history = History::load();
    let cursor_str = query.cursor.as_deref();
    let (entries, has_more, next_cursor) =
        history.filter(since, until, cursor_str, module, limit);

    // Build entry DTOs.
    let entry_dtos: Vec<HistoryEntryDto> = entries.iter().map(HistoryEntryDto::from).collect();

    // Build summary (only on dashboard call).
    let summary = if include_summary {
        let s = history.summary();
        let last_pour_dto = s.last_pour.as_ref().map(HistoryEntryDto::from);
        Some(HistorySummaryDto {
            version: s.version,
            last_pour: last_pour_dto,
            today_count: s.today_count,
            week_count: s.week_count,
            streak_days: s.streak,
            per_module_today: s.per_module_today.clone(),
        })
    } else {
        None
    };

    Json(HistoryResponse {
        entries: entry_dtos,
        summary,
        has_more,
        next_cursor,
    })
    .into_response()
}
