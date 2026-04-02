use chrono::{Duration, Utc};
use pour::data::history::{History, HistoryData, HistoryEntry, format_relative};

/// Create a History with the given entries, backed by a temp file.
fn history_with_entries(entries: Vec<HistoryEntry>) -> (History, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("history.json");

    let data = HistoryData { entries };
    let json = serde_json::to_string_pretty(&data).expect("serialize");
    std::fs::write(&path, json).expect("write temp history");

    (History::load_from(path), dir)
}

fn entry(module: &str, hours_ago: i64) -> HistoryEntry {
    HistoryEntry {
        module_key: module.to_string(),
        timestamp: Utc::now() - Duration::hours(hours_ago),
        vault_path: format!("test/{module}.md"),
        first_field: None,
    }
}

fn entry_days_ago(module: &str, days: i64) -> HistoryEntry {
    HistoryEntry {
        module_key: module.to_string(),
        timestamp: Utc::now() - Duration::days(days),
        vault_path: format!("test/{module}.md"),
        first_field: None,
    }
}

#[test]
fn empty_history_returns_none_and_zeros() {
    let (h, _dir) = history_with_entries(vec![]);
    assert!(h.last_pour().is_none());
    assert_eq!(h.today_count(), 0);
    assert_eq!(h.week_count(), 0);
    assert_eq!(h.streak(), 0);
    assert!(h.recent(5).is_empty());
    assert!(h.per_module_today().is_empty());
    assert!(h.last_per_module().is_empty());
}

#[test]
fn last_pour_returns_most_recent() {
    let (h, _dir) = history_with_entries(vec![entry("coffee", 5), entry("me", 1)]);
    let last = h.last_pour().expect("should have entries");
    assert_eq!(last.module_key, "me");
}

#[test]
fn today_count_only_counts_today() {
    let (h, _dir) = history_with_entries(vec![
        entry("coffee", 1),         // ~1h ago, today
        entry("me", 2),             // ~2h ago, today
        entry_days_ago("music", 2), // 2 days ago
    ]);
    // The first two should be today (unless test runs at midnight)
    assert!(h.today_count() >= 2);
}

#[test]
fn week_count_includes_this_week() {
    let (h, _dir) = history_with_entries(vec![
        entry("coffee", 1),
        entry_days_ago("me", 1),
        entry_days_ago("music", 2),
        entry_days_ago("coffee", 10), // >1 week ago
    ]);
    // At least 3 should be this week (the 10-day-ago one might not be)
    assert!(h.week_count() >= 3);
}

#[test]
fn streak_consecutive_days() {
    let (h, _dir) = history_with_entries(vec![
        entry("coffee", 1),          // today
        entry_days_ago("me", 1),     // yesterday
        entry_days_ago("music", 2),  // 2 days ago
        entry_days_ago("coffee", 5), // gap — 5 days ago
    ]);
    // Streak should be 3 (today, yesterday, 2 days ago)
    assert_eq!(h.streak(), 3);
}

#[test]
fn streak_zero_when_no_recent_captures() {
    let (h, _dir) = history_with_entries(vec![entry_days_ago("coffee", 5)]);
    assert_eq!(h.streak(), 0);
}

#[test]
fn per_module_today_groups_correctly() {
    let (h, _dir) = history_with_entries(vec![
        entry("coffee", 1),
        entry("coffee", 2),
        entry("me", 1),
        entry_days_ago("music", 2),
    ]);
    let counts = h.per_module_today();
    assert_eq!(*counts.get("coffee").unwrap_or(&0), 2);
    assert_eq!(*counts.get("me").unwrap_or(&0), 1);
    assert!(!counts.contains_key("music")); // not today
}

#[test]
fn recent_returns_most_recent_first() {
    let (h, _dir) =
        history_with_entries(vec![entry("coffee", 5), entry("me", 3), entry("music", 1)]);
    let recent = h.recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].module_key, "music");
    assert_eq!(recent[1].module_key, "me");
}

#[test]
fn last_per_module_tracks_each_module() {
    let (h, _dir) =
        history_with_entries(vec![entry("coffee", 5), entry("coffee", 1), entry("me", 3)]);
    let map = h.last_per_module();
    assert!(map.contains_key("coffee"));
    assert!(map.contains_key("me"));
    // Coffee's latest should be the 1-hour-ago entry
    let coffee_ts = map["coffee"];
    let me_ts = map["me"];
    assert!(coffee_ts > me_ts);
}

#[test]
fn record_persists_to_disk() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("history.json");
    let mut h = History::load_from(path.clone());

    assert_eq!(h.today_count(), 0);
    h.record("coffee", "Coffee/2026/test.md", Some("Ethiopia Yirg"));
    assert_eq!(h.today_count(), 1);

    // Reload from disk
    let h2 = History::load_from(path);
    assert_eq!(h2.today_count(), 1);
    assert_eq!(h2.last_pour().unwrap().module_key, "coffee");
}

#[test]
fn format_relative_just_now() {
    let ts = Utc::now() - Duration::minutes(5);
    assert_eq!(format_relative(ts), "just now");
}

#[test]
fn format_relative_today_with_time() {
    let ts = Utc::now() - Duration::hours(3);
    let result = format_relative(ts);
    // Should be HH:MM format
    assert!(result.contains(':'), "expected HH:MM, got: {result}");
}

#[test]
fn format_relative_yesterday() {
    let ts = Utc::now() - Duration::hours(30);
    // This might be "yesterday" or a time depending on when the test runs
    // but for ~30h ago it should reliably be "yesterday"
    let result = format_relative(ts);
    assert!(
        result == "yesterday" || result.contains(':') || result.contains("d ago"),
        "unexpected: {result}"
    );
}

#[test]
fn format_relative_days_ago() {
    let ts = Utc::now() - Duration::days(4);
    assert_eq!(format_relative(ts), "4d ago");
}

#[test]
fn format_relative_weeks_ago() {
    let ts = Utc::now() - Duration::days(14);
    assert_eq!(format_relative(ts), "2w ago");
}
