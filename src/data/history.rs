use anyhow::Result;
use chrono::{DateTime, Datelike, Local, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::history_legacy::migrate_legacy;
use super::history_summary::{
    compute_summary, load_or_recompute_summary, summary_path, write_summary,
};

// Re-export so callers using `crate::data::history::HistorySummary` keep working.
pub use super::history_summary::HistorySummary;

/// A single recorded capture event.
///
/// String/Option fields use `#[serde(default)]` for forward compatibility —
/// future fields added to JSONL lines will be silently ignored by older readers.
/// `timestamp` is intentionally required: an entry without a valid timestamp is
/// unparseable and will be skipped by `load_jsonl`, which is the correct behavior
/// (a dateless entry has no meaningful place in any time-based query).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Opaque identifier in the form `<YYYYMMDDTHHmmss>-<module_key>`.
    /// Used by the API to identify captures in `/api/v1/captures/{id}`.
    /// Absent on legacy entries (pre-Step-C); callers should treat `None` as
    /// meaning the entry predates the API and has no associated capture id.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub module_key: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub vault_path: String,
    /// Value of the first field at capture time (for dashboard display).
    #[serde(default)]
    pub first_field: Option<String>,
}

/// Monotonically-increasing counter for within-millisecond id uniqueness.
///
/// When two `record()` calls arrive at the same millisecond (e.g. test fixtures
/// or rapid concurrent PWA submits), the counter suffix prevents duplicate ids.
/// The counter resets to 0 on process restart which is fine — the ms prefix
/// already provides sufficient real-world uniqueness.
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Manages the capture history log persisted at `~/.pour/cache/history.jsonl`.
///
/// Write path: append a single JSON line per capture (O(1)).
/// Read path: parse JSONL at startup, build in-memory vec.
/// Summary cache: `history-summary.json` for fast dashboard stats.
#[derive(Debug)]
pub struct History {
    entries: Vec<HistoryEntry>,
    path: PathBuf,
    summary: HistorySummary,
}

impl History {
    /// Load history from the default location
    /// (`~/.pour/cache/history.jsonl`, or under `$POUR_HOME/cache/` if set).
    /// Returns empty history if the file is missing or corrupt.
    pub fn load() -> Self {
        Self::load_from(crate::paths::history_path())
    }

    /// Load history from a specific file path.
    pub fn load_from(path: PathBuf) -> Self {
        // Check for legacy format and migrate if needed
        let legacy_path = path.with_extension("json");
        if !path.exists()
            && legacy_path.exists()
            && let Some(migrated) = migrate_legacy(&legacy_path, &path)
        {
            let summary = compute_summary(&migrated);
            let _ = write_summary(&summary_path(&path), &summary);
            return History {
                entries: migrated,
                path,
                summary,
            };
        }

        let entries = load_jsonl(&path);
        let summary = load_or_recompute_summary(&summary_path(&path), &entries);

        History {
            entries,
            path,
            summary,
        }
    }

    /// Record a successful capture and persist to disk.
    ///
    /// `at` is the canonical timestamp for this capture. The TUI passes
    /// `Utc::now()`; the server passes the `captured_at`-derived UTC value
    /// so that offline replays are dated correctly (contract §10).
    ///
    /// Returns the opaque `id` string assigned to the new entry.
    pub fn record(
        &mut self,
        module_key: &str,
        vault_path: &str,
        first_field: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<String> {
        // Build a collision-resistant id by appending milliseconds and a
        // monotonic in-process counter.
        //
        // Format: YYYYMMDDTHHmmSSsss-<counter>-<module_key>
        // - ms suffix: prevents same-second collisions under real concurrent load.
        // - counter suffix: prevents same-millisecond collisions in tests or when
        //   the same captured_at value is replayed (the counter resets on restart
        //   but ms already handles real-world concurrent PWA submits).
        // - Legacy entries (no ms/counter) remain uniquely findable — find_by_id
        //   uses exact-string matching so old ids resolve correctly.
        let seq = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let id = format!("{}-{}-{}", at.format("%Y%m%dT%H%M%S%3f"), seq, module_key);
        let entry = HistoryEntry {
            id: Some(id.clone()),
            module_key: module_key.to_owned(),
            timestamp: at,
            vault_path: vault_path.to_owned(),
            first_field: first_field.map(|s| s.to_owned()),
        };

        // Append single line to JSONL — O(1) write
        append_entry(&self.path, &entry)?;

        self.entries.push(entry);

        // Recompute and persist summary
        self.summary = compute_summary(&self.entries);
        let _ = write_summary(&summary_path(&self.path), &self.summary);

        Ok(id)
    }

    /// Persist history to disk (full rewrite — used only for migration/tests).
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&self.path)?;
        for entry in &self.entries {
            let line = serde_json::to_string(entry)?;
            writeln!(file, "{}", line)?;
        }
        file.flush()?;

