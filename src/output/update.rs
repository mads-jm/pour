//! `update` mode — merge frontmatter keys into an existing note.
//!
//! The third write mode, and the first that edits bytes the user already owns.
//! Everything here is built around one rule from spec §2: **pour never
//! re-emits the note.** It reads the current frontmatter to compute new values,
//! then asks the transport to patch exactly the keys the module names. The
//! body, the key order, the quoting style, and every key the module does not
//! mention are all untouched.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use chrono::DateTime;

use crate::config::{FieldConfig, FieldType, ModuleConfig, WriteMode};
use crate::output::frontmatter::{FrontmatterValue, format_number, read_frontmatter};
use crate::output::{apply_wikilink, template};
use crate::transport::fs::FsWriter;
use crate::transport::{Transport, TransportMode, TransportPatchError, TransportReadError};
use crate::visibility::visible_field_indices;

/// Obsidian command fired when the target note does not exist yet (spec §2.3).
///
/// Fixed, not configurable: a config key for it was explicitly out of scope for
/// v1, and this is the command ID of the core Daily notes plugin's "Open
/// today's daily note", which is what makes the template run. If a user's
/// periodic notes come from somewhere else, the capture still fails loudly with
/// an actionable message rather than fabricating a note.
const NOTE_CREATE_COMMAND: &str = "daily-notes";

/// Grace period between firing the note-creation command and re-reading.
///
/// `POST /commands/` returns as soon as Obsidian has dispatched the command,
/// not once the template has finished writing the file. One short wait is the
/// whole retry budget — §2.3 allows exactly one retry.
const NOTE_CREATE_SETTLE: Duration = Duration::from_millis(500);

// ── Value tokens ─────────────────────────────────────────────────────────────

/// What a `counter` token asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CounterOp {
    /// `16` — add to the note's current value.
    Increment(f64),
    /// `=160` — overwrite it. The fat-finger correction.
    Set(f64),
}

/// A `toggle` or `counter` value token that could not be understood.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueParseError {
    /// A `counter` token that is neither `N` nor `=N`.
    NotANumber(String),
    /// A `toggle` token that is not a recognised boolean word.
    NotABoolean(String),
}

impl std::fmt::Display for ValueParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueParseError::NotANumber(raw) => write!(
                f,
                "{raw:?} is not a number — use `16` to add or `=16` to set"
            ),
            ValueParseError::NotABoolean(raw) => {
                write!(
                    f,
                    "{raw:?} is not true/false — use `false` or `off` to clear"
                )
            }
        }
    }
}

impl std::error::Error for ValueParseError {}

/// Parse a `counter` value token: `16` increments, `=16` sets.
pub fn parse_counter_token(raw: &str) -> Result<CounterOp, ValueParseError> {
    let trimmed = raw.trim();
    let (is_set, digits) = match trimmed.strip_prefix('=') {
        Some(rest) => (true, rest.trim()),
        None => (false, trimmed),
    };
    let n: f64 = digits
        .parse()
        .map_err(|_| ValueParseError::NotANumber(raw.to_string()))?;
    if !n.is_finite() {
        return Err(ValueParseError::NotANumber(raw.to_string()));
    }
    Ok(if is_set {
        CounterOp::Set(n)
    } else {
        CounterOp::Increment(n)
    })
}

/// Parse a `toggle` value token.
///
/// `false`/`off`/`no`/`0` clear it; `true`/`on`/`yes`/`1` set it. Spec §5 rules
/// out a `--off` flag deliberately — the value token *is* the correction path.
pub fn parse_toggle_token(raw: &str) -> Result<bool, ValueParseError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Ok(true),
        "false" | "off" | "no" | "0" => Ok(false),
        _ => Err(ValueParseError::NotABoolean(raw.to_string())),
    }
}

// ── Outcome ──────────────────────────────────────────────────────────────────

/// What an `update` write did, in the shape the caller needs to report it.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateOutcome {
    /// Resolved vault-relative path of the note that was patched.
    pub vault_path: String,
    /// Per-field resulting state in module field order, e.g. `water: 64/96 oz`.
    /// Simultaneously the confirmation, the correction prompt, and the
    /// progress display (spec §3.2).
    pub echoes: Vec<String>,
    /// Keys that were absent from the note's frontmatter and had to be
    /// inserted — the user's template is stale (spec §2.4).
    pub inserted_keys: Vec<String>,
    /// `true` when the API could not serve the patch and it went to disk
    /// instead (spec §2.1).
    pub degraded: bool,
}

