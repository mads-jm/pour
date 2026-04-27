// Engine-level unit tests for `History::filter` (§6.5 cursor pagination).
//
// Validates the filter logic directly without going through the HTTP layer.
// Covers: empty filter, since-only, until-only, both, module filter, limit
// pagination, and the same-millisecond cursor correctness fix.

use chrono::{DateTime, TimeZone, Utc};
use pour::data::history::{History, HistoryEntry};
use std::io::Write;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_entry(
    id: &str,
    module_key: &str,
    timestamp: DateTime<Utc>,
) -> HistoryEntry {
    HistoryEntry {
        id: Some(id.to_string()),
        module_key: module_key.to_string(),
        timestamp,
        vault_path: format!("test/{id}.md"),
        first_field: None,
    }
}

fn history_with(entries: Vec<HistoryEntry>) -> (History, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.jsonl");
    let mut file = std::fs::File::create(&path).unwrap();
    for e in &entries {
        let line = serde_json::to_string(e).unwrap();
        writeln!(file, "{}", line).unwrap();
    }
    file.flush().unwrap();
    (History::load_from(path), dir)
}

fn ts(year: i32, month: u32, day: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, h, m, s).unwrap()
}

// ---------------------------------------------------------------------------
// Empty filter
// ---------------------------------------------------------------------------

#[test]
fn filter_no_constraints_returns_all_descending() {
    let t1 = ts(2026, 4, 20, 10, 0, 0);
    let t2 = ts(2026, 4, 21, 10, 0, 0);
    let t3 = ts(2026, 4, 22, 10, 0, 0);
    let (h, _dir) = history_with(vec![
        make_entry("20260420T100000000-0-coffee", "coffee", t1),
        make_entry("20260421T100000000-0-coffee", "coffee", t2),
        make_entry("20260422T100000000-0-coffee", "coffee", t3),
    ]);

    let (entries, has_more, next_cursor) = h.filter(None, None, None, None, 10);
    assert_eq!(entries.len(), 3);
    assert!(!has_more);
    assert!(next_cursor.is_none());
    // Descending by id — most recent first.
    assert_eq!(entries[0].id.as_deref().unwrap(), "20260422T100000000-0-coffee");
    assert_eq!(entries[2].id.as_deref().unwrap(), "20260420T100000000-0-coffee");
}

#[test]
fn filter_empty_history_returns_empty() {
    let (h, _dir) = history_with(vec![]);
    let (entries, has_more, next_cursor) = h.filter(None, None, None, None, 10);
    assert!(entries.is_empty());
    assert!(!has_more);
    assert!(next_cursor.is_none());
}

// ---------------------------------------------------------------------------
// since-only filter
// ---------------------------------------------------------------------------

#[test]
fn filter_since_inclusive_lower_bound() {
    let t1 = ts(2026, 4, 20, 10, 0, 0);
    let t2 = ts(2026, 4, 21, 10, 0, 0);
    let t3 = ts(2026, 4, 22, 10, 0, 0);
    let (h, _dir) = history_with(vec![
        make_entry("20260420T100000000-0-coffee", "coffee", t1),
        make_entry("20260421T100000000-0-coffee", "coffee", t2),
        make_entry("20260422T100000000-0-coffee", "coffee", t3),
    ]);

    // since = t2: should return t2 and t3 (inclusive).
    let (entries, _, _) = h.filter(Some(t2), None, None, None, 10);
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.timestamp >= t2));
}

// ---------------------------------------------------------------------------
// until-only filter
// ---------------------------------------------------------------------------

#[test]
fn filter_until_exclusive_upper_bound() {
    let t1 = ts(2026, 4, 20, 10, 0, 0);
    let t2 = ts(2026, 4, 21, 10, 0, 0);
    let t3 = ts(2026, 4, 22, 10, 0, 0);
    let (h, _dir) = history_with(vec![
        make_entry("20260420T100000000-0-coffee", "coffee", t1),
        make_entry("20260421T100000000-0-coffee", "coffee", t2),
        make_entry("20260422T100000000-0-coffee", "coffee", t3),
    ]);

    // until = t3: should return t1 and t2 (t3 excluded).
    let (entries, _, _) = h.filter(None, Some(t3), None, None, 10);
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.timestamp < t3));
}

// ---------------------------------------------------------------------------
// since + until (windowed query)
// ---------------------------------------------------------------------------

#[test]
fn filter_since_and_until_window() {
    let t1 = ts(2026, 4, 20, 10, 0, 0);
    let t2 = ts(2026, 4, 21, 10, 0, 0);
    let t3 = ts(2026, 4, 22, 10, 0, 0);
    let t4 = ts(2026, 4, 23, 10, 0, 0);
    let (h, _dir) = history_with(vec![
        make_entry("20260420T100000000-0-coffee", "coffee", t1),
        make_entry("20260421T100000000-0-coffee", "coffee", t2),
        make_entry("20260422T100000000-0-coffee", "coffee", t3),
        make_entry("20260423T100000000-0-coffee", "coffee", t4),
    ]);

    // since=t2, until=t4 → t2, t3 (t4 excluded).
    let (entries, _, _) = h.filter(Some(t2), Some(t4), None, None, 10);
    assert_eq!(entries.len(), 2);
    let ids: Vec<&str> = entries.iter().map(|e| e.id.as_deref().unwrap()).collect();
    // Descending.
    assert!(ids[0] > ids[1]);
}

