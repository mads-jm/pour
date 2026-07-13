//! Priors review panel (L1, coffee/TUI): a read-only, config-declared surface
//! that shows the best-relevant prior captures beside the capture form.
//!
//! The panel is **read-only**: it never writes, never blocks submit, and only
//! resolves at form-open and on `match_on`-field change (§2). This module is
//! decoupled from the TUI — the resolver, JsonLogic builder, plan derivation,
//! and frontmatter foundation are all unit-testable without a terminal.
//!
//! Layering:
//! - [`plan`] — derive a resolver-ready plan from `[priors]` config (or the
//!   zero-config default).
//! - [`jsonlogic`] — injection-safe `/search/` predicate builder.
//! - [`search`] — transport wrapper collecting the module's captures (API→FS).
//! - [`resolver`] — cascade match, qualifying-first ranking, numeric summary.

pub mod jsonlogic;
pub mod plan;
pub mod resolver;
pub mod search;

use std::collections::HashMap;

use crate::config::ModuleConfig;
use crate::transport::Transport;

pub use plan::PriorsPlan;
pub use resolver::{Capture, PanelRow, ResolvedPanel, Summary, resolve};

/// Resolve the priors panel for a module against the current form values.
///
/// Fetches the module's corpus via the transport (API `/search/`-family or FS
/// scan) and runs the pure resolver. Returns `None` for the empty state.
///
/// This is the single entry point the TUI calls at form-open and on
/// `match_on`-field change.
pub async fn resolve_panel(
    transport: &Transport,
    module: &ModuleConfig,
    match_values: &HashMap<String, String>,
) -> Option<ResolvedPanel> {
    let plan = PriorsPlan::build(module);
    let module_dir = module_directory(&module.path);
    let corpus = search::fetch_corpus(transport, &module_dir).await;
    resolve(&plan, &corpus, match_values)
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
