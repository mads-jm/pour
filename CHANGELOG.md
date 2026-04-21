# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added

- `list` field option (default `false`) — when `true`, values containing `", "` split into a YAML list or into multiple wikilinks. Valid on `text`, `static_select`, and `dynamic_select`; rejected at load on other field types.
- Module-level duplicate field-name validation — `validate()` now errors on duplicate `name` entries within a module (previously only sub-field and template-field duplicates were checked).
- `BrowserState.error` — transport errors during directory listing now surface above the configure browser entry list instead of being silently swallowed.
- `App.deferred_stderr` — autocreate diagnostics emitted during TUI raw mode are buffered and printed after terminal restore on clean exit (known limitation: not drained on panic).
- Keyboard shortcut reference: `pour - docs/02 references/keyboard-shortcuts.md`.

### Changed

- **Breaking (behavior):** Field values containing `", "` now render as a literal string by default. Previously they were unconditionally split into YAML lists / multiple wikilinks. Set `list = true` on the field to restore the splitting behavior. Affects users who relied on implicit comma-splitting in v0.2.0 — their YAML output changes from a list to a quoted scalar until the flag is set.
- `ApiClient.base_url` is now private; use `ApiClient::base_url()` accessor.
- Composite-table rendering uses Unicode display width (`unicode-width`) instead of byte length, correcting column alignment for CJK / emoji cell values.
- Configure-screen label widths (`prefix_len`, `hint_len`, `dialog_width`) use Unicode display width.
- Form-screen prompt cursor position uses Unicode display width for the prompt prefix.
- Path references in `CLAUDE.md`, `resources/mads_vault_structure.md`, and architecture docs updated to `~/.pour/` + `POUR_HOME`.
- Design spec sections renumbered 6→5, 7→6 (had skipped §5).
- Keybinding documentation corrected: module-settings / vault-settings dashboard hotkeys are `e` / `v`, not `Ctrl+E` / `Ctrl+V`.

### Fixed

- P0: Comma-containing field values no longer explode into YAML lists unconditionally (see `list` flag above).
- P0: Duplicate field names at module level now fail validation with a clear error.
- P1: Cursor drift on non-ASCII form prompts (byte-length vs display-width).
- P1: `eprintln!` calls in `autocreate.rs` no longer corrupt the TUI when raw mode is active.
- P1: Browse errors in the configure directory picker now display instead of showing an empty list.
- P1: Stale browse error is cleared immediately when a new listing is dispatched (no longer lingers during async re-fetch).
- P2: Replaced `expect()` in config TOML array-of-tables access and cycle-detection with fallible error paths / invariant-documented `expect`.
- P2: Replaced dead `BrowserState.loading` field.

### Docs

- New `CHANGELOG.md`.
- `CLAUDE.md` and `resources/mads_vault_structure.md` path references updated.
- Added `src/paths.rs` and `src/util.rs` to the architecture overview.
- Added ADR-001 through ADR-004 descriptions to the ADR index.
- Added append-mode FS fallback behavior section.
- Added `pour init` step to README Quick Start; README `api_key` reference points to `secrets.toml`.
- Added `open` and `percent-encoding` to tech-stack tables.
- Documented `date_format` in the field-types reference.

### Known Limitations

- `App.deferred_stderr` is not drained from the panic hook — autocreate diagnostics may be lost on panic.
- Form value-side cursor math still uses byte length (`cursor_position` is a byte offset); CJK / emoji glyphs in field *values* can drift cursor position. Prompt-side is correct.
- `date_format` affects `path` rendering only; `render_append_template` hardcodes `%Y-%m-%d` for the `{{date}}` token.

---

## [0.2.0] — 2026-04-07

### Added

- `secrets.toml` — API key now lives in `~/.pour/secrets.toml`, separate from `config.toml`. Auto-migrated from `config.toml` on first load. Resolution priority: `POUR_API_KEY` env var > `secrets.toml` > `config.toml`.
- `percent-encoding` for API URLs — vault paths containing spaces are now correctly encoded in REST API requests.
- `config_version` serde default via `#[serde(default)]` — configs without `config_version` parse cleanly as `"0.1.0"`.
- Vault settings rebuild now includes `date_format` row in the configurator.
- `add_field_on_disk` serializes `show_when`, `allow_create`, `wikilink`, `create_template`, and `post_create_command` when writing fields back to `config.toml`.
- `validate_form` skips hidden (non-visible) fields during submit validation.
- `partition_fields` filters hidden field values from frontmatter and body output.
- `clamp_active_to_visible` — form focus recovers correctly when the visible field set shrinks.
- Guard against duplicate YAML key when a user field is named `"icon"`.
- Extensible `static_select` — `allow_create = true` accepts and persists novel values to `config.toml`.

