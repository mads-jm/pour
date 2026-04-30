use serde::Deserialize;
use std::path::Path;

use super::history::{HistoryEntry, load_jsonl};

/// Legacy on-disk schema — used only for migration from the old `history.json` format.
#[derive(Debug, Deserialize)]
struct LegacyHistoryData {
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

/// Attempt to migrate from the old `history.json` (single JSON array) format
/// to the new `history.jsonl` format. Returns the entries on success.
pub(crate) fn migrate_legacy(legacy_path: &Path, jsonl_path: &Path) -> Option<Vec<HistoryEntry>> {
    let contents = std::fs::read_to_string(legacy_path).ok()?;
    let legacy: LegacyHistoryData = serde_json::from_str(&contents).ok()?;

    if let Some(parent) = jsonl_path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }

    // Write entries as JSONL
    use std::io::Write as IoWrite;
    let mut file = std::fs::File::create(jsonl_path).ok()?;
    for entry in &legacy.entries {
        let line = serde_json::to_string(entry).ok()?;
        writeln!(file, "{}", line).ok()?;
    }
    // Flush to OS and sync to disk before removing legacy file
    file.flush().ok()?;
    file.sync_all().ok()?;

    // Verify the written file by counting parseable lines
    let verified = load_jsonl(jsonl_path);
    if verified.len() < legacy.entries.len() {
        // Migration incomplete — leave legacy file in place for next attempt
        let _ = std::fs::remove_file(jsonl_path);
        return None;
    }

    // Remove old file only after verified write
    let _ = std::fs::remove_file(legacy_path);

    Some(legacy.entries)
}