impl UpdateOutcome {
    /// The one-shot confirmation line: `water: 64/96 oz · ✓ 20260805.md`.
    pub fn echo_line(&self) -> String {
        let file = self
            .vault_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.vault_path);
        format!("{} · ✓ {file}", self.echoes.join(" · "))
    }

    /// Advisory notices worth surfacing after an otherwise successful capture.
    pub fn notices(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.inserted_keys.is_empty() {
            out.push(format!(
                "added missing frontmatter {} {} to {} — your template is stale",
                if self.inserted_keys.len() == 1 {
                    "key"
                } else {
                    "keys"
                },
                self.inserted_keys.join(", "),
                self.vault_path
            ));
        }
        if self.degraded {
            out.push(
                "the Obsidian API could not patch frontmatter; wrote to the file directly"
                    .to_string(),
            );
        }
        out
    }
}

// ── The write ────────────────────────────────────────────────────────────────

/// Execute an **update** write: patch the module's fields into the frontmatter
/// of the existing note at the resolved `path`.
///
/// `fallback_root` is the filesystem root this module's notes live under —
/// `module.base_path` if it overrides, otherwise the vault's. It exists because
/// `Transport::Api` carries no vault path of its own, so the API path has no
/// `FsWriter` to degrade *to* without being handed one (spec §2.1).
///
/// `now` is passed explicitly so `captured_at`-derived timestamps from the
/// server thread through, exactly as `write_create`/`write_append` do.
pub async fn write_update(
    transport: &Transport,
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
    date_format: Option<&str>,
    fallback_root: &str,
    now: DateTime<chrono::Local>,
) -> Result<UpdateOutcome> {
    if module.mode != WriteMode::Update {
        bail!("write_update called on a non-update module");
    }

    let vault_path = template::render_path(&module.path, field_values, date_format, now);
    let fallback = FsWriter::new(PathBuf::from(fallback_root));

    let content = read_note(transport, &fallback, &vault_path).await?;
    let current = read_frontmatter(&content).ok_or_else(|| {
        anyhow!(
            "{vault_path} has no frontmatter block — add the properties in Obsidian first \
             (pour only ever mutates existing properties, it never restructures a note)"
        )
    })?;

    let plan = plan_updates(module, field_values, &current)?;
    if plan.is_empty() {
        bail!("nothing to update — no field had a value");
    }

    let mut outcome = UpdateOutcome {
        vault_path: vault_path.clone(),
        echoes: Vec::new(),
        inserted_keys: Vec::new(),
        degraded: false,
    };

    // Sequential, one key at a time: the transport's patch operation is
    // single-key on both backends (the API's PATCH names exactly one `Target`),
    // so a multi-field submission is *not* atomic across keys. Every value-level
    // failure is raised by `plan_updates` above, before a single byte is
    // written, which leaves only transport failures — a concurrent Obsidian save
    // landing between two patches, or I/O — able to strike mid-loop. When one
    // does, earlier keys have already landed and nothing rolls them back, so the
    // error says exactly which ones did rather than reporting a half-applied
    // capture as a clean failure.
    let mut applied: Vec<String> = Vec::new();
    for (key, value, echo) in plan {
        if !current.contains_key(&key) {
            outcome.inserted_keys.push(key.clone());
        }

        let mut result = transport.patch_frontmatter(&vault_path, &key, &value).await;
        if let Err(TransportPatchError::Unsupported(reason)) = &result {
            // §2.1 — a plugin too old (or an API gone quiet) degrades the
            // capture to the filesystem instead of failing it. The invariant
            // "the fs fallback always works" is the whole portability story.
            tracing::debug!(key = %key, reason = %reason, "degrading frontmatter patch to filesystem");
            outcome.degraded = true;
            result = fallback.patch_frontmatter(&vault_path, &key, &value);
        }
        if let Err(e) = result {
            return Err(anyhow!(partial_write_message(
                &vault_path,
                &key,
                &applied,
                &e
            )));
        }

        applied.push(key);
        outcome.echoes.push(echo);
    }

    Ok(outcome)
}

/// The error for a key that failed to write, naming the keys that already did.
///
/// The note is a shared artifact: telling the user "the capture failed" when
/// half of it is on disk sends them to re-run a command that would double-count
/// the keys that landed.
fn partial_write_message(
    vault_path: &str,
    key: &str,
    applied: &[String],
    err: &TransportPatchError,
) -> String {
    let mut msg = format!("failed to set '{key}' in {vault_path}: {err}");
    if !applied.is_empty() {
        msg.push_str(&format!(
            " — {} already written to the note and {} not rolled back",
            applied
                .iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(", "),
            if applied.len() == 1 { "was" } else { "were" },
        ));
    }
    msg
}

