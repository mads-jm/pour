//! Wikilink stripping for match comparison.
//!
//! Obsidian wikilinks take the forms `[[Target]]`, `[[Target|Alias]]`,
//! `[[Target#Fragment]]`, and combinations. Pour's frontmatter *writer* wraps
//! select/text values in `[[ ]]` when a field declares `wikilink = true`. To
//! compare such a stored value against the bare target the resolver keys on
//! (e.g. the picked bean/roaster name), we strip the wikilink syntax down to
//! its target.
//!
//! This is shared foundation — the priors resolver uses it for wikilink-mode
//! match comparison, and lookup-fields L1 will reuse it. It is intentionally
//! narrow: it resolves the *target* (the file the link points at), discarding
//! any alias or heading/block fragment.

/// Strip Obsidian wikilink syntax from a value, returning the bare target.
///
/// Rules:
/// - `[[Target|Alias]]` → `Target`
/// - `[[Target#Fragment]]` → `Target`
/// - `[[Target#Frag|Alias]]` → `Target`
/// - `[[Target]]` → `Target`
/// - A value with no wikilink wrapping passes through unchanged.
///
/// Only a single leading `[[ … ]]` wrap is unwrapped; the value is trimmed
/// first so surrounding whitespace (e.g. from YAML) does not defeat the match.
/// If the value is not a well-formed single wikilink, it is returned trimmed
/// but otherwise unchanged — this keeps the function total and non-crashing on
/// arbitrary externally-edited frontmatter.
pub fn strip_wikilink(value: &str) -> String {
    let trimmed = value.trim();

    let inner = match trimmed
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
    {
        Some(inner) => inner,
        // Not a wikilink — return the trimmed value unchanged.
        None => return trimmed.to_string(),
    };

    // Drop an alias (`|Alias`) — the target is the portion before the first `|`.
    let target = inner.split('|').next().unwrap_or(inner);
    // Drop a heading/block fragment (`#Fragment`).
    let target = target.split('#').next().unwrap_or(target);

    target.trim().to_string()
}
