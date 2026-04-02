use anyhow::Result;
use chrono::{DateTime, Datelike, Local, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A single recorded capture event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub module_key: String,
    pub timestamp: DateTime<Utc>,
    pub vault_path: String,
    /// Value of the first field at capture time (for dashboard display).
    #[serde(default)]
    pub first_field: Option<String>,
}

/// On-disk schema for the history file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryData {
    #[serde(default)]
    pub entries: Vec<HistoryEntry>,
}

/// Manages the capture history log persisted at `~/.cache/pour/history.json`.
#[derive(Debug)]
pub struct History {
    data: HistoryData,
    path: PathBuf,
}

impl History {
    /// Load history from the default platform cache directory.
    /// Returns empty history if the file is missing or corrupt.
    pub fn load() -> Self {
        let path = default_history_path();
        Self::load_from(path)
    }

    /// Load history from a specific file path.
    pub fn load_from(path: PathBuf) -> Self {
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str::<HistoryData>(&contents).ok())
            .unwrap_or_default();

        History { data, path }
    }

    /// Record a successful capture and persist to disk.
    pub fn record(&mut self, module_key: &str, vault_path: &str, first_field: Option<&str>) {
        self.data.entries.push(HistoryEntry {
            module_key: module_key.to_owned(),
            timestamp: Utc::now(),
            vault_path: vault_path.to_owned(),
            first_field: first_field.map(|s| s.to_owned()),
        });
        let _ = self.save();
    }

    /// Persist history to disk (atomic write).
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.data)?;
        let tmp_path = self.path.with_extension("tmp");
        std::fs::write(&tmp_path, &json)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    /// Most recent entry, if any.
    pub fn last_pour(&self) -> Option<&HistoryEntry> {
        self.data.entries.last()
    }

    /// Number of entries logged today (local time).
    pub fn today_count(&self) -> usize {
        let today = Local::now().date_naive();
        self.data
            .entries
            .iter()
            .filter(|e| e.timestamp.with_timezone(&Local).date_naive() == today)
            .count()
    }

    /// Number of entries logged in the current calendar week (Mon–Sun, local time).
    pub fn week_count(&self) -> usize {
        let now = Local::now();
        let today = now.date_naive();
        let weekday = today.weekday().num_days_from_monday(); // Mon=0
        let week_start = today - chrono::Duration::days(weekday as i64);

        self.data
            .entries
            .iter()
            .filter(|e| {
                let d = e.timestamp.with_timezone(&Local).date_naive();
                d >= week_start && d <= today
            })
            .count()
    }

    /// Consecutive days with at least one capture, ending today or yesterday.
    pub fn streak(&self) -> u64 {
        if self.data.entries.is_empty() {
            return 0;
        }

        let today = Local::now().date_naive();

        // Collect unique capture dates
        let mut dates: Vec<chrono::NaiveDate> = self
            .data
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
    pub fn per_module_today(&self) -> HashMap<String, usize> {
        let today = Local::now().date_naive();
        let mut counts: HashMap<String, usize> = HashMap::new();

        for entry in &self.data.entries {
            if entry.timestamp.with_timezone(&Local).date_naive() == today {
                *counts.entry(entry.module_key.clone()).or_insert(0) += 1;
            }
        }

        counts
    }

    /// Last N entries (most recent first).
    pub fn recent(&self, n: usize) -> Vec<&HistoryEntry> {
        self.data.entries.iter().rev().take(n).collect()
    }

    /// Most recent timestamp per module key.
    pub fn last_per_module(&self) -> HashMap<String, DateTime<Utc>> {
        let mut map: HashMap<String, DateTime<Utc>> = HashMap::new();

        for entry in &self.data.entries {
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
}

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

/// Resolve the default history file path.
fn default_history_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("pour")
        .join("history.json")
}