        // Also update summary
        let _ = write_summary(&summary_path(&self.path), &self.summary);
        Ok(())
    }

    /// All entries, oldest first.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Find an entry by its opaque `id` string.
    pub fn find_by_id(&self, id: &str) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id.as_deref() == Some(id))
    }

    /// Most recent entry, if any.
    pub fn last_pour(&self) -> Option<&HistoryEntry> {
        self.entries.last()
    }

    /// Number of entries logged today (local time).
    /// Returns cached value when the summary is fresh.
    pub fn today_count(&self) -> usize {
        if self.summary_is_fresh() {
            return self.summary.today_count;
        }
        let today = Local::now().date_naive();
        self.entries
            .iter()
            .filter(|e| e.timestamp.with_timezone(&Local).date_naive() == today)
            .count()
    }

    /// Number of entries logged in the current calendar week (Mon–Sun, local time).
    /// Returns cached value when the summary is fresh.
    pub fn week_count(&self) -> usize {
        if self.summary_is_fresh() {
            return self.summary.week_count;
        }
        let now = Local::now();
        let today = now.date_naive();
        let weekday = today.weekday().num_days_from_monday(); // Mon=0
        let week_start = today - chrono::Duration::days(weekday as i64);

        self.entries
            .iter()
            .filter(|e| {
                let d = e.timestamp.with_timezone(&Local).date_naive();
                d >= week_start && d <= today
            })
            .count()
    }

    /// Consecutive days with at least one capture, ending today or yesterday.
    /// Returns cached value when the summary is fresh.
    pub fn streak(&self) -> u64 {
        if self.summary_is_fresh() {
            return self.summary.streak;
        }
        if self.entries.is_empty() {
            return 0;
        }

        let today = Local::now().date_naive();

        // Collect unique capture dates
        let mut dates: Vec<chrono::NaiveDate> = self
            .entries
            .iter()
            .map(|e| e.timestamp.with_timezone(&Local).date_naive())
            .collect();
        dates.sort();
        dates.dedup();

        // Must include today or yesterday to have an active streak
        let last_date = match dates.last() {
            Some(d) => *d,
            None => return 0,
        };

        let gap = (today - last_date).num_days();
        if gap > 1 {
            return 0;
        }

        // Walk backwards from the most recent date
        let mut streak = 1u64;
        for i in (0..dates.len().saturating_sub(1)).rev() {
            if (dates[i + 1] - dates[i]).num_days() == 1 {
                streak += 1;
            } else {
                break;
            }
        }

        streak
    }

    /// Capture counts by module for today (local time).
    /// Returns cached value when the summary is fresh.
    pub fn per_module_today(&self) -> HashMap<String, usize> {
        if self.summary_is_fresh() {
            return self.summary.per_module_today.clone();
        }
        let today = Local::now().date_naive();
        let mut counts: HashMap<String, usize> = HashMap::new();

        for entry in &self.entries {
            if entry.timestamp.with_timezone(&Local).date_naive() == today {
                *counts.entry(entry.module_key.clone()).or_insert(0) += 1;
            }
        }

        counts
    }

    /// Last N entries (most recent first).
    pub fn recent(&self, n: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Paginated, filtered history query (§6.5 cursor pagination).
    ///
    /// Filters by:
    /// - `since`: inclusive lower bound (`timestamp >= since`).
    /// - `until`: exclusive upper bound (`timestamp < until`). Raw timestamp
    ///   filter — kept for direct "older than X date" use. NOT used as the
    ///   pagination cursor; use `cursor` for pagination.
    /// - `cursor`: opaque string cursor (the `id` of the last entry from the
    ///   previous page). Filters to entries whose `id` is lexicographically
    ///   less than the cursor. Because ids are `YYYYMMDDTHHmmSSsss-seq-module`,
    ///   lexicographic order == chronological+counter order, so this correctly
    ///   handles same-millisecond entries that a timestamp-only cursor would drop.
    /// - `module`: exact `module_key` match.
    ///
    /// Returns entries in descending id order (most recent first).
    ///
    /// `limit` must be ≥ 1. The method fetches up to `limit + 1` entries to
    /// detect whether a next page exists:
    /// - If > `limit` found: `has_more = true`,
    ///   `next_cursor = Some(entries[limit - 1].id)`.
    /// - Otherwise: `has_more = false`, `next_cursor = None`.
    pub fn filter(
        &self,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
        cursor: Option<&str>,
        module: Option<&str>,
        limit: usize,
    ) -> (Vec<HistoryEntry>, bool, Option<String>) {
        let mut filtered: Vec<&HistoryEntry> = self
            .entries
            .iter()
            .filter(|e| {
                if let Some(s) = since
                    && e.timestamp < s
                {
                    return false;
                }
                if let Some(u) = until
                    && e.timestamp >= u
                {
                    return false;
                }
                // Cursor: id-based exclusive upper bound. Entries whose id is
                // lexicographically >= cursor are on a previous page.
                if let Some(c) = cursor {
                    let entry_id = e.id.as_deref().unwrap_or("");
                    if entry_id >= c {
                        return false;
                    }
                }
                if let Some(m) = module
                    && e.module_key != m
                {
                    return false;
                }
                true
            })
            .collect();

        // Sort descending by id (lexicographic == chronological+counter for our
        // id format). Entries without an id (legacy) sort last (empty string).
        filtered.sort_by(|a, b| {
            let a_id = a.id.as_deref().unwrap_or("");
            let b_id = b.id.as_deref().unwrap_or("");
            b_id.cmp(a_id)
        });

        let has_more = filtered.len() > limit;
        // next_cursor is the id of the last entry we return on this page.
        // The client passes it as `?cursor=<next_cursor>` for the next page.
        let next_cursor: Option<String> = if has_more {
            filtered
                .get(limit.saturating_sub(1))
                .and_then(|e| e.id.clone())
        } else {
            None
        };

        let entries: Vec<HistoryEntry> = filtered.into_iter().take(limit).cloned().collect();

        (entries, has_more, next_cursor)
    }

    /// Returns the precomputed summary, recomputing if stale.
    pub fn summary(&self) -> &HistorySummary {
        &self.summary
    }

    /// Most recent timestamp per module key.
    /// Returns cached value when the summary is fresh.
    pub fn last_per_module(&self) -> HashMap<String, DateTime<Utc>> {
        if self.summary_is_fresh() {
            return self.summary.last_per_module.clone();
        }
        let mut map: HashMap<String, DateTime<Utc>> = HashMap::new();

        for entry in &self.entries {
            map.entry(entry.module_key.clone())
                .and_modify(|ts| {
                    if entry.timestamp > *ts {
                        *ts = entry.timestamp;
                    }
                })
                .or_insert(entry.timestamp);
        }

        map
    }

    /// Check if the summary cache is fresh (same date and entry count).
    fn summary_is_fresh(&self) -> bool {
        let today = Local::now().date_naive();
        self.summary.computed_date == Some(today)
            && self.summary.total_entries == self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// JSONL I/O
// ---------------------------------------------------------------------------

/// Append a single entry as one JSON line to the JSONL file.
fn append_entry(path: &Path, entry: &HistoryEntry) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let line = serde_json::to_string(entry)?;
    writeln!(file, "{}", line)?;
    file.flush()?;
    Ok(())
}

/// Load all entries from a JSONL file. Silently skips unparseable lines
/// (handles both corruption from partial writes and forward-compatible
/// schema changes with unknown fields).
pub(crate) fn load_jsonl(path: &Path) -> Vec<HistoryEntry> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(trimmed) {
            entries.push(entry);
        }
        // Silently skip unparseable lines — forward compatibility
    }

    entries
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Format a UTC timestamp as a human-readable relative time string.
pub fn format_relative(dt: DateTime<Utc>) -> String {
    let now_local = Local::now();
    let dt_local = dt.with_timezone(&Local);
    let today = now_local.date_naive();
    let dt_date = dt_local.date_naive();

    if dt_date == today {
        let hours_ago = (now_local - dt_local).num_hours();
        if hours_ago < 1 {
            return "just now".to_string();
        }
        return dt_local.format("%H:%M").to_string();
    }

    let days_ago = (today - dt_date).num_days();

    if days_ago == 1 {
        return "yesterday".to_string();
    }

    if days_ago < 7 {
        return format!("{days_ago}d ago");
    }

    let weeks = days_ago / 7;
    format!("{weeks}w ago")
}
