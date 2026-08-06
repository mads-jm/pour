//! One-shot argv capture — `pour <module> <field> [value]` (spec §5).
//!
//! The velocity ethos made literal: the whole interaction is one shell line, no
//! TUI, echo the result, exit. This module owns the *grammar* (which argument
//! means what, and what tokens each field type accepts); the write itself is
//! [`crate::output::write_update`], the same path the TUI and the PWA use.
//!
//! It lives in the library rather than `main.rs` so the parse layer is
//! testable: `tests/` compiles against the lib, not the binary.
//!
//! Two deliberate limits for v1:
//!
//! - **`toggle`/`counter` fields only.** A text-bearing one-shot
//!   (`pour me "thought"`) raises quoting and required-field questions the spec
//!   defers to its own pass.
//! - **`update` modules only.** Everything else needs a full form's worth of
//!   values; claiming otherwise would produce half-empty notes.

use anyhow::Result;

use crate::config::{Config, FieldType, WriteMode};
use crate::output::update::{parse_counter_token, parse_toggle_token};
use crate::transport::Transport;

/// A resolved one-shot invocation, ready to write.
#[derive(Debug, Clone, PartialEq)]
pub struct OneShot {
    pub module_key: String,
    pub field_name: String,
    /// The value token, already normalised and validated for the field's type —
    /// an absent `toggle` value has become `"true"` by this point.
    pub value: String,
}

/// Why a one-shot invocation could not be resolved. Every variant is a user
/// error worth a non-zero exit and an actionable message.
#[derive(Debug, Clone, PartialEq)]
pub enum OneShotError {
    UnknownModule {
        module: String,
        available: Vec<String>,
    },
    UnknownField {
        module: String,
        field: String,
        available: Vec<String>,
    },
    /// The module exists but does not mutate an existing note.
    NotUpdateMode { module: String },
    /// The field exists but its type is not wired for one-shot capture in v1.
    UnsupportedFieldType {
        field: String,
        field_type: &'static str,
    },
    /// A `counter` invoked with no value — there is no sensible default delta.
    MissingValue { field: String },
    /// The value token did not parse for the field's type.
    BadValue { field: String, reason: String },
    /// Arguments past `[value]`. Rejected rather than ignored so the grammar
    /// stays free to grow (spec §4.2's `--date` lands here in v1.1).
    UnexpectedArgs { extra: Vec<String> },
}

impl std::fmt::Display for OneShotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OneShotError::UnknownModule { module, available } => write!(
                f,
                "unknown module '{module}'\navailable modules: {}",
                available.join(", ")
            ),
            OneShotError::UnknownField {
                module,
                field,
                available,
            } => write!(
                f,
                "module '{module}' has no field '{field}'\navailable fields: {}",
                available.join(", ")
            ),
            OneShotError::NotUpdateMode { module } => write!(
                f,
                "module '{module}' is not an update-mode module — one-shot capture \
                 only mutates existing notes; run `pour {module}` for the form"
            ),
            OneShotError::UnsupportedFieldType { field, field_type } => write!(
                f,
                "field '{field}' is a {field_type} — one-shot capture is wired for \
                 toggle and counter fields only"
            ),
            OneShotError::MissingValue { field } => write!(
                f,
                "field '{field}' is a counter and needs a value: `{field} 16` to add, \
                 `{field} =16` to set"
            ),
            OneShotError::BadValue { field, reason } => write!(f, "field '{field}': {reason}"),
            OneShotError::UnexpectedArgs { extra } => write!(
                f,
                "unexpected argument{} after the value: {}",
                if extra.len() == 1 { "" } else { "s" },
                extra.join(" ")
            ),
        }
    }
}

impl std::error::Error for OneShotError {}

/// Resolve `args` against the config.
///
/// `args` is the raw `std::env::args()` vector, `args[0]` included.
///
/// Returns `Ok(None)` when there is no field argument — `pour <module>` keeps
/// meaning exactly what it means today, which is the one hard compatibility
/// constraint on this grammar.
pub fn parse(args: &[String], config: &Config) -> Result<Option<OneShot>, OneShotError> {
    let (Some(module_key), Some(field_name)) = (args.get(1), args.get(2)) else {
        return Ok(None);
    };

    if args.len() > 4 {
        return Err(OneShotError::UnexpectedArgs {
            extra: args[4..].to_vec(),
        });
    }

    let Some(module) = config.modules.get(module_key) else {
        let mut available: Vec<String> = config.modules.keys().cloned().collect();
        available.sort();
        return Err(OneShotError::UnknownModule {
            module: module_key.clone(),
            available,
        });
    };

    let Some(field) = module.fields.iter().find(|f| &f.name == field_name) else {
        return Err(OneShotError::UnknownField {
            module: module_key.clone(),
            field: field_name.clone(),
            available: module.fields.iter().map(|f| f.name.clone()).collect(),
        });
    };

    if module.mode != WriteMode::Update {
        return Err(OneShotError::NotUpdateMode {
            module: module_key.clone(),
        });
    }

    let raw = args.get(3).map(String::as_str);

    let value = match field.field_type {
        // A bare field name means "yes, it happened" — the common case gets
        // the shortest command. `false`/`off` is the correction path.
        FieldType::Toggle => {
            let token = raw.unwrap_or("true");
            let flag = parse_toggle_token(token).map_err(|e| OneShotError::BadValue {
                field: field_name.clone(),
                reason: e.to_string(),
            })?;
            flag.to_string()
        }
        FieldType::Counter => {
            let token = raw.ok_or_else(|| OneShotError::MissingValue {
                field: field_name.clone(),
            })?;
            // Parsed here purely to fail before touching the vault; the write
            // path re-parses the same token.
            parse_counter_token(token).map_err(|e| OneShotError::BadValue {
                field: field_name.clone(),
                reason: e.to_string(),
            })?;
            token.trim().to_string()
        }
        ref other => {
            return Err(OneShotError::UnsupportedFieldType {
                field: field_name.clone(),
                field_type: crate::server::dto::mapping::field_type_wire(other),
            });
        }
    };

    Ok(Some(OneShot {
        module_key: module_key.clone(),
        field_name: field_name.clone(),
        value,
    }))
}

/// Execute a resolved one-shot and return the line to print.
///
/// `water: 64/96 oz · ✓ 20260805.md` — confirmation, correction prompt, and
/// progress display in one line (spec §3.2). Any §2.4 stale-template notice is
/// appended on its own line.
pub async fn run(config: &Config, transport: &Transport, shot: &OneShot) -> Result<String> {
    let module = config
        .modules
        .get(&shot.module_key)
        .ok_or_else(|| anyhow::anyhow!("unknown module '{}'", shot.module_key))?;

    // A root-overriding module writes through its own filesystem transport,
    // exactly as the TUI and server write paths do.
    let module_transport = Transport::for_module(module);
    let transport = module_transport.as_ref().unwrap_or(transport);

    let root = module
        .root_override()
        .unwrap_or(config.vault.effective_base_path())
        .to_string();

    let field_values =
        std::collections::HashMap::from([(shot.field_name.clone(), shot.value.clone())]);

    let outcome = crate::output::write_update(
        transport,
        module,
        &field_values,
        config.vault.date_format.as_deref(),
        &root,
        chrono::Local::now(),
    )
    .await?;

    let mut out = outcome.echo_line();
    for notice in outcome.notices() {
        out.push('\n');
        out.push_str(&notice);
    }
    Ok(out)
}
