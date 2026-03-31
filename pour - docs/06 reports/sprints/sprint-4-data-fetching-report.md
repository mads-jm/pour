---
tags:
  - sprint-report
  - v1
date created: Tuesday, March 31st 2026, 6:40:22 pm
sprint: 4
status: complete
date modified: Tuesday, March 31st 2026, 10:34:08 pm
---

# Sprint 4 Report: Data Fetching

## Objective

Implement the cache layer and dynamic data fetching with 3-tier fallback (transport -> cache -> empty) for populating dynamic select dropdowns.

## Tasks Completed

### TASK-010: Cache Layer (`src/data/cache.rs`)

- `Cache` struct backed by `~/.cache/pour/state.json`
- `load()` / `load_from()` — returns empty cache on missing or corrupt file (no panics)
- `get(source)` — returns `Option<Vec<String>>` for a source path
- `set(source, items)` — updates entry with UTC timestamp via chrono
- `save()` — atomic persist to disk (temp file + rename), creates parent directories
- Uses `serde_json` for serialization, `dirs::cache_dir()` for platform path
- __6 tests__: round-trip, missing file, corrupt file, parent dir creation, unknown source, overwrite

### TASK-011: Dynamic Data Fetching (`src/data/mod.rs`)

- `fetch_options(transport, source_path, cache) -> Vec<String>` — async 3-tier fallback
- Tier 1: `transport.list_directory()` (API with FS fallback handled by transport layer)
- Tier 2: `cache.get()` for previously fetched items
- Tier 3: empty vec (TUI will offer freetext input)
- Results normalized to file stems before caching (strips `.md` extension, filters directory entries)
- On successful transport fetch, updates cache in-memory (caller saves)
- __5 tests__: transport success + cache population, normalization, transport fail -> cache fallback, both miss -> empty, empty dir -> cache fallback

### Bonus Fix: Transport Dispatcher

- `Transport::append_under_heading()` now delegates to heading-aware `FsWriter::append_under_heading()` instead of plain `append_to_file()`
- Updated dispatcher test to use a file with actual heading content

## Quality Gates

| Gate | Result |
|------|--------|
| `cargo check` | Clean |
| `cargo test -- --test-threads=1` | __71 passed__, 0 failed |
| `cargo clippy` | Clean |
| `cargo fmt -- --check` | Clean |

## Test Coverage Summary

| Module | Test File | Count |
|--------|-----------|-------|
| config | `tests/config.rs` | 14 |
| data/cache | `tests/data/cache.rs` | 6 |
| data/fetch | `tests/data/fetch.rs` | 5 |
| output/frontmatter | `tests/output/frontmatter.rs` | 7 |
| output/template | `tests/output/template.rs` | 8 |
| output/orchestration | `tests/output/orchestration.rs` | 5 |
| transport/api | `tests/transport/api.rs` | 5 |
| transport/fs | `tests/transport/fs.rs` | 10 |
| transport/dispatcher | `tests/transport/dispatcher.rs` | 6 |
| __Total__ | | __71__ |

## Inspector Findings and Fixes

1. __CRITICAL — API vs FS return shape mismatch__: `fetch_options` cached raw transport results without normalizing. API returns `"Ethiopia.md"`, FS returns `"Ethiopia"`. __Fixed__: added `normalize_items()` that strips `.md` extensions and filters directory entries before caching.
2. __MAJOR — Non-atomic cache save__: `Cache::save()` wrote directly, risking corruption on crash. __Fixed__: now uses temp file + rename (same pattern as `FsWriter`).
3. __MAJOR — No path traversal validation__: Config `source` field accepted `../../etc/secrets`. __Fixed__: `Config::validate()` now rejects source paths containing `..`. Added test.
4. __Minor — `default_cache_path()` relative fallback__: Falls back to `.cache` if `dirs::cache_dir()` returns None. Accepted for v1 (exotic platform edge case).
5. __Minor — `ApiClient::new` uses `.expect()`__: Pre-existing from Sprint 2. Deferred — `reqwest::Client::builder().build()` effectively never fails.

## Readiness for Sprint 5

All foundation layers are complete:
- __Config__ (Sprint 1) — types, loading, validation (incl. path traversal check)
- __Transport__ (Sprint 2) — API client, FS writer, heading-aware dispatcher with fallback
- __Output__ (Sprint 3) — frontmatter generation, template rendering, write orchestration
- __Data__ (Sprint 4) — cache layer, normalized dynamic fetch with 3-tier fallback

Sprint 5 (TUI) can begin: app state, dashboard, form, and summary views.

