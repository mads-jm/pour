---
tags:
  - architecture
  - adr
  - tui
date created: Tuesday, March 31st 2026, 10:03:30 pm
date modified: Friday, April 3rd 2026, 4:11:47 am
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

See also [[System-Architecture-Overview]], [[ratatui]], and [[sprint-6-integration-report]].




