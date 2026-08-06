# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added — habit capture v1 (frontmatter mutation primitives)

Spec: `pour - docs/08 specs/pour-habit-capture.md` §§2, 3, 5, 6. Four general primitives; **no habit-specific code** — the `habit` module is a plain config block.

- **`mode = "update"`** — a third write mode that merges frontmatter keys into an *existing* note resolved from a strftime `path`. The body is never touched, key order and quoting survive byte-for-byte, and no key outside the module's field list is rewritten. Pour never creates the note: over the API it fires the `daily-notes` command and retries the read once; over the filesystem it fails loudly. A note that exists but lacks the key gets it inserted plus a "your template is stale" notice — capture-first.
- **`toggle` and `counter` field types** — both default to `target = "frontmatter"` and are valid in any write mode. `toggle` is a bool (space flips it in the TUI). `counter` accumulates: `16` adds to the current value, `=16` sets it; a missing or `null` key reads as `0`, values parse as `f64`, and integral results emit bare. New `counter`-only keys `unit` (display-only) and `goal` (config-only reach-target, never written to the vault). `limit`/`limit_period` stay reserved and unimplemented.
- **`Transport::patch_frontmatter`** — one mutation primitive, two backends. API: `PATCH /vault/{path}` with `Operation: replace`, `Target-Type: frontmatter`, `Target: <key>`, `Create-Target-If-Missing: true` — pour's first PATCH request. Filesystem: a guarded surgical edit (capture mtime → read → replace exactly one line → re-verify mtime → `atomic_replace`) that aborts **without writing** on a mismatch — the stat precedes the read so a concurrent save landing *during* the read is caught too. A plugin too old to serve the PATCH degrades to the filesystem path instead of failing the capture. Multi-key updates are sequential and not atomic; a mid-write failure names the keys that already landed.
- **`output::frontmatter::read_frontmatter` / `patch_frontmatter_line`** — a read-only parser and a single-key line-level patcher, both pure and unit-tested. Deliberately not a YAML round-trip: parsing is safe, re-emitting is what destroys formatting. A key whose value spans multiple lines is refused rather than orphaned.
- **One-shot argv capture** — `pour <module> <field> [value]` writes without entering the TUI, echoes the resulting state (`water: 64/96 oz · ✓ 20260805.md`), and exits 0. Unknown module, unknown field, or a bad value token exits non-zero *before* the vault is touched. `pour <module>` with no field argument opens the TUI form exactly as before — **zero behavior change for existing modules**. Wired for `toggle`/`counter` fields on `update` modules only.
- **TUI widgets** — `toggle` renders `[x]`/`[ ]`; `counter` shows the note's current state alongside its input (`now 64/96 oz`), read once over the active transport at form open. A failed or slow read renders a placeholder rather than blocking or aborting the form (ADR-003). A toggle whose note value is not a boolean word is left unseeded, renders `[?]`, and is skipped on submit — pour does not coerce an unreadable value to `false` and then write that guess back.
- **Server-side `toggle`/`counter` validation** — a malformed value token submitted over the API comes back as a `400 validation_failed` with a `field`/`code` pair (`invalid_toggle`, `invalid_counter`), the same shape every other field type gets, instead of a `500 write_error`.
- **`habit` module in `resources/mads_config.toml`** — seed only, not mirrored into the live stowed config. `mobile_visible = false`: the PWA has no toggle/counter widget yet.
- **76 new tests**, densest around the one-way door: byte-for-byte preservation over a realistic daily note, a concurrent write landing between the baseline stat and the read aborting with the file untouched, multi-key writes landing in order and reporting partial application on failure, an unreadable toggle surviving a submit, API→filesystem degradation, missing-note and stale-template handling, and every new config validation rule.

### Changed

