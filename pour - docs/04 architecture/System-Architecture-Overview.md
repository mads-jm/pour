---
tags:
  - architecture
  - overview
date created: Tuesday, March 31st 2026, 10:04:14 pm
date modified: Tuesday, April 7th 2026, 3:30:34 am
---

# System Architecture Overview

The codebase strictly separates concerns to isolate terminal drawing from data logic. This note is the short-form companion to [[pour-design-spec]].

* `src/main.rs`: Entry point. Owns CLI parsing, config load, terminal lifecycle, event loop polling, and orchestrates submit/cache persistence. See also [[ADR-003-Synchronous-TUI-Async-Operations]].
* `src/init.rs`: First-run setup. Implements the `pour init` flow — generates a starter `config.toml` with interactive vault path selection and example modules.
* `src/tui/`: Presentation layer. Routes events to screen handlers (`dashboard.rs`, `form.rs`, `summary.rs`, `configure.rs`) and dispatches `Action` enums. The dashboard acts as an ambient capture surface — showing recent activity, capture rhythm stats, and module gaps rather than a simple launcher. Built with [[ratatui]] and [[crossterm]].
* `src/tui/configure.rs`: In-app configurator. Provides a TUI form for editing module scalar fields (path, mode, display_name, append_under_header, callout_type, icon) with a vault directory browser for path selection, and a QuickSelect picker for callout types. Also hosts vault-level settings (`ConfigureLevel::VaultSettings`) accessible via `Ctrl+V` from the dashboard.
* `src/app.rs`: State management. Owns `FormState`, `ConfigureState`, `BrowserState`, active field indices, and input validation. `FormState.active_field` is a __visible-set index__ (into the list returned by `visible_field_indices`); `active_config_idx` mirrors it as the config-level index and is used by `clamp_active_to_visible` to correctly recover focus when the visible set shrinks. `FormState.callout_overrides` holds per-entry callout type selections (seeded from config, cyclable via Left/Right on textarea fields).
* `src/visibility.rs`: Conditional field visibility. `visible_field_indices(fields, values)` returns the subset of config field indices that are currently visible given `show_when` rules. Called on every form key event to keep navigation bounded to visible fields. Integration points: `src/app.rs` uses `visible_field_indices` to compute `FormState.active_field` bounds and clamp focus on visibility change; `src/output/` skips hidden fields during frontmatter/body generation; `src/config.rs` validates `show_when` constraints (no self-reference, no circular chains, no `composite_array` controllers) at load time; `src/tui/configure.rs` serializes `show_when` when writing fields back to disk via `add_field_on_disk` / `update_field_on_disk`.
* `src/output/`: Write execution. Orchestrates `frontmatter.rs` generation and `template.rs` path/template rendering (including `{{callout}}` resolution and field-level callout wrapping). Both create mode (`partition_fields`) and append mode (`render_append_template`) apply callout wrapping; runtime overrides from `FormState.callout_overrides` take precedence over config values. Related: [[ADR-002-Custom-YAML-Serialization]].
* `src/data/`: Fetch, cache, and history tier. `cache.rs` backs dynamic select dropdowns; `history.rs` tracks capture events (timestamp, module, vault path) persisted at `~/.pour/cache/history.jsonl` (append-only JSONL — one JSON object per line, O(1) writes) with a precomputed `history-summary.json` cache for instant dashboard rendering. Surfaces ambient stats on the dashboard (last pour, today/week counts, streak, per-module activity, gaps). Auto-migrates from the legacy single-array `history.json` format on first load. `presets.rs` stores and retrieves per-module saved field-value sets persisted at `~/.pour/presets.json`. Related: [[The-3-Tier-Data-Fallback]].
* `src/transport/`: Network/disk boundary. Hides the complexity of API vs filesystem from the rest of the application. Exposes `execute_command()` for firing Obsidian plugin commands via the REST API `/commands/` endpoint (no-op on filesystem transport). Related: [[ADR-001-Hybrid-Transport-Layer]].
* `src/paths.rs`: Centralized path resolution. All runtime file locations (`~/.pour/`, `config.toml`, `secrets.toml`, `presets.json`, `cache/`) are computed here via `pour_home()` and friends. `POUR_HOME` env var overrides the base directory. No other module constructs these paths manually.
* `src/util.rs`: Cross-platform atomic file replacement. `atomic_replace(src, dst)` moves a temp file to its final destination safely — works around `std::fs::rename` failing across volumes on Windows.
* `src/autocreate.rs`: Inline note creation. On form submit, scans `dynamic_select` fields with `allow_create = true` for novel values (not in the existing options list), sanitizes the value into a safe cross-platform filename, and creates a note via the transport layer. Updates the in-memory cache on success. Supports two creation modes: __bare stub__ (minimal `date`-only frontmatter) for fields without `create_template`, and __template-driven__ (full frontmatter from `[templates.<name>]` fields via sub-form overlay) for fields with `create_template`. Template path resolution expands strftime tokens before `{{name}}` substitution to prevent injection. Also handles `post_create_command` dispatch after successful template-driven creation.

## Append-Mode Filesystem Fallback

When a module uses `mode = "append"` and the API is unavailable, the filesystem transport cannot perform a true in-place append because it has no read-modify-write mechanism for arbitrary heading targets without risking data loss. Instead, Pour falls back to creating a standalone __atomic note__ with a timestamped filename (e.g. `20260421-143022-me.md`) at the module's configured path directory. The note contains the rendered `append_template` output as its body. This ensures zero data loss — the entry is always persisted — even when the target daily note is inaccessible. The user can manually merge the standalone note into their daily note if desired.

This behavior is documented in [[ADR-001-Hybrid-Transport-Layer]] and [[ADR-004-API-Append-Read-Modify-Write]].

For the integrated event loop and subsystem wiring, see [[sprint-6-integration-report]].





