use anyhow::Result;
use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, Write as IoWrite};
use std::path::{Path, PathBuf};

/// A single recorded capture event.
///
/// String/Option fields use `#[serde(default)]` for forward compatibility —
/// future fields added to JSONL lines will be silently ignored by older readers.
/// `timestamp` is intentionally required: an entry without a valid timestamp is
/// unparseable and will be skipped by `load_jsonl`, which is the correct behavior
/// (a dateless entry has no meaningful place in any time-based query).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    #[serde(default)]
    pub module_key: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub vault_path: String,
    /// Value of the first field at capture time (for dashboard display).
    #[serde(default)]
    pub first_field: Option<String>,
}

/// Legacy on-disk schema — used only for migration from the old `history.json` format.
#[derive(Debug, Deserialize)]
struct LegacyHistoryData {
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

/// Precomputed dashboard statistics, persisted alongside the JSONL log so the
/// dashboard can render instantly without parsing the full history.
///
/// All fields are `#[serde(default)]` for forward compatibility — new summary
/// fields can be added without breaking older readers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistorySummary {
    /// Schema version for future migration. Current: 1.
    #[serde(default = "summary_version_default")]
    pub version: u32,

    #[serde(default)]
    pub last_pour: Option<HistoryEntry>,

    #[serde(default)]
    pub last_per_module: HashMap<String, DateTime<Utc>>,

    #[serde(default)]
    pub streak: u64,

    /// The local date when the summary was last computed.
    /// If this doesn't match today, the summary is stale and must be recomputed.
    #[serde(default)]
    pub computed_date: Option<NaiveDate>,

    #[serde(default)]
    pub today_count: usize,

    #[serde(default)]
    pub week_count: usize,

    #[serde(default)]
    pub per_module_today: HashMap<String, usize>,

    /// The Monday of the week when summary was computed.
    #[serde(default)]
    pub week_start: Option<NaiveDate>,

    #[serde(default)]
    pub total_entries: usize,
}

fn summary_version_default() -> u32 {
    1
}

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
    pub fn record(
        &mut self,
        module_key: &str,
        vault_path: &str,
        first_field: Option<&str>,
    ) -> Result<()> {
        let entry = HistoryEntry {
            module_key: module_key.to_owned(),
            timestamp: Utc::now(),
            vault_path: vault_path.to_owned(),
            first_field: first_field.map(|s| s.to_owned()),
        };

        // Append single line to JSONL — O(1) write
        append_entry(&self.path, &entry)?;

        self.entries.push(entry);

        // Recompute and persist summary
        self.summary = compute_summary(&self.entries);
        let _ = write_summary(&summary_path(&self.path), &self.summary);

        Ok(())
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
fn load_jsonl(path: &Path) -> Vec<HistoryEntry> {
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
// Summary cache
// ---------------------------------------------------------------------------

fn summary_path(jsonl_path: &Path) -> PathBuf {
    jsonl_path
        .parent()
        .unwrap_or(jsonl_path)
        .join("history-summary.json")
}

fn compute_summary(entries: &[HistoryEntry]) -> HistorySummary {
    let now = Local::now();
    let today = now.date_naive();
    let weekday = today.weekday().num_days_from_monday();
    let week_start = today - chrono::Duration::days(weekday as i64);

    let mut last_per_module: HashMap<String, DateTime<Utc>> = HashMap::new();
    let mut today_count = 0usize;
    let mut week_count = 0usize;
    let mut per_module_today: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        // last_per_module
        last_per_module
            .entry(entry.module_key.clone())
            .and_modify(|ts| {
                if entry.timestamp > *ts {
                    *ts = entry.timestamp;
                }
            })
            .or_insert(entry.timestamp);

        let d = entry.timestamp.with_timezone(&Local).date_naive();
        if d == today {
            today_count += 1;
            *per_module_today
                .entry(entry.module_key.clone())
                .or_insert(0) += 1;
        }
        if d >= week_start && d <= today {
            week_count += 1;
        }
    }

    // Streak calculation
    let streak = if entries.is_empty() {
        0
    } else {
        let mut dates: Vec<NaiveDate> = entries
            .iter()
            .map(|e| e.timestamp.with_timezone(&Local).date_naive())
            .collect();
        dates.sort();
        dates.dedup();

        let last_date = dates.last().copied().unwrap_or(today);
        let gap = (today - last_date).num_days();
        if gap > 1 {
            0
        } else {
            let mut s = 1u64;
            for i in (0..dates.len().saturating_sub(1)).rev() {
                if (dates[i + 1] - dates[i]).num_days() == 1 {
                    s += 1;
                } else {
                    break;
                }
            }
            s
        }
    };

    HistorySummary {
        version: 1,
        last_pour: entries.last().cloned(),
        last_per_module,
        streak,
        computed_date: Some(today),
        today_count,
        week_count,
        per_module_today,
        week_start: Some(week_start),
        total_entries: entries.len(),
    }
}

fn write_summary(path: &Path, summary: &HistorySummary) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(summary)?;
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, &json)?;
    crate::util::atomic_replace(&tmp_path, path)?;
    Ok(())
}

fn load_or_recompute_summary(path: &Path, entries: &[HistoryEntry]) -> HistorySummary {
    // Try loading cached summary
    if let Ok(contents) = std::fs::read_to_string(path)
        && let Ok(summary) = serde_json::from_str::<HistorySummary>(&contents)
    {
        let today = Local::now().date_naive();
        if summary.computed_date == Some(today) && summary.total_entries == entries.len() {
            // Summary is fresh — use it
            return summary;
        }
    }

    // Stale or missing — recompute from entries
    let summary = compute_summary(entries);
    let _ = write_summary(path, &summary);
    summary
}

// ---------------------------------------------------------------------------
// Legacy migration
// ---------------------------------------------------------------------------

/// Attempt to migrate from the old `history.json` (single JSON array) format
/// to the new `history.jsonl` format. Returns the entries on success.
fn migrate_legacy(legacy_path: &Path, jsonl_path: &Path) -> Option<Vec<HistoryEntry>> {
    let contents = std::fs::read_to_string(legacy_path).ok()?;
    let legacy: LegacyHistoryData = serde_json::from_str(&contents).ok()?;

    if let Some(parent) = jsonl_path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }

    // Write entries as JSONL
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
