use anyhow::Result;
use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::history::HistoryEntry;

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

pub(crate) fn summary_path(jsonl_path: &Path) -> PathBuf {
    jsonl_path
        .parent()
        .unwrap_or(jsonl_path)
        .join("history-summary.json")
}

pub(crate) fn compute_summary(entries: &[HistoryEntry]) -> HistorySummary {
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

pub(crate) fn write_summary(path: &Path, summary: &HistorySummary) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(summary)?;
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, &json)?;
    crate::util::atomic_replace(&tmp_path, path)?;
    Ok(())
}

pub(crate) fn load_or_recompute_summary(path: &Path, entries: &[HistoryEntry]) -> HistorySummary {
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
