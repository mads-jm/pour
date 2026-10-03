---
tags:
  - architecture
  - adr
  - tui
date created: Tuesday, March 31st 2026, 10:03:30 pm
date modified: Friday, October 2nd 2026, 11:00:00 pm
---

# ADR 003: Blocking UI During Async Transport

__Date:__ 2026-03-31  
__Status:__ Accepted (v1 specific)

__Context:__  
The `main.rs` event loop uses `tokio` for async data fetching and API submissions. Managing true background async in a TUI requires complex channels and shared state to prevent UI freezing while waiting for network responses.

__Decision:__  
For v1, `fetch_dynamic_options` and `handle_submit` calls are `await`ed inline, effectively blocking the UI thread during network operations.

__Consequences:__  
Acceptable tradeoff for v1 velocity. Filesystem writes are near-instantaneous, and API calls enforce a strict 5-second timeout. True non-blocking UI is deferred to a future epic.

__Note (2026-10-02):__ Still in force after v1. The inline awaits now also include `fetch_current_values` and `resolve_priors` at form open, and the `post_write_shell` hook inside `handle_submit`. The hook has a 30-second timeout, so a slow hook can hold the UI far longer than the 5-second API timeout this ADR assumed. The completion sound plays on its own thread and does not block.

See also [[System-Architecture-Overview]], [[ratatui]], and [[v0.1.0-report]] (sprint 6).




