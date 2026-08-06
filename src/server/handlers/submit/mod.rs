/// Handler for `POST /api/v1/submit/{module}` (§6.4).
///
/// The keystone endpoint. Pipeline:
/// 1. Validate auth (middleware).
/// 2. Content-Type + idempotency-key checks.
/// 3. Validate + parse body, module lookup, `captured_at`, visibility,
///    field validation → [`validate`].
/// 4. Auto-create novel `dynamic_select` values → [`autocreate_step`].
/// 5. Main write (`write_create` / `write_append`) → [`write_step`].
/// 6. History recording → [`history_step`].
/// 7. 201 response with `Location` header.
///
/// Logging: module name + vault path + autocreate count on success;
/// module name + error code on failure. NO field values, NO composite
/// data, NO user content (§14).
mod autocreate_step;
mod history_step;
mod idempotency_lookup;
mod validate;
mod write_step;

use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::body::to_bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Local, Utc};

use super::super::{
    AppState,
    dto::{AutoCreatedDto, PostCreateCommandDto, SubmitResponse, SubmitWarningDto},
};
use crate::config::FieldType;

// ---------------------------------------------------------------------------
// SubmitContext
// ---------------------------------------------------------------------------

/// Mutable pipeline state threaded through each submit step.
///
/// Populated incrementally: `validate::run` fills the request-derived fields;
/// subsequent steps populate `auto_created`, `vault_path`, `history_id`, etc.
pub(super) struct SubmitContext {
    // -- From routing --------------------------------------------------------
    /// The module key extracted from the URL path.
    pub module_key: String,

    // -- From validate::run --------------------------------------------------
    /// Visible field values after show_when filtering (§6.4 belt-and-suspenders).
    pub field_values: HashMap<String, String>,
    /// Owned set of visible field names (resolved by show_when).
    pub visible_names: HashSet<String>,
    /// Ordered list of (field_name, field_type) for all module fields.
    /// Used by `history_step` to find `first_field` without re-borrowing state.
    pub field_order: Vec<(String, FieldType)>,
    /// Resolved submission timestamp in UTC.
    pub now_utc: DateTime<Utc>,
    /// Resolved submission timestamp in the local timezone (for file paths).
    pub now_local: DateTime<Local>,
    /// Sub-form inputs for templated auto-create fields.
    pub auto_create_inputs: HashMap<String, HashMap<String, String>>,
    /// Composite array data keyed by field name.
    pub composite_data: HashMap<String, Vec<Vec<String>>>,
    /// Per-field callout style overrides.
    pub callout_overrides: HashMap<String, String>,
    /// Per-field callout title overrides.
    pub callout_titles: HashMap<String, String>,

    // -- From autocreate_step::run -------------------------------------------
    /// Notes created during auto-create (reported in 201 response).
    pub auto_created: Vec<AutoCreatedDto>,
    /// Post-create commands fired during auto-create.
    pub post_create_commands: Vec<PostCreateCommandDto>,
    /// Non-fatal warnings accumulated across all steps.
    pub warnings: Vec<SubmitWarningDto>,

    // -- From write_step::run ------------------------------------------------
    /// Vault-relative path of the written note.
    pub vault_path: String,
    /// The transport the write actually used. Follows `state.transport_mode`
    /// except for a module that overrides the vault root, which is always
    /// filesystem — see `Transport::for_module`.
    pub transport_mode: crate::transport::TransportMode,

    // -- From history_step::run ----------------------------------------------
    /// Unique history record id (used in the `Location` header).
    pub history_id: String,
}