/// Read the target note, honouring §2.3 (missing note) and degrading the read
/// to the filesystem when the API has gone away.
async fn read_note(transport: &Transport, fallback: &FsWriter, vault_path: &str) -> Result<String> {
    match transport.read_file(vault_path).await {
        Ok(content) => Ok(content),
        Err(TransportReadError::NotFound) => {
            if transport.mode() == TransportMode::Api {
                // Ask Obsidian to create today's note *with its template*, then
                // retry exactly once. Best-effort: if the command fails, the
                // retry simply fails too and the user gets the same actionable
                // message they would have without it.
                let _ = transport.execute_command(NOTE_CREATE_COMMAND).await;
                tokio::time::sleep(NOTE_CREATE_SETTLE).await;
                if let Ok(content) = transport.read_file(vault_path).await {
                    return Ok(content);
                }
            }
            Err(anyhow!(missing_note_message(vault_path)))
        }
        Err(TransportReadError::Unreachable(_)) => match fallback.read_file(vault_path) {
            Ok(content) => Ok(content),
            Err(TransportReadError::NotFound) => Err(anyhow!(missing_note_message(vault_path))),
            Err(e) => Err(anyhow!("{e}")),
        },
        Err(e) => Err(anyhow!("{e}")),
    }
}

fn missing_note_message(vault_path: &str) -> String {
    format!(
        "{vault_path} doesn't exist yet — open Obsidian or create the note first. \
         Pour never creates it: your daily-note template owns its shape."
    )
}

/// Frontmatter mutations to apply: `(key, value, echo)`, in module field order.
type UpdatePlan = Vec<(String, FrontmatterValue, String)>;

/// Turn the submitted field values into the exact set of key mutations.
///
/// Fields hidden by `show_when` and fields left blank are skipped entirely —
/// a blank counter means "no change today", not "set to zero".
fn plan_updates(
    module: &ModuleConfig,
    field_values: &HashMap<String, String>,
    current: &BTreeMap<String, String>,
) -> Result<UpdatePlan> {
    let visible = visible_field_indices(&module.fields, field_values);
    let mut plan: UpdatePlan = Vec::new();

    for &idx in &visible {
        let field = &module.fields[idx];
        let Some(raw) = field_values.get(&field.name) else {
            continue;
        };
        if raw.trim().is_empty() {
            continue;
        }

        // Flattened rather than `.context()`d: callers print `{e}`, and a
        // capture error the user cannot read is a capture error they cannot fix.
        let (value, echo) = compute_value(field, raw, current)
            .map_err(|e| anyhow!("field '{}': {e:#}", field.name))?;
        plan.push((field.name.clone(), value, echo));
    }

    Ok(plan)
}

/// Compute one field's new frontmatter value and its echo string.
fn compute_value(
    field: &FieldConfig,
    raw: &str,
    current: &BTreeMap<String, String>,
) -> Result<(FrontmatterValue, String)> {
    match field.field_type {
        FieldType::Toggle => {
            let flag = parse_toggle_token(raw)?;
            Ok((
                FrontmatterValue::Bool(flag),
                format!("{}: {flag}", field.name),
            ))
        }
        FieldType::Counter => {
            let op = parse_counter_token(raw)?;
            let next = match op {
                CounterOp::Increment(n) => current_number(current, &field.name)? + n,
                CounterOp::Set(n) => n,
            };
            Ok((FrontmatterValue::Number(next), counter_echo(field, next)))
        }
        FieldType::Number => {
            let n: f64 = raw
                .trim()
                .parse()
                .map_err(|_| anyhow!(ValueParseError::NotANumber(raw.to_string()).to_string()))?;
            Ok((
                FrontmatterValue::Number(n),
                format!("{}: {}", field.name, format_number(n)),
            ))
        }
        _ => {
            let value = if field.wikilink == Some(true) {
                apply_wikilink(raw.to_string(), field.list)
            } else {
                raw.to_string()
            };
            let echo = format!("{}: {value}", field.name);
            Ok((FrontmatterValue::Text(value), echo))
        }
    }
}

/// The note's current value for `key`, as a `counter` reads it.
///
/// A missing key, an empty value, and an explicit `null`/`~` all read as `0` —
/// which is what keeps a template's `water: null` ("untouched today")
/// meaningfully distinct from an explicit `0` *in the note* while still being
/// arithmetically usable here.
///
/// A non-numeric value is an error rather than a silent `0`: incrementing it
/// would overwrite whatever the user actually put there.
fn current_number(current: &BTreeMap<String, String>, key: &str) -> Result<f64> {
    let Some(raw) = current.get(key) else {
        return Ok(0.0);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") || trimmed == "~" {
        return Ok(0.0);
    }
    trimmed.parse::<f64>().map_err(|_| {
        anyhow!("frontmatter key '{key}' holds {trimmed:?}, which a counter cannot increment")
    })
}

/// `water: 64/96 oz` — value, optional goal, optional unit.
fn counter_echo(field: &FieldConfig, value: f64) -> String {
    let rendered = match field.goal {
        Some(goal) => format!("{}/{}", format_number(value), format_number(goal)),
        None => format_number(value),
    };
    match field
        .unit
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        Some(unit) => format!("{}: {rendered} {unit}", field.name),
        None => format!("{}: {rendered}", field.name),
    }
}
