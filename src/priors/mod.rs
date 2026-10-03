//! Priors review panel (L1.5, TUI): a read-only, config-declared surface that
//! shows the most similar prior captures beside the capture form, one column
//! per prior, rows aligned to the form's fields.
//!
//! The panel is **read-only**: it never writes, never blocks submit, and only
//! resolves at form-open and when a `match_on` or gate field changes (§2). This module is
//! decoupled from the TUI — the resolver, JsonLogic builder, plan derivation,
//! and frontmatter foundation are all unit-testable without a terminal.
//!
//! Layering:
//! - [`plan`] — derive a resolver-ready plan from `[priors]` config (or the
//!   zero-config default).
//! - [`jsonlogic`] — injection-safe `/search/` predicate builder.
//! - [`search`] — transport wrapper collecting the module's captures (API→FS).
//! - [`resolver`] — `show_when` hard filter, similarity score, ordering,
//!   opt-in summary column.

pub mod jsonlogic;
pub mod plan;
pub mod resolver;
pub mod search;

use std::collections::HashMap;

use crate::config::ModuleConfig;
use crate::transport::Transport;

pub use plan::PriorsPlan;
pub use resolver::{
    Capture, PriorColumn, ResolvedPanel, SummaryColumn, age_label, resolve, same_as_current,
};

/// Resolve the priors panel for a module against the current form values.
///
/// Fetches the module's corpus via the transport (API `/search/`-family or FS
/// scan) and runs the pure resolver. A panel with no columns is the empty
/// state.
///
/// This is the single entry point the TUI calls at form-open and when a
/// `match_on` or gate field changes.
pub async fn resolve_panel(
    transport: &Transport,
    module: &ModuleConfig,
    form_values: &HashMap<String, String>,
) -> ResolvedPanel {
    let plan = PriorsPlan::build(module);
    let module_dir = module_directory(&module.path);
    let corpus = search::fetch_corpus(transport, &module_dir).await;
    resolve(&plan, &corpus, &visible_form_values(module, form_values))
}

/// The form values the resolver may compare: only those of fields
/// `show_when` currently shows.
///
/// The form keeps a field's value after `show_when` hides it, so switching
/// the brew method from Pour Over to Espresso leaves a stale `brewer` behind.
/// Submit never writes hidden fields (`output::partition_fields` filters
/// through the same [`crate::visibility::visible_field_indices`]), and the
/// resolver follows the same rule: a hidden value neither scores, nor gates,
/// nor turns the header into `no close match`.
pub fn visible_form_values(
    module: &ModuleConfig,
    form_values: &HashMap<String, String>,
) -> HashMap<String, String> {
    crate::visibility::visible_field_indices(&module.fields, form_values)
        .into_iter()
        .filter_map(|i| {
            let name = &module.fields[i].name;
            form_values
                .get(name)
                .map(|value| (name.clone(), value.clone()))
        })
        .collect()
}

/// The raw form values of the plan's trigger fields (`match_on` plus gates),
/// in [`PriorsPlan::trigger_fields`] order, with `""` for a field that has no
/// value.
///
/// The TUI snapshots this before and after each key and re-resolves only when
/// it changed. It reads the raw values, hidden fields included, so a gate
/// change always re-resolves; [`resolve_panel`] then drops whatever the new
/// gate hides. A field outside the triggers (`dose_g`) leaves the snapshot
/// unchanged: it only moves `·` marks, which the renderer recomputes every
/// frame.
pub fn trigger_values(module: &ModuleConfig, form_values: &HashMap<String, String>) -> Vec<String> {
    PriorsPlan::build(module)
        .trigger_fields()
        .into_iter()
        .map(|f| form_values.get(f).cloned().unwrap_or_default())
        .collect()
}

/// Derive the vault directory that holds a module's captures from its `path`
/// template. The `path` may contain date/field tokens and a filename; the
/// directory is everything up to the last `/`.
pub fn module_directory(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(pos) => trimmed[..pos].to_string(),
        None => String::new(),
    }
}