- Config validation rejects keys that another mode owns, rather than ignoring them: `unit`/`goal` off a `counter`; `append_under_header`/`append_template`/`append_shallow`/`daily_link`/`frontmatter_date_format`/`[modules.<n>.frontmatter]` on an `update` module; and `composite_array`, `list = true`, or a body target on an `update` module's fields.
- The configure editor's mode cycler gains `update` and its field-type cycler gains `toggle`/`counter`. Without this, auto-save silently rewrote an `update` module as `create` and a `counter` field as `text`.
- `check_paths` warns on a missing target note for `update` modules, as it already did for `append`.

## [1.0.0] — 2026-04-29 — The Freeze

### Fixed — v1 hardening (closes the v1.0.0 pre-release assessment Open Bugs list)

- **fs path traversal on the write path** — `src/transport/fs.rs::resolve_path_validated` now rejects `..` components, absolute paths, and `~`-prefixes on every public method (`create_file`, `append_to_file`, `append_under_heading`, `read_file`). Carry-over from v0.2.0; assessment Critical.
- **Windows atomicity for `atomic_replace`** — `src/transport/atomic.rs` now uses a single `std::fs::rename` call (which on Windows resolves to `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` — atomic at the OS level). The previously-`#[ignore]`d demo test in `tests/util_atomic.rs` is now an active regression guard.
- **char-indexed cursor** in form text/textarea/composite editors — emoji, CJK, and any non-ASCII character no longer panic on backspace or arrow-key movement. 13 sites converted across `src/tui/form/key/`.
- **strftime injection** in `src/output/template.rs` — literal `%` characters in user templates are no longer reinterpreted by chrono. Date/time access stays available via `{{date}}`, `{{time}}` placeholders.
- **`expect()` discipline** — three "module already verified above" sites in `src/config.rs` and one `reqwest client build` site in `src/transport/api.rs` now propagate as `Result`. `ApiClient::new` signature changed to `Result<Self, _>`.
- **Surfaced silent persistence failures** — 7 previously-swallowed `let _ = ...save()` patterns in `src/tui/loop_.rs` and `src/app.rs` now route through a new ephemeral status-bar toast. Failures appear as a one-line warning at the bottom of any TUI screen for 5 seconds.

### Added

- **`Config::edit()` transactional API** (`src/config_edit.rs`) — single load → mutate → validate → persist atomic operation. The 15 public `*_on_disk` mutator methods (e.g., `update_module_on_disk`, `add_field_on_disk`) are now thin facades that route through `edit()`; behavior preserved.
- **Generic `JsonStore<T>`** (`src/data/json_store.rs`) with optional migration hook — backs `Cache`, `Presets`, `FieldPresets` via a single sanctioned write path.
- **`transport::atomic::atomic_replace`** — the single sanctioned atomic-write primitive for the codebase. `util.rs` keeps a `pub use` for backwards compatibility.
- **`config_updates`** module — canonical home for `build_module_updates`, `build_vault_updates`, `build_field_updates`, `build_sub_field_updates` (formerly duplicated between `main.rs` and `tui/configure.rs`).
- **File-size budget CI ratchet** — `scripts/check-file-size.sh` fails CI on any `src/**/*.rs` over 800 LOC without a `// LINTOK: oversized: <reason>` annotation. Runs in `.github/workflows/ci.yml`.
- **102 new tests** pinning previously-untested behavior:
  - `tests/tui_configure.rs` — 52 cases (render dispatch, key routing per mode, auto-save, scroll sync, build_*_updates round-trips, browser nav, confirm dialog). Closes the assessment's "single biggest test hole heading into v1.0.0".
  - `tests/output/template_snapshot.rs` — 30 cases pinning `render_path` and `render_append_template`, including 8 documented intentional divergences.
  - `tests/util_atomic.rs` — 7 cases for the atomic-write primitive (1 originally `#[ignore]`d Windows demo; un-ignored after the v1 hardening fix).
  - `tests/data_json_store.rs` — 8 cases for the generic store + migration hook.
  - `tests/config_edit.rs` — 5 cases for the transactional `Config::edit()` (round-trip, rollback-on-error, draft.parsed access, validation rollback, no-op).

