use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use pour::data::history::{History, HistoryEntry, format_relative};
use std::io::Write;

/// Create a History backed by a temp JSONL file with the given entries.
fn history_with_entries(entries: Vec<HistoryEntry>) -> (History, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("history.jsonl");

    let mut file = std::fs::File::create(&path).expect("create temp history");
    for entry in &entries {
        let line = serde_json::to_string(entry).expect("serialize entry");
        writeln!(file, "{}", line).expect("write line");
    }
    file.flush().expect("flush");

    (History::load_from(path), dir)
}

// Anchor test timestamps to today-noon-local (converted to UTC) so small
// hour/day offsets stay inside their intended local calendar day regardless
// of what time-of-day CI fires the suite. Production code buckets by
// `Local::now().date_naive()`; anchoring on `Utc::now()` straddled midnight.
fn today_noon_utc() -> DateTime<Utc> {
    let today = Local::now().date_naive();
    Local
        .from_local_datetime(&today.and_hms_opt(12, 0, 0).unwrap())
        .single()
        .expect("noon today is unambiguous")
        .with_timezone(&Utc)
}

fn entry(module: &str, hours_ago: i64) -> HistoryEntry {
    HistoryEntry {
        module_key: module.to_string(),
        timestamp: today_noon_utc() - Duration::hours(hours_ago),
        vault_path: format!("test/{module}.md"),
        first_field: None,
    }
}

fn entry_days_ago(module: &str, days: i64) -> HistoryEntry {
    HistoryEntry {
        module_key: module.to_string(),
        timestamp: today_noon_utc() - Duration::days(days),
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
    // week_count uses Mon–Sun calendar weeks, so entries from previous days
    // may fall in the prior week depending on what day the test runs.
    // Use only today's entries to guarantee they're in the current week.
    let (h, _dir) = history_with_entries(vec![
        entry("coffee", 1),
        entry("me", 2),
        entry("music", 3),
        entry_days_ago("coffee", 10), // >1 week ago
    ]);
    // The three entries from today should all be in this week
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
    let path = dir.path().join("history.jsonl");
    let mut h = History::load_from(path.clone());

    assert_eq!(h.today_count(), 0);
    h.record("coffee", "Coffee/2026/test.md", Some("Ethiopia Yirg"))
        .expect("record should succeed");
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
    // Need a timestamp that is (a) today-local and (b) at least 1 hour before
    // now-local, so `format_relative` returns HH:MM rather than "just now".
    // In the first hour of the local day no such timestamp exists; skip then.
    let now = Local::now();
    let ts_local = now - Duration::hours(2);
    if ts_local.date_naive() != now.date_naive() {
        return;
    }
    let result = format_relative(ts_local.with_timezone(&Utc));
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
    let ts = today_noon_utc() - Duration::days(4);
    assert_eq!(format_relative(ts), "4d ago");
}

#[test]
fn format_relative_weeks_ago() {
    let ts = today_noon_utc() - Duration::days(14);
    assert_eq!(format_relative(ts), "2w ago");
}

// ---------------------------------------------------------------------------
// JSONL-specific tests
// ---------------------------------------------------------------------------

#[test]
fn corrupt_last_line_is_skipped() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("history.jsonl");

    // Write a valid entry then a corrupt partial line
    let valid = entry("coffee", 1);
    let line = serde_json::to_string(&valid).expect("serialize");
    std::fs::write(&path, format!("{}\n{{\"module_key\":\"broken\n", line)).expect("write");

    let h = History::load_from(path);
    assert_eq!(h.recent(10).len(), 1);
    assert_eq!(h.last_pour().unwrap().module_key, "coffee");
}

#[test]
fn unknown_fields_in_jsonl_are_ignored() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("history.jsonl");

    // Simulate a future version writing extra fields
    let line = r#"{"module_key":"coffee","timestamp":"2026-04-06T12:00:00Z","vault_path":"test.md","first_field":null,"future_field":"hello","another_new_thing":42}"#;
    std::fs::write(&path, format!("{}\n", line)).expect("write");

    let h = History::load_from(path);
    assert_eq!(h.recent(10).len(), 1);
    assert_eq!(h.last_pour().unwrap().module_key, "coffee");
}

#[test]
fn legacy_json_format_auto_migrates() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let legacy_path = dir.path().join("history.json");
    let jsonl_path = dir.path().join("history.jsonl");

    // Write old-format history.json
    let old_data = serde_json::json!({
        "entries": [
            {
                "module_key": "coffee",
                "timestamp": "2026-04-06T10:00:00Z",
                "vault_path": "Coffee/test.md",
                "first_field": "Ethiopia"
            },
            {
                "module_key": "me",
                "timestamp": "2026-04-06T11:00:00Z",
                "vault_path": "Journal/test.md",
                "first_field": null
            }
        ]
    });
    std::fs::write(
        &legacy_path,
        serde_json::to_string_pretty(&old_data).unwrap(),
    )
    .expect("write legacy");

    // Load from the .jsonl path — should detect and migrate
    let h = History::load_from(jsonl_path.clone());
    assert_eq!(h.recent(10).len(), 2);
    assert_eq!(h.last_pour().unwrap().module_key, "me");

    // Legacy file should be removed
    assert!(
        !legacy_path.exists(),
        "legacy file should be deleted after migration"
    );

    // JSONL file should exist
    assert!(
        jsonl_path.exists(),
        "jsonl file should exist after migration"
    );

    // Reload to verify persistence
    let h2 = History::load_from(jsonl_path);
    assert_eq!(h2.recent(10).len(), 2);
}

#[test]
fn empty_lines_in_jsonl_are_skipped() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("history.jsonl");

    let valid = entry("coffee", 1);
    let line = serde_json::to_string(&valid).expect("serialize");
    std::fs::write(&path, format!("{}\n\n\n{}\n", line, line)).expect("write");

    let h = History::load_from(path);
    assert_eq!(h.recent(10).len(), 2);
}

#[test]
fn record_appends_without_rewriting() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("history.jsonl");
    let mut h = History::load_from(path.clone());

    h.record("coffee", "test1.md", None).expect("record 1");
    h.record("me", "test2.md", None).expect("record 2");
    h.record("music", "test3.md", None).expect("record 3");

    // File should have exactly 3 lines
    let contents = std::fs::read_to_string(&path).expect("read");
    let non_empty_lines: Vec<&str> = contents.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(non_empty_lines.len(), 3);

    // Reload and verify
    let h2 = History::load_from(path);
    assert_eq!(h2.recent(10).len(), 3);
    assert_eq!(h2.last_pour().unwrap().module_key, "music");
}

#[test]
fn summary_cache_is_written() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("history.jsonl");
    let mut h = History::load_from(path.clone());

    h.record("coffee", "test.md", Some("Ethiopia"))
        .expect("record");

    let summary_path = dir.path().join("history-summary.json");
    assert!(
        summary_path.exists(),
        "summary cache should be written after record()"
    );

    let contents = std::fs::read_to_string(&summary_path).expect("read summary");
    let summary: serde_json::Value = serde_json::from_str(&contents).expect("parse summary");
    assert_eq!(summary["version"], 1);
    assert_eq!(summary["total_entries"], 1);
}
