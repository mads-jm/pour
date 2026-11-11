//! Transport wrapper that collects a module's prior captures with their
//! frontmatter, mirroring the ADR-001 API→FS fallback.
//!
//! # Two paths, one corpus shape
//!
//! Both paths produce the same `Vec<Capture>` so the resolver (which owns the
//! hard filter, scoring, rank, and summary) behaves identically regardless of
//! transport. This is the "search path + FS-scan fallback parity" the brief
//! requires.
//!
//! - **API up:** list the module directory, then fetch each note as
//!   `application/vnd.olrapi.note+json` to obtain pre-parsed frontmatter and
//!   `stat.mtime`. Obsidian's parser handles the frontmatter, so Pour's own
//!   reader is not exercised here.
//! - **API down:** filesystem scan of the module directory; read each `.md`
//!   file and parse its frontmatter with [`parse_frontmatter`], using the file
//!   mtime as the recency key.
//!
//! # Why corpus-fetch rather than `/search/` filtering
//!
//! The resolver drops priors that fail the `show_when` gate filter, then
//! scores the rest by how many `match_on` fields agree with the form (§4.1).
//! A server-side `/search/` query can express the exact-match filter but not
//! the scoring, so the client would still have to fetch and rank everything
//! that passes. Fetching the module's corpus once and resolving in-process
//! keeps the two transport paths equivalent and keeps all match and rank logic
//! in one unit-testable place. The injection-safe JsonLogic builder
//! (`super::jsonlogic`) is kept and tested as the query a future server-side
//! filter would send (see [`build_tier_predicate`]), so pushing the gate
//! filter to the server stays a local change.
//!
//! A conservative bound is applied: the corpus fetch stops after
//! `MAX_CORPUS` notes so a large module directory cannot stall form-open.

use std::collections::HashMap;

use serde_json::Value;

use crate::data::frontmatter_read::{Frontmatter, FrontmatterValue, parse_frontmatter};
use crate::transport::{Transport, TransportReadError};

use super::jsonlogic::build_predicate;
use super::plan::{MatchKey, PriorsPlan};
use super::resolver::Capture;

/// Upper bound on how many notes the corpus fetch reads per resolve. Keeps
/// form-open responsive on large module directories; the resolver only shows
/// `limit` columns anyway (default 3).
const MAX_CORPUS: usize = 500;

/// Fetch the module's prior captures from the given vault-relative directory.
///
/// Returns an empty vec (never an error) when the directory is missing or the
/// transport is unavailable — the panel then shows its empty state rather than
/// surfacing an error mid-capture.
pub async fn fetch_corpus(transport: &Transport, module_dir: &str) -> Vec<Capture> {
    // List candidate note stems in the module directory.
    let entries = match transport.list_directory_entries(module_dir).await {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut captures = Vec::new();
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        if captures.len() >= MAX_CORPUS {
            break;
        }
        let vault_path = join_vault_path(module_dir, &entry.name);
        if let Some(cap) = fetch_one(transport, &vault_path).await {
            captures.push(cap);
        }
    }

    captures
}

/// Fetch a single capture. On the API path this uses note+json (Obsidian parses
/// the frontmatter); on the FS path it reads the file and parses frontmatter
/// via Pour's reader.
async fn fetch_one(transport: &Transport, vault_path: &str) -> Option<Capture> {
    match transport {
        Transport::Api(client) => {
            let note = client.read_note_json(vault_path).await.ok()?;
            Some(Capture {
                frontmatter: frontmatter_from_json(&note.frontmatter),
                recency: note.stat.mtime,
                path: vault_path.to_string(),
            })
        }
        Transport::Fs(writer) => match writer.read_file_with_mtime(vault_path) {
            Ok((content, mtime)) => Some(Capture {
                frontmatter: parse_frontmatter(&content),
                recency: mtime,
                path: vault_path.to_string(),
            }),
            Err(TransportReadError::NotFound) => None,
            Err(_) => None,
        },
    }
}

/// Convert an Obsidian note+json frontmatter object into Pour's `Frontmatter`
/// shape, so both transport paths hand the resolver the same value type.
///
/// Scalars (string/number/bool) become `Scalar`; arrays become `List`; nested
/// objects/nulls are skipped (out of L1's frontmatter subset).
fn frontmatter_from_json(map: &serde_json::Map<String, Value>) -> Frontmatter {
    let mut fm = Frontmatter::new();
    for (key, value) in map {
        match value {
            Value::String(s) => {
                fm.insert(key.clone(), FrontmatterValue::Scalar(s.clone()));
            }
            Value::Number(n) => {
                fm.insert(key.clone(), FrontmatterValue::Scalar(n.to_string()));
            }
            Value::Bool(b) => {
                fm.insert(key.clone(), FrontmatterValue::Scalar(b.to_string()));
            }
            Value::Array(items) => {
                let strs: Vec<String> = items
                    .iter()
                    .filter_map(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        Value::Number(n) => Some(n.to_string()),
                        Value::Bool(b) => Some(b.to_string()),
                        _ => None,
                    })
                    .collect();
                fm.insert(key.clone(), FrontmatterValue::List(strs));
            }
            // null / object → not part of the L1 subset; skip.
            _ => {}
        }
    }
    fm
}

/// Build the JsonLogic predicate that requires the first `tier_len` match
/// keys to equal the current form values.
///
/// No caller uses it at runtime. The resolver filters and scores in-process
/// (see module docs). It is the tested, injection-safe `/search/` query a
/// future server-side filter would send. Returns `None` when any of those keys
/// lacks a value, since the query can't be built.
pub fn build_tier_predicate(
    plan: &PriorsPlan,
    tier_len: usize,
    match_values: &HashMap<String, String>,
) -> Option<Value> {
    let keys = plan.match_keys.get(..tier_len)?;
    if keys.is_empty() {
        return None;
    }

    let mut pairs: Vec<(&MatchKey, &str)> = Vec::with_capacity(keys.len());
    for key in keys {
        let raw = match_values.get(&key.field)?;
        let value = raw.trim();
        if value.is_empty() {
            return None;
        }
        // For wikilink keys, the target is the stripped form; the builder emits
        // both bare and `[[wrapped]]` comparisons.
        pairs.push((key, value));
    }

    Some(build_predicate(&pairs))
}

/// Join a vault directory and a note stem into a vault-relative `.md` path.
fn join_vault_path(dir: &str, stem: &str) -> String {
    let dir = dir.trim_end_matches('/');
    if dir.is_empty() {
        format!("{stem}.md")
    } else {
        format!("{dir}/{stem}.md")
    }
}
