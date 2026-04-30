/// Step 2 — Idempotency-Key header extraction, format validation, and
/// cache lookup.
///
/// Covers §9 of the API contract:
/// - Extract the `Idempotency-Key` header (optional).
/// - Validate format: 1–256 ASCII printable characters.
/// - Return `InFlight` / `Replay` / `Fresh` outcomes via the shared
///   `IdempotencyOutcome` enum, letting `mod.rs` decide whether to short-
///   circuit or proceed.
///
/// This step does NOT call `complete()` or `release()` — that is the
/// responsibility of the outer `handler` wrapper in `mod.rs` which holds the
/// key for the full round-trip.
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::server::{
    AppState,
    dto::{error_codes, error_response, extra_error_codes},
    idempotency::IdempotencyOutcome,
};

/// Outcome from the idempotency lookup step.
pub(super) enum IdempotencyResult {
    /// No `Idempotency-Key` header was present — continue as normal.
    NoKey,
    /// Key was present and is fresh — proceed; key is stored in the returned
    /// `String` so the outer handler can call `complete()` / `release()`.
    Fresh(String),
    /// Key was present but rejected (invalid format, in-flight, or replay).
    /// The `Response` is ready to return immediately.
    Done(Response),
}

/// Extract and check the idempotency key from the request headers.
///
/// This function is synchronous — no I/O, just header parsing + cache lookup.
pub(super) fn run(headers: &HeaderMap, state: &AppState) -> IdempotencyResult {
    let key = match headers.get("idempotency-key").and_then(|v| v.to_str().ok()) {
        Some(k) => k.to_string(),
        None => return IdempotencyResult::NoKey,
    };

    // Validate format: 1–256 ASCII printable characters.
    if key.is_empty()
        || key.len() > 256
        || !key.chars().all(|c| c.is_ascii() && !c.is_ascii_control())
    {
        return IdempotencyResult::Done(error_response(
            StatusCode::BAD_REQUEST,
            error_codes::VALIDATION_FAILED,
            "Idempotency-Key must be 1–256 ASCII printable characters.",
        ));
    }

    match state.idempotency.get_or_insert_in_flight(&key) {
        IdempotencyOutcome::InFlight => IdempotencyResult::Done(error_response(
            StatusCode::CONFLICT,
            extra_error_codes::IDEMPOTENCY_REPLAY_IN_FLIGHT,
            "This Idempotency-Key is currently being processed.",
        )),
        IdempotencyOutcome::Replay { status, body } => {
            let mut resp = Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Replay", "true")
                .body(axum::body::Body::from(body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
            resp.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            IdempotencyResult::Done(resp)
        }
        IdempotencyOutcome::Fresh => IdempotencyResult::Fresh(key),
    }
}