// ---------------------------------------------------------------------------
// Module filter
// ---------------------------------------------------------------------------

#[test]
fn filter_module_filters_correctly() {
    let t1 = ts(2026, 4, 20, 10, 0, 0);
    let t2 = ts(2026, 4, 21, 10, 0, 0);
    let t3 = ts(2026, 4, 22, 10, 0, 0);
    let (h, _dir) = history_with(vec![
        make_entry("20260420T100000000-0-coffee", "coffee", t1),
        make_entry("20260421T100000000-0-me", "me", t2),
        make_entry("20260422T100000000-0-coffee", "coffee", t3),
    ]);

    let (entries, _, _) = h.filter(None, None, None, Some("coffee"), 10);
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.module_key == "coffee"));
}

// ---------------------------------------------------------------------------
// Limit + cursor pagination
// ---------------------------------------------------------------------------

#[test]
fn filter_limit_produces_has_more_and_cursor() {
    let entries: Vec<HistoryEntry> = (0..5)
        .map(|i| {
            let t = ts(2026, 4, 20 + i, 10, 0, 0);
            make_entry(&format!("2026042{}T100000000-0-coffee", i), "coffee", t)
        })
        .collect();
    let (h, _dir) = history_with(entries);

    let (page, has_more, next_cursor) = h.filter(None, None, None, None, 3);
    assert_eq!(page.len(), 3);
    assert!(has_more);
    assert!(next_cursor.is_some());
}

#[test]
fn filter_cursor_pages_through_all_entries() {
    let t1 = ts(2026, 4, 20, 10, 0, 0);
    let t2 = ts(2026, 4, 21, 10, 0, 0);
    let t3 = ts(2026, 4, 22, 10, 0, 0);
    let t4 = ts(2026, 4, 23, 10, 0, 0);
    let (h, _dir) = history_with(vec![
        make_entry("20260420T100000000-0-coffee", "coffee", t1),
        make_entry("20260421T100000000-0-coffee", "coffee", t2),
        make_entry("20260422T100000000-0-coffee", "coffee", t3),
        make_entry("20260423T100000000-0-coffee", "coffee", t4),
    ]);

    let (page1, has_more, cursor) = h.filter(None, None, None, None, 2);
    assert_eq!(page1.len(), 2);
    assert!(has_more);
    let cursor_str = cursor.unwrap();

    let (page2, has_more2, _) = h.filter(None, None, Some(&cursor_str), None, 2);
    assert_eq!(page2.len(), 2);
    assert!(!has_more2);

    // All 4 distinct ids across both pages.
    let mut all: Vec<&str> = page1
        .iter()
        .chain(page2.iter())
        .map(|e| e.id.as_deref().unwrap())
        .collect();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), 4, "all 4 entries must appear exactly once");
}

// ---------------------------------------------------------------------------
// Same-millisecond entries: cursor must not drop any
// ---------------------------------------------------------------------------

#[test]
fn filter_same_ms_cursor_returns_all_entries() {
    // Three entries sharing the same timestamp but different counter suffix.
    let t = ts(2026, 4, 25, 12, 0, 0);
    let (h, _dir) = history_with(vec![
        make_entry("20260425T120000000-0-coffee", "coffee", t),
        make_entry("20260425T120000000-1-coffee", "coffee", t),
        make_entry("20260425T120000000-2-coffee", "coffee", t),
    ]);

    // Page 1: limit=2
    let (page1, has_more, cursor) = h.filter(None, None, None, None, 2);
    assert_eq!(page1.len(), 2, "page 1 must have 2 entries");
    assert!(has_more, "must have more entries");
    let cursor_str = cursor.expect("cursor must be set");

    // Page 2: use the cursor
    let (page2, has_more2, _) = h.filter(None, None, Some(&cursor_str), None, 2);
    assert_eq!(page2.len(), 1, "page 2 must have 1 remaining entry");
    assert!(!has_more2);

    // All 3 ids must appear across both pages.
    let all: Vec<&str> = page1
        .iter()
        .chain(page2.iter())
        .map(|e| e.id.as_deref().unwrap())
        .collect();
    assert!(
        all.contains(&"20260425T120000000-0-coffee"),
        "entry 0 missing; all={all:?}"
    );
    assert!(
        all.contains(&"20260425T120000000-1-coffee"),
        "entry 1 missing; all={all:?}"
    );
    assert!(
        all.contains(&"20260425T120000000-2-coffee"),
        "entry 2 missing; all={all:?}"
    );
}
