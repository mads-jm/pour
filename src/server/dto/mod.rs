/// Wire-only DTO types for the Pour server API.
///
/// These are intentionally separate from the engine types in `src/config.rs`.
/// The wire format is contract-bound; engine refactors must not silently break
/// the response shape. Each optional field serializes to JSON `null` when None
/// (no `skip_serializing_if` — the contract requires explicit null).
///
/// ## Module layout
/// - `response`  — server-to-client shapes (Serialize only, no Config imports)
/// - `requests`  — client-to-server shapes (Deserialize only)
/// - `mapping`   — Config→DTO `From` impls and `build_config_response`
pub mod mapping;
pub mod requests;
pub mod response;

// Re-export the entire public surface so existing `use super::dto::Foo` sites
// continue to compile without modification.
pub use mapping::build_config_response;
pub use requests::{PresetReorderRequest, PresetUpsertRequest, SubmitRequest};
// Only re-export items accessed via `dto::Foo` by other server modules.
// `ConfigResponse`, `FieldDto`, etc. are only used within dto/mapping.rs itself
// and don't need to be re-exported here.
pub use response::{
    AutoCreatedDto, CaptureResponse, HistoryEntryDto, HistoryResponse, HistorySummaryDto,
    PostCreateCommandDto, PresetDto, PresetUpsertResponse, PresetsListResponse, SubmitResponse,
    SubmitWarningDto,
};

// ---------------------------------------------------------------------------
// Error envelope (§5.2)
// ---------------------------------------------------------------------------

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

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

// ---------------------------------------------------------------------------
// Extra error codes (§5.2 extensions)
// ---------------------------------------------------------------------------

pub mod extra_error_codes {
    pub const READ_ERROR: &str = "read_error";
    pub const CAPTURED_AT_OUT_OF_RANGE: &str = "captured_at_out_of_range";
    pub const AUTO_CREATE_INPUT_REQUIRED: &str = "auto_create_input_required";
    pub const IDEMPOTENCY_REPLAY_IN_FLIGHT: &str = "idempotency_replay_in_flight";
}
