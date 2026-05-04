/// Step 5 — History recording (non-fatal).
///
/// Derives `first_field` from the visible, non-textarea, non-composite fields
/// and calls `History::record`. On failure a warning is pushed and a fallback
/// `history_id` is synthesized — the submit still returns 201.
///
/// Populates `ctx.history_id`.
use super::SubmitContext;
use crate::config::FieldType;
use crate::data::history::History;
use crate::server::dto::SubmitWarningDto;

/// Record the submission in history and set `ctx.history_id`.
///
/// This step is infallible from the caller's perspective: on `History::record`
/// failure a warning is emitted and a synthetic id is used.
pub(super) fn run(ctx: &mut SubmitContext) {
    // Derive first_field: first non-textarea, non-composite visible field value.
    let module_key = &ctx.module_key;

    // We need to look up the field list from the config reference. Since we
    // only need to iterate field metadata (not write state), borrow through the
    // Arc-owned config is fine here.
    //
    // The module has already been validated; unwrap is safe.
    let first_field_value = first_non_textarea_field(ctx);

    let history_id = {
        let mut history = History::load();
        match history.record(
            module_key,
            &ctx.vault_path,
            first_field_value.as_deref(),
            ctx.now_utc,
        ) {
            Ok(id) => id,
            Err(e) => {
                ctx.warnings.push(SubmitWarningDto {
                    code: "history_record_failed",
                    field: None,
                    message: format!("history record failed: {e}"),
                });
                // Synthesize a stable fallback id (rare path — only when History::record fails).
                format!(
                    "{}-fallback-{}",
                    ctx.now_utc.format("%Y%m%dT%H%M%S%3f"),
                    module_key
                )
            }
        }
    };

    ctx.history_id = history_id;
}

/// Find the first visible, non-textarea, non-composite field value for
/// history summary.
fn first_non_textarea_field(ctx: &SubmitContext) -> Option<String> {
    // field_order is stored on SubmitContext so we don't need to re-borrow
    // state. It was captured during the validate step.
    ctx.field_order
        .iter()
        .filter(|(name, ft)| {
            ctx.visible_names.contains(name.as_str())
                && *ft != FieldType::Textarea
                && *ft != FieldType::CompositeArray
        })
        .find_map(|(name, _)| ctx.field_values.get(name).cloned())
}
