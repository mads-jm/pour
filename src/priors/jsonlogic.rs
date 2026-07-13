//! Injection-safe JsonLogic predicate builder for the `/search/` fast path.
//!
//! # Injection safety (load-bearing)
//!
//! `match_on` values are user-controlled (roaster/bean names, picked in the
//! form). They ride as JSON **data** inside a `serde_json::Value` tree — they
//! are NEVER interpolated into a query string. The tree is handed to
//! `reqwest`'s JSON body serializer, which escapes them as string literals in
//! the request body. There is no code path where a user value is concatenated
//! into a query. The builder test asserts this: a value containing JSON/DQL
//! metacharacters appears only as a JSON string node, never as a structural
//! part of the tree.
//!
//! # Accessor shape (Architect decision #2 — CONFIRMED)
//!
//! The Obsidian Local REST API's OpenAPI spec
//! (`obsidian-local-rest-api-openapi.yaml`, `/search/` examples) documents the
//! frontmatter accessor as `{"var": "frontmatter.<key>"}` with `{"==": [...]}`
//! for equality. This is confirmed from that authoritative source, so it is not
//! guessed. The single function [`match_var`] centralises the accessor shape:
//! if a future API revision changes it, that one function is the only edit.

use serde_json::{Value, json};

use super::plan::{MatchKey, MatchMode};

/// Build the `{"var": "frontmatter.<field>"}` accessor node for a field.
///
/// Centralised so the accessor shape lives in exactly one place (see module
/// docs, Architect decision #2).
fn match_var(field: &str) -> Value {
    json!({ "var": format!("frontmatter.{field}") })
}

/// Build a JsonLogic predicate tree for a conjunction of match keys against
/// their resolved (target) values.
///
/// `pairs` is `(key, comparison_value)`; the comparison value is the *target*
/// the resolver keys on (already wikilink-stripped for wikilink-mode keys, so
/// the transport can compare against `frontmatter.<field>` — see the FS-scan
/// fallback which strips the stored side symmetrically).
///
/// - Equality keys emit `{"==": [{"var": "frontmatter.<field>"}, <value>]}`.
/// - Wikilink keys emit an `or` over the bare target and the `[[target]]`
///   wrapped form, because Obsidian's `/search/` compares the *raw* stored
///   frontmatter string, which is typically `"[[Onyx]]"` rather than `"Onyx"`.
///   This keeps the API path equivalent to the FS-scan path (which strips the
///   stored side before comparing).
///
/// An empty `pairs` slice yields `true` (match everything) — callers guard
/// against that, but a total function is safer than a panic.
pub fn build_predicate(pairs: &[(&MatchKey, &str)]) -> Value {
    if pairs.is_empty() {
        return Value::Bool(true);
    }

    let clauses: Vec<Value> = pairs
        .iter()
        .map(|(key, value)| clause_for(key, value))
        .collect();

    json!({ "and": clauses })
}

fn clause_for(key: &MatchKey, value: &str) -> Value {
    match key.mode {
        MatchMode::Equality => {
            json!({ "==": [match_var(&key.field), value] })
        }
        MatchMode::Wikilink => {
            // The stored value may be the bare target or a `[[target]]` wrap.
            let var = match_var(&key.field);
            let wrapped = format!("[[{value}]]");
            json!({
                "or": [
                    { "==": [var.clone(), value] },
                    { "==": [var, wrapped] },
                ]
            })
        }
    }
}
