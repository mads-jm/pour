pub mod cache;

use crate::transport::Transport;
use cache::Cache;
use std::path::Path;

/// Fetch options for a dynamic select field using a 3-tier fallback:
///
/// 1. **Transport** — `list_directory` via the API or filesystem.
/// 2. **Cache** — Previously fetched items from `state.json`.
/// 3. **Empty** — Return an empty vec; the TUI will offer freetext input.
///
/// Results are normalized to file stems (no `.md` extension, no trailing `/`)
/// so that cached values are consistent regardless of which transport backend
/// originally fetched them.
///
/// On a successful (non-empty) transport fetch the cache is updated so
/// subsequent offline launches can still populate the dropdown.
pub async fn fetch_options(
    transport: &Transport,
    source_path: &str,
    cache: &mut Cache,
) -> Vec<String> {
    // Tier 1: try the transport layer (API with FS fallback).
    if let Ok(items) = transport.list_directory(source_path).await
        && !items.is_empty()
    {
        let normalized = normalize_items(items);
        cache.set(source_path, normalized.clone());
        return normalized;
    }

    // Tier 2: fall back to cached data.
    if let Some(items) = cache.get(source_path) {
        return items;
    }

    // Tier 3: nothing available — caller should offer freetext.
    Vec::new()
}

/// Normalize raw directory listing items to file stems.
///
/// - Strips `.md` extension (e.g. `"Ethiopia.md"` → `"Ethiopia"`)
/// - Strips trailing `/` from directory entries (then excludes them)
/// - Filters out empty strings
fn normalize_items(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .filter(|s| !s.ends_with('/'))
        .map(|s| {
            Path::new(&s)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&s)
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