### Changed — decomposition (Phases 0–5)

The v0.3 → v1.0 window was the last cheap moment to split the god-modules before the public-surface freeze. Every entry below preserves observable behavior; tests (783 → 876 — none regressed) anchor the move.

- **`src/main.rs` 1945 LOC → 205 LOC** — bootstrap-only. Event loop and 21 action handlers moved to `src/tui/loop_.rs`. (Slice 13)
- **`src/tui/form.rs` 3852 LOC** split into `tui/form/` directory: `mod.rs` 523, `render/{mod,fields,composite}.rs`, `key/{mod,text,select,composite,navigation,submit}.rs`, `overlays/{mod,preset_picker,sub_form,small}.rs`. The 930-line `handle_key` is now mode-conditional across 6 small files. (Slices 11a/b/c)
- **`src/tui/configure.rs` 2478 LOC** split into `tui/configure/` directory: `mod.rs` 91, `render.rs`, `autosave.rs`, `init.rs`, `key/{mod,fields,sub_fields,vault,modules,presets}.rs`. (Slice 12)
- **`src/server/mod.rs` 528 LOC → 302 LOC** — static-asset serving moved to `server/static_assets.rs`; router construction moved to `server/routing.rs`. (Slice 7)
- **`src/server/dto.rs` 627 LOC** split into `server/dto/{mod,response,requests,mapping}.rs`. `mapping.rs` isolates the deep `Config → DTO` walk so `response.rs` is Config-free. (Slice 10)
- **`src/server/handlers/submit.rs` 607 LOC** split into `submit/{mod,validate,idempotency_lookup,autocreate_step,write_step,history_step}.rs` with a `SubmitContext` struct. (Slice 9)
- **`src/data/history.rs` 673 LOC → 474 LOC** — statistical computation moved to `history_summary.rs`; legacy-format reader moved to `history_legacy.rs`. (Slice 15)
- **`src/app.rs` 1308 LOC → 1011 LOC** — configure init helpers (`build_field_settings`, `build_sub_field_settings`, `init_vault_configure`, `init_new_module_configure`) moved to `tui/configure/init.rs`. Thin wrappers stay on `App` for test compat. (Slice 14)

### Changed — DRY collapses

- **18 atomic-write blocks → 1** in `src/config.rs` via `Config::write_atomic` (Slice 3) and the new transactional `Config::edit()` (Slice 8). Universal orphan `.tmp` cleanup on rename failure (was missing at 17 of 18 sites).
- **3 JSON-store implementations → 1** generic `JsonStore<T>` (Slice 2). Consolidates `Cache`, `Presets`, `FieldPresets` save/load.
- **`build_*_updates` duplication → 1 module** at `src/config_updates.rs` (Slice 5). `main.rs` -197 LOC, `tui/configure.rs` -190 LOC.
- **2 `{{key}}` substitution loops → 1 kernel** in `output/template.rs` (Slice 4). The 8 intentional divergences between `render_path` and `render_append_template` are now explicit configuration of the shared kernel.
- **Module-ordering helper** consolidated to `init::module_order()` (Slice 6).

### Changed — surface

- **`lib.rs` public surface curated** (Slice 17): `config_updates`, `transport::atomic`, `server::{dto,routing,static_assets}`, `tui::{loop_,render}` demoted to `pub(crate)`. Two `tui::loop_` entry points (`run_loop`, `fetch_dynamic_options`) re-exported. `cargo doc --no-deps` builds clean with zero warnings.
- **Inline `#[cfg(test)] mod tests`** in `src/autocreate.rs` removed; tests moved to `tests/autocreate.rs` per project convention. Last remaining inline test module is gone. (Slice 16)

## [0.3.0] — 2026-04-29 — Mobile / PWA Capture Surface

