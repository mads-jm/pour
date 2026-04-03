---
tags:
  - architecture
  - overview
date created: Tuesday, March 31st 2026, 10:04:14 pm
date modified: Friday, April 3rd 2026, 4:11:41 am
---

# System Architecture Overview

The codebase strictly separates concerns to isolate terminal drawing from data logic. This note is the short-form companion to [[pour-design-spec]].

* `src/main.rs`: Entry point. Owns CLI parsing, config load, terminal lifecycle, event loop polling, and orchestrates submit/cache persistence. See also [[ADR-003-Synchronous-TUI-Async-Operations]].
* `src/init.rs`: First-run setup. Implements the `pour init` flow — generates a starter `config.toml` with interactive vault path selection and example modules.
* `src/tui/`: Presentation layer. Routes events to screen handlers (`dashboard.rs`, `form.rs`, `summary.rs`, `configure.rs`) and dispatches `Action` enums. The dashboard acts as an ambient capture surface — showing recent activity, capture rhythm stats, and module gaps rather than a simple launcher. Built with [[ratatui]] and [[crossterm]].
* `src/tui/configure.rs`: In-app configurator. Provides a TUI form for editing module scalar fields (path, mode, display_name, append_under_header, callout_type) with a vault directory browser for path selection, and a QuickSelect picker for callout types. Also hosts vault-level settings (`ConfigureLevel::VaultSettings`) accessible via `Ctrl+V` from the dashboard.
* `src/app.rs`: State management. Owns `FormState`, `ConfigureState`, `BrowserState`, active field indices, and input validation.
* `src/output/`: Write execution. Orchestrates `frontmatter.rs` generation and `template.rs` path/template rendering (including `{{callout}}` resolution and field-level callout wrapping). Related: [[ADR-002-Custom-YAML-Serialization]].
* `src/data/`: Fetch, cache, and history tier. `cache.rs` backs dynamic select dropdowns; `history.rs` tracks capture events (timestamp, module, vault path) persisted at `~/.cache/pour/history.json` and surfaces ambient stats on the dashboard (last pour, today/week counts, streak, per-module activity, gaps). Related: [[The-3-Tier-Data-Fallback]].
* `src/transport/`: Network/disk boundary. Hides the complexity of API vs filesystem from the rest of the application. Exposes `execute_command()` for firing Obsidian plugin commands via the REST API `/commands/` endpoint (no-op on filesystem transport). Related: [[ADR-001-Hybrid-Transport-Layer]].
* `src/autocreate.rs`: Inline note creation. On form submit, scans `dynamic_select` fields with `allow_create = true` for novel values (not in the existing options list), sanitizes the value into a safe cross-platform filename, and creates a note via the transport layer. Updates the in-memory cache on success. Supports two creation modes: __bare stub__ (minimal `date`-only frontmatter) for fields without `create_template`, and __template-driven__ (full frontmatter from `[templates.<name>]` fields via sub-form overlay) for fields with `create_template`. Template path resolution expands strftime tokens before `{{name}}` substitution to prevent injection. Also handles `post_create_command` dispatch after successful template-driven creation.

For the integrated event loop and subsystem wiring, see [[sprint-6-integration-report]].