impl SubmitContext {
    /// `transport_mode` starts at the app-level mode — the truth for every
    /// module that writes to the vault. `write_step` refines it for a module
    /// that overrides the root.
    fn new(module_key: String, transport_mode: crate::transport::TransportMode) -> Self {
        Self {
            module_key,
            transport_mode,
            field_values: HashMap::new(),
            visible_names: HashSet::new(),
            field_order: Vec::new(),
            now_utc: Utc::now(),
            now_local: Utc::now().with_timezone(&Local),
            auto_create_inputs: HashMap::new(),
            composite_data: HashMap::new(),
            callout_overrides: HashMap::new(),
            callout_titles: HashMap::new(),
            auto_created: Vec::new(),
            post_create_commands: Vec::new(),
            warnings: Vec::new(),
            vault_path: String::new(),
            history_id: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Public handler — Axum entry point
// ---------------------------------------------------------------------------

pub async fn handler(
    State(state): State<AppState>,
    Path(module_key): Path<String>,
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    // -- Content-Type check (§4, §6.4) ------------------------------------
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("application/json") {
        use super::super::dto::{error_codes, error_response};
        return error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            error_codes::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be application/json.",
        );
    }

    // -- Idempotency-Key handling (§9) ------------------------------------
    let idempotency_outcome = idempotency_lookup::run(req.headers(), &state);

    let idempotency_key: Option<String> = match idempotency_outcome {
        idempotency_lookup::IdempotencyResult::NoKey => None,
        idempotency_lookup::IdempotencyResult::Fresh(key) => Some(key),
        idempotency_lookup::IdempotencyResult::Done(resp) => return resp,
    };

    // -- Inner pipeline ---------------------------------------------------
    let resp = submit_inner(&state, module_key, req).await;

    // -- Idempotency completion (§9) --------------------------------------
    if let Some(ref key) = idempotency_key {
        let (parts, body) = resp.into_parts();
        let bytes = to_bytes(body, usize::MAX).await.unwrap_or_default();
        if parts.status.is_success() {
            state
                .idempotency
                .complete(key, parts.status, bytes.to_vec());
        } else {
            state.idempotency.release(key);
        }
        let mut rebuilt = Response::from_parts(parts, axum::body::Body::from(bytes));
        rebuilt
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        rebuilt
    } else {
        resp
    }
}

// ---------------------------------------------------------------------------
// Inner pipeline
// ---------------------------------------------------------------------------

async fn submit_inner(
    state: &AppState,
    module_key: String,
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    let mut ctx = SubmitContext::new(module_key.clone(), state.transport_mode);

    // Step 1: validate (parse body, module lookup, captured_at, visibility,
    // field validation).
    let validated = match validate::run(state, &module_key, req).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Populate context from validated data.
    ctx.field_values = validated.field_values;
    ctx.visible_names = validated.visible_names;
    ctx.now_utc = validated.now_utc;
    ctx.now_local = validated.now_local;
    ctx.auto_create_inputs = validated.auto_create_inputs;
    ctx.composite_data = validated.composite_data;
    ctx.callout_overrides = validated.callout_overrides;
    ctx.callout_titles = validated.callout_titles;

    // Capture field order from the validated module (needed by history_step
    // to find first_field without re-borrowing state).
    ctx.field_order = state
        .config
        .modules
        .get(&module_key)
        .map(|m| {
            m.fields
                .iter()
                .map(|f| (f.name.clone(), f.field_type.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Step 2: autocreate (best-effort; transport errors become warnings).
    if let Err(resp) = autocreate_step::run(&mut ctx, state).await {
        return resp;
    }

    // Step 3: main write.
    if let Err(resp) = write_step::run(&mut ctx, state).await {
        return resp;
    }

    // Step 4: history (non-fatal; failures become warnings).
    history_step::run(&mut ctx);

    // -- Domain log: submit ok (no field values per §14) ------------------
    tracing::info!(
        module = %module_key,
        vault_path = %ctx.vault_path,
        autocreate = ctx.auto_created.len(),
        "submit ok"
    );

    // -- Build 201 response -----------------------------------------------
    // ctx, not state: a root-overriding module wrote to disk regardless of
    // whether the app-level transport is the API.
    let transport_mode_str = match ctx.transport_mode {
        crate::transport::TransportMode::Api => "API",
        crate::transport::TransportMode::FileSystem => "FileSystem",
    };

    let captured_at_str = ctx.now_utc.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let body = SubmitResponse {
        vault_path: ctx.vault_path.clone(),
        transport_mode: transport_mode_str,
        auto_created: ctx.auto_created,
        post_create_commands: ctx.post_create_commands,
        history_id: ctx.history_id.clone(),
        captured_at: captured_at_str,
        warnings: ctx.warnings,
    };

    let location = format!("/api/v1/history/{}", ctx.history_id);
    let mut resp = (StatusCode::CREATED, Json(body)).into_response();
    if let Ok(loc) = HeaderValue::from_str(&location) {
        resp.headers_mut().insert(header::LOCATION, loc);
    }
    resp
}