### Added — `pour serve` + PWA companion

- `pour serve` subcommand: HTTP server on port 8421 (default, `--port` overrides), `0.0.0.0` binding, QR code + URL printed at startup. Bearer-token auth via `mobile_token` in `~/.pour/secrets.toml` (auto-generated on first run); `?token=` bootstrap flow for QR scans.
- Nine `/api/v1/*` endpoints — `health`, `config`, `options`, `submit`, `captures`, `history`, `presets` (CRUD + reorder). Contract frozen in `pour - docs/08 specs/pour-api-contract.md`; OpenAPI at `pour - docs/02 references/pour-openapi.yaml`.
- Embedded PWA via `rust-embed` — vanilla HTML/CSS/JS shell with module list, dynamic form rendering, submit, history, "Add to Home Screen" support on iOS/Android.
- Idempotency layer — in-memory LRU (1024 cap), in-flight TTL 60 s, done TTL 5 min. `Idempotency-Key` header per contract §9 makes submit retries safe.
- Client-supplied `captured_at` ISO 8601 timestamp on submit preserves moment-of-capture from the phone.
- `mobile_visible = false` per-module opt-out from the phone interface.
- Phase 2 PWA scope (closed 2026-04-27): IndexedDB offline queue, service worker app-shell cache, sub-form overlay for `create_template` fields, preset mutation UI (save/edit/delete/reorder from PWA), 90-day history heatmap, bottom-tab navigation, cursor-paginated history list. Closeout report: `pour - docs/06 reports/v1.0.0-phase2-closeout.md`.

### Added — TUI ↔ Serve handoff

- Press `s` from the dashboard to suspend the TUI and run `pour serve` inline with the QR code visible in the terminal. Ctrl+C drains gracefully (5 s budget) and returns to the dashboard. One process, one terminal, transient server. Plan + audit notes: `pour - docs/08 specs/pour-tui-serve-handoff.md`.
- New `src/server/startup.rs` — shared banner + token resolution between `pour serve` CLI and the TUI handoff. Banner uses fixed-width Unicode box-drawing (70 chars), truncates URLs with ellipsis when they exceed inner width.
- New `src/server/run_with_shutdown` — cancellable axum entry point using `with_graceful_shutdown`. CLI `run` delegates with `tokio::signal::ctrl_c()`; TUI handoff installs its signal handler via a spawned task before any setup work to eliminate the pre-poll Ctrl+C kill window.
- Errors from the handoff (port-in-use, drain timeout, bind failure) surface through `app.startup_warnings` as a dismissable overlay on the next dashboard render.

### Added — Hierarchical preset picker

- `preset_axes` config (config v3) — multi-axis preset organisation.
- `preset_tree` build / validate / suggest — TUI overlay with key handling and rendering tests.

### Added — TUI form rework

- Sub-form UX with inline `◂ ▸` cycling indicators; Up/Down navigates rows, Left/Right cycles values inline (no dropdowns inside overlays).

### Changed

- `config_version` bumped to `0.3.0`.
- `Config` and sub-types now derive `Clone` so the TUI handoff can pass a config snapshot to the server without reloading from disk.
- `run_with_shutdown` derives the bound address from `listener.local_addr()` rather than a separate `port` parameter; logs report the actual bound address.

### Fixed

- Integration tests now panic if `POUR_HOME` is unset, preventing tests from writing to a real `~/.pour/`.
- CI fixes post-PWA merge.

### Documentation

- v0.2.0 accuracy + linking pass across `pour - docs/`.
- New plan + audit doc for the TUI ↔ Serve handoff at `pour - docs/08 specs/pour-tui-serve-handoff.md`.
- Phase 2 closeout report at `pour - docs/06 reports/v1.0.0-phase2-closeout.md`.

## [0.2.2] — 2026-04-22

### Changed

- Release bundling improvements.
- Test bump.

## [0.2.1] — 2026-04-22

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
