---
tags:
  - architecture
  - overview
date created: Tuesday, March 31st 2026, 10:04:14 pm
date modified: Tuesday, March 31st 2026, 10:34:07 pm
---

# System Architecture Overview

The codebase strictly separates concerns to isolate terminal drawing from data logic. This note is the short-form companion to [[pour-design-spec]].

* `src/main.rs`: Entry point. Owns CLI parsing, config load, terminal lifecycle, event loop polling, and orchestrates submit/cache persistence. See also [[ADR-003-Synchronous-TUI-Async-Operations]].
* `src/tui/`: Presentation layer. Routes events to screen handlers (`dashboard.rs`, `form.rs`, `summary.rs`) and dispatches `Action` enums. Built with [[ratatui]] and [[crossterm]].
* `src/app.rs`: State management. Owns `FormState`, active field indices, and input validation.
* `src/output/`: Write execution. Orchestrates `frontmatter.rs` generation and `template.rs` path rendering. Related: [[ADR-002-Custom-YAML-Serialization]].
* `src/data/`: Fetch and cache tier. Related: [[The-3-Tier-Data-Fallback]].
* `src/transport/`: Network/disk boundary. Hides the complexity of API vs filesystem from the rest of the application. Related: [[ADR-001-Hybrid-Transport-Layer]].

For the integrated event loop and subsystem wiring, see [[sprint-6-integration-report]].

