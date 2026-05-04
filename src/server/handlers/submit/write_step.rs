/// Step 4 — Main write (`write_create` / `write_append`).
///
/// Dispatches to the appropriate output function based on the module's
/// `WriteMode`. On success, populates `ctx.vault_path`. On failure, returns
/// a ready `Response` with a 500 `write_error` envelope.
use axum::http::StatusCode;
use axum::response::Response;
use serde_json::json;

use super::SubmitContext;
use crate::config::WriteMode;
use crate::server::{
    AppState,
    dto::{error_codes, error_response_with_details},
};

/// Execute the main write and store the resulting vault path in `ctx`.
pub(super) async fn run(ctx: &mut SubmitContext, state: &AppState) -> Result<(), Response> {
    let module = state
        .config
        .modules
        .get(&ctx.module_key)
        .expect("module already validated in validate step");

    let date_fmt = state.config.vault.date_format.as_deref();

    let write_result = match module.mode {
        WriteMode::Create => {
            crate::output::write_create(
                &state.transport,
                module,
                &ctx.field_values,
                &ctx.composite_data,
                date_fmt,
                &ctx.callout_overrides,
                &ctx.callout_titles,
                ctx.now_local,
            )
            .await
        }
        WriteMode::Append => {
            crate::output::write_append(
                &state.transport,
                module,
                &ctx.field_values,
                &ctx.composite_data,
                date_fmt,
                &ctx.callout_overrides,
                &ctx.callout_titles,
                ctx.now_local,
            )
            .await
        }
    };

    match write_result {
        Ok(path) => {
            ctx.vault_path = path;
            Ok(())
        }
        Err(e) => {
            tracing::warn!(
                module = %ctx.module_key,
                code = %error_codes::WRITE_ERROR,
                "submit failed"
            );
            Err(error_response_with_details(
                StatusCode::INTERNAL_SERVER_ERROR,
                error_codes::WRITE_ERROR,
                "Engine write failed.",
                json!({ "engine_error": e.to_string() }),
            ))
        }
    }
}
