---
tags:
  - sprint-report
  - v1
date created: Tuesday, March 31st 2026, 7:22:06 pm
sprint: 6
status: complete
date modified: Friday, April 3rd 2026, 4:11:47 am
---

# Sprint 6 Report: Integration

## Objective

Wire all subsystems together into a working binary so that `cargo run` launches a TUI dashboard and `cargo run -- <module>` opens a form for direct entry. This is the final sprint of v1.

## Tasks Completed

### TASK-016: Main Event Loop (`src/main.rs`)

- `#[tokio::main]` async entry point
- CLI arg parsing: no args = dashboard, one arg = fast-path direct to form
- Config load via `Config::load()` with user-friendly stderr error + exit(1) on failure
- Transport connect via `Transport::connect(&config).await` (auto-fallback from API to filesystem)
- `App::new(config, transport)` builds central state
- Fast path: validates module name exists, sets `selected_module` index, calls `init_form()`, fetches dynamic_select options
- Unknown module name prints available modules to stderr and exits with code 1
- Panic hook: `std::panic::set_hook` restores terminal before printing panic info
- Terminal lifecycle: `ratatui::init()` / `ratatui::restore()` with clean teardown on all exit paths
- Main loop in `run_loop()`:
  - `terminal.draw(|frame| tui::render(&app, frame))`
  - `event::poll(100ms)` + `event::read()` for crossterm events
  - Ctrl+C always breaks cleanly
  - Routes key events through `tui::handle_event(&mut app, key_event)`
  - `Action::Quit` breaks the loop
  - `Action::Navigate(Screen::Form)` triggers dynamic option fetch for new form
  - `Action::Submit` calls `handle_submit()` for validation + write + summary transition
  - `Action::None` continues

### TASK-019: Dynamic Select Integration

- `fetch_dynamic_options()` scans module fields for `FieldType::DynamicSelect`, collects `(field_name, source)` pairs
- Calls `data::fetch_options(&transport, &source, &mut cache)` for each, populating `form_state.field_options`
- Invoked on fast-path init AND on dashboard module selection (Navigate to Form)
- Cache loaded once at startup via `Cache::load()`, persisted after each write via `cache.save()` (best-effort, ignores save errors)

### TASK-020: Error Handling

- Config load failure: stderr message + exit code 1 (no TUI initialized)
- Unknown module fast-path: stderr with available module list + exit code 1
- Transport connect: already fallback-safe (API -> filesystem)
- Write failure on submit: `SummaryState` populated with error message, `file_path = None`, displayed as "Error" header in red on summary screen
- Validation failure on submit: errors stored in `form_state.validation_errors`, displayed inline in form footer, form stays open (no screen transition)
- Panic: hook restores terminal before printing panic, so the user sees the panic message in a clean terminal
- Main loop IO error: terminal restored, error printed to stderr, exit code 1

## Quality Gates

| Gate | Result |
|------|--------|
| `cargo check` | Pass |
| `cargo test -- --test-threads=1` | 80 tests passing (9 app, 14 config, 11 data, 20 output, 26 transport) |
| `cargo clippy` | 0 warnings |
| `cargo fmt -- --check` | Clean |
| `cargo build --release` | Pass |

## Architecture Notes

### Event Loop Design

The main loop is intentionally synchronous within each iteration: draw, poll, handle, act. The `fetch_dynamic_options` and `handle_submit` calls are `await`ed inline, which means the UI blocks during transport operations. This is acceptable for v1 because:
1. Filesystem writes are near-instant
2. API calls have a 5-second timeout
3. True background async would require channels or `tokio::spawn` + shared state, adding complexity disproportionate to the UX gain

### Separation of Concerns

- `main.rs` owns: CLI parsing, config load, terminal lifecycle, event loop, submit orchestration, cache persistence
- `tui/mod.rs` owns: event routing to screen handlers, action dispatch
- `app.rs` owns: form initialization, validation
- `output/` owns: write execution (frontmatter, template, transport calls)
- `data/` owns: option fetching with 3-tier fallback, cache storage

### What's NOT in V1

- Background async refresh of dynamic selects (options load before first render, but don't refresh in background)
- Multiple-argument CLI support (only one module at a time)
- Logging/tracing (structured logging is a future sprint)
- Undo/confirmation before write

## Files Changed

- `src/main.rs` — complete rewrite from placeholder to full integration

## Test Coverage Note

The main event loop and submit handler are inherently difficult to unit test (they own terminal state and async I/O). All component logic they call into (config, app, transport, output, data) has thorough test coverage. Manual verification: `cargo run` launches dashboard, `cargo run -- <module>` opens form directly.




