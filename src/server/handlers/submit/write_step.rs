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
    dto::{SubmitWarningDto, error_codes, error_response_with_details},
};
use crate::transport::Transport;

/// Execute the main write and store the resulting vault path in `ctx`.
pub(super) async fn run(ctx: &mut SubmitContext, state: &AppState) -> Result<(), Response> {
    let module = state
        .config
        .modules
        .get(&ctx.module_key)
        .expect("module already validated in validate step");

    let date_fmt = state.config.vault.date_format.as_deref();

    // A root-overriding module writes through its own filesystem transport.
    // `ctx.transport_mode` follows the transport actually used, so the 201
    // response cannot claim "API" for a write the API never saw.
    let module_transport = Transport::for_module(module);
    let transport = module_transport.as_ref().unwrap_or(&state.transport);
    ctx.transport_mode = transport.mode();

    let write_result = match module.mode {
        WriteMode::Create => {
            crate::output::write_create(
                transport,
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
                transport,
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
        // An update module is a module like any other to the server (spec §5).
        // The §2.4 stale-template notice rides back on the 201 as a warning,
        // matching every other best-effort signal in this pipeline.
        WriteMode::Update => {
            let root = module
                .root_override()
                .unwrap_or(state.config.vault.effective_base_path())
                .to_string();
            crate::output::write_update(
                transport,
                module,
                &ctx.field_values,
                date_fmt,
                &root,
                ctx.now_local,
            )
            .await
            .map(|outcome| {
                for notice in outcome.notices() {
                    ctx.warnings.push(SubmitWarningDto {
                        code: "frontmatter_update_notice",
                        field: None,
                        message: notice,
                    });
                }
                outcome.vault_path
            })
        }
    };

    match write_result {
        Ok(path) => {
            ctx.vault_path = path;
            run_hook(ctx, state, module).await;
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

/// Fire `post_write_shell` for a LAN-submitted capture — only if the module
/// opted in via `post_write_shell_on_serve`.
///
/// Default-off is the point: a capture arriving over the network must not run a
/// command just because someone at the keyboard would have. Failures become
/// warnings on the 201, matching every other best-effort step in this pipeline.
async fn run_hook(ctx: &mut SubmitContext, state: &AppState, module: &crate::config::ModuleConfig) {
    let Some(command) = &module.post_write_shell else {
        return;
    };
    if !module.hook_fires_on_serve() {
        return;
    }

    let root = module
        .root_override()
        .unwrap_or(state.config.vault.effective_base_path());
    let title = ctx
        .field_values
        .get("title")
        .map(String::as_str)
        .unwrap_or_default();

    let hook_ctx = crate::hooks::HookContext::new(root, &ctx.vault_path, title, ctx.now_local);

    if let Some(message) = crate::hooks::run(command, &hook_ctx).await {
        tracing::warn!(module = %ctx.module_key, "post_write_shell failed");
        ctx.warnings.push(SubmitWarningDto {
            code: "post_write_shell_failed",
            field: None,
            message,
        });
    }
}