### Changed

- All runtime state moved from `~/.config/pour/` + `~/.cache/pour/` to `~/.pour/` (`POUR_HOME` override supported).
- `std::fs::rename`-based atomic writes replaced by `util::atomic_replace` — fixes data loss on Windows cross-device renames.
- `serde_yaml` removed; YAML frontmatter uses custom serialization (see ADR-002).
- Default config placeholder syntax changed to `{{bean}}` style.
- Version bump to `0.2.0`.

### Fixed

- Leading zeros in `config_version` are now rejected at parse time.
- Empty `one_of = []` in `show_when` is rejected at parse time.

### Known Bugs (open at release)

- Strftime injection — `render_path` expands `%` tokens in user-supplied values.
- YAML escaping — `format_scalar` does not escape newlines, backslashes, or YAML reserved words.
- Comma-in-value splits to list — `", "` in icon/field values produces a YAML list instead of a scalar.
- Duplicate field names at module level cause silent HashMap overwrite (sub-field/template duplicates are checked).
- FS transport has no `..` check at the transport layer (config validates at load, no defense-in-depth).
- `prefix_len` uses byte width — non-ASCII prompts cause cursor drift.
- `eprintln!` during raw mode in `autocreate.rs` produces garbled output.

---

## [0.1.0] — 2026-04-06

### Added

- Config-driven module system: TOML `config.toml` → typed structs → validated at load.
- Hybrid transport layer: Obsidian Local REST API (`https://127.0.0.1:27124`) with automatic filesystem fallback (see ADR-001).
- Two write modes: `append` (insert under heading in existing note) and `create` (new file with YAML frontmatter).
- Custom YAML frontmatter generation — Obsidian Properties compatible, no `serde_yaml` dependency (see ADR-002).
- Template rendering: strftime tokens (`%Y`, `%m`, `%d`, `%H`, `%M`, `%S`) and field name placeholders (`{{field_name}}`) in paths and append templates.
- `dynamic_select` field type with 3-tier data fallback: API directory listing → filesystem scan → JSON cache (`~/.pour/cache/state.json`) → freetext input.
- Background async refresh of dynamic select options while TUI renders from cache.
- `static_select` field type with hardcoded options list.
- `text`, `number`, `textarea` field types.
- `composite_array` field type — tabular sub-field data entry, serialized as YAML array.
- `show_when` conditional visibility — gates field rendering, navigation, validation, and output on another field's value.
- `allow_create` on `dynamic_select` — inline novel-value entry with auto-created bare stub notes.
- `create_template` + `[templates]` — sub-form overlay for structured inline note creation.
- `post_create_command` — fires an Obsidian plugin command after template-driven note creation.
- `wikilink` field option — wraps output value in `[[...]]` Obsidian wikilink syntax.
- `preset_exclude` field option — excludes a field from preset capture and application.
- Per-module named presets persisted to `~/.pour/presets.json`.
- Capture history persisted to `~/.pour/cache/history.jsonl` (append-only JSONL, O(1) writes) with precomputed `history-summary.json` for dashboard stats.
- Dashboard: ambient stats (last pour, today/week counts, streak, per-module activity, gap indicators), module list, connection status header.
- In-app configurator (`e` / `v` from dashboard): edit module scalar fields and vault settings without leaving the TUI. Uses `toml_edit` for comment-preserving atomic writes.
- Vault directory browser for path selection inside the configurator.
- `pour init` — first-run setup generating a starter `config.toml` with interactive vault path selection and example modules.
- CLI fast-path: `pour <module>` bypasses dashboard and launches directly into a module form.
- Post-submit summary view (Enter → menu, A → pour another, Q → quit).
- Panic hook restores terminal cleanly on unexpected exit.
- `callout_type` on modules and `callout` on `textarea` fields — wraps body output in Obsidian `> [!type]` blockquote syntax.
- `append_shallow` option — shallow insert for compatibility with daily note plugins.
- `daily_link` module config key.
- `POUR_CONFIG` env var for pointing at an alternate config file (used in tests).

### Design Decisions

- Synchronous event loop (draw/poll/handle per tick) — async would add complexity without meaningful UX gain for v0.1 (see ADR-003).
- `std::fs::rename` for atomic cache writes (broken on Windows; fixed in v0.2 by `util::atomic_replace`).
- Comma-in-value heuristic: `", "` in field output splits into a YAML list — intentional for multi-select, breaks literal commas.
