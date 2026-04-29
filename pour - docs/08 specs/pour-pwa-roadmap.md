---
tags:
  - spec
  - pwa
  - mobile
  - roadmap
aliases:
  - pwa roadmap
  - mobile roadmap
  - pwa phases
date created: Monday, April 27th 2026, 4:34:59 pm
status: living document — updated as Phase 1.5/2/3 work lands
date modified: Wednesday, April 29th 2026, 5:31:48 pm
---

# Pour PWA Roadmap

> The mobile/PWA companion landed in Phase 1 (Steps A–G2 of the `web` branch). This doc tracks the gaps surfaced during real-device test-drive, the Phase 2 deferral list locked in [[ADR-005-PWA-Companion]], and the Phase 3+ horizon. Companion to [[pour-api-contract]] (binding wire shape) and [[pour-design-spec]] §7 (architecture intent).

## 1. Status as of 2026-04-27

`web` branch is feature-complete for Phase 1 + Phase 1.5 + Phase 1.5++. Test suite ~774 passing (run `cargo test` for the live count).

Phase 1 (Steps A–G2) shipped: `pour serve`, full `/api/v1/*` surface, embedded PWA, integration tests, OpenAPI spec, ADR + WASM-tradeoffs note.

Phase 1.5 closed real-device test-drive gaps: preset selector (read-only chips), `show_when` cascade fix, scenario8 captured_at timezone test fix.

Phase 1.5++ closed a UX-inspector audit: visual identity overhaul (cyan accent, ▽ icon, header redesign, mobile-fit polish), client-side idempotency-key persistence, contract §9 amendment (round 5: only 2xx cacheable), server-side idempotency cacheability fix, structured logging via `tracing`, test isolation fix (the `~/.pour/` pollution race in `tests/server_submit.rs`).

Mutation paths (preset save/delete, autocreate sub-form, offline queue) remain Phase 2.

## 2. Phase 1.5 — DONE (2026-04-26)

### 2.1 Preset Selector (read-only) — DONE

Saved presets render as chip row above the form fields. Tap-to-apply mirrors TUI semantics (`preset_exclude` fields untouched, missing keys reset to field defaults). Read-only — save/delete still happens via the TUI; that lands in Phase 2.3 with mutation UI.

Implementation: `web/app.js` `fetchPresets`/`buildPresetChipRow`/`applyPreset`; `web/styles.css` chip-row scroll-snap. `aria-pressed` for toggle semantics.

### 2.2 `show_when` Cascade — DONE

Root cause was two compounding bugs: (1) fields with `show_when` that started hidden were never inserted into the DOM, so `recomputeVisibility` queried `null`; (2) `collectValues()` gated on visibility before reading values, creating a stale snapshot. Fix: render all fields unconditionally and toggle the `hidden` attribute on each; `readCurrentFieldValues()` reads all DOM fields without a visibility gate.

Equals match now works for `static_select`, `dynamic_select`, `text`, `number` controllers. `one_of` and empty-controller edge cases preserved from the Step E iteration.

## 2½. Phase 1.5++ — DONE (2026-04-27)

UX inspector audit closed across visual identity, mobile fit, and one CRITICAL idempotency-on-retry bug. All landed in `web/`, `src/server/`, and `tests/`.

### 2½.1 Visual Identity Overhaul

- __Cyan accent palette__ replaces the forest-teal stub (`--accent: #0891b2` light / `#67e8f9` dark). Theme color in HTML meta and manifest match.
- __▽ icon__ — the canonical Pour glyph used in `src/tui/summary.rs` (`▽ saved`) and `src/tui/form.rs` (`▽ pour {module_key}`). White-filled triangle on cyan circle, vertices within 80% safe zone for Android maskable cropping.
- __Header reads `pour › <vault>`__ — lowercase, monospace, dynamically populated from `/api/v1/health.vault_base_path` (basename only).
- __Status pill__ — color-only (no emojis). Border + text colored by transport: success (API), warning (FileSystem), danger (offline).
- __Module tiles__ — show `[<key>]` dim subtext under display name, mirroring TUI dashboard rows.

### 2½.2 Mobile UX

- __Sticky submit bar__ with `padding-bottom: max(16px, env(safe-area-inset-bottom))` — keyboard never covers it; notch/home-indicator respected.
- __`safe-area-inset` padding__ on body, header, sticky bar, toast.
- __SVG chevron on `<select>`__ so dropdowns are recognizable (was hidden by `appearance: none`).
- __Composite array stacks vertically on `max-width: 480px`__ — three-column layout was unreadable on portrait phones.
- __Skeleton loading__ on form view enter — labels + disabled `Loading…` selects render synchronously before async fetches.
- __Empty state copy__ for empty modules list and empty `dynamic_select` with `allow_create: false`.

### 2½.3 Token Rotation + a11y

- 401 anywhere via `apiFetch` → clear `localStorage.pour_token`, jump to token-gate view, throw "Unauthorized" (callers' catch suppresses toast).
- `aria-required`, `aria-describedby`, `role="alert"` on field errors, `role="status" aria-live="polite"` on toast, `:focus-visible` outlines.
- Single `<main>` with `<section>` children (HTML spec compliance).

### 2½.4 Idempotency-on-retry — CRITICAL Bug Fix (contract round 5)

__Bug:__ server cached every response (including 4xx/5xx). Client-side persistent `Idempotency-Key` (added Phase 1.5++) made it strictly worse — user could not retry-after-fix on `validation_failed` because the server replayed the cached 400 for 5 minutes.

__Contract amendment §9 (round 5):__ only __2xx terminal successes__ are cached. 4xx and 5xx are NOT cached — these are recoverable. The client SHOULD reuse the same key for retries after a recoverable error so duplicate-write races never open.

__Server fix:__ `src/server/handlers/submit.rs` gates `idempotency.complete()` on `parts.status.is_success()`; non-2xx calls a new `idempotency.release()` that removes the entry entirely so retries execute fresh.

__Client fix:__ `_pendingIdempotencyKey` persists across retries within a form session, rotates on 2xx success or explicit navigation (Pour Another, dashboard).

Tests: `submit_idempotency_400_not_cached_allows_retry_with_fix`, `submit_idempotency_201_is_cached_replay_returns_201` (regression guard).

### 2½.5 Structured Logging (`pour serve`)

- `tracing` + `tracing-subscriber` + `tower-http` (trace feature only)
- `init_logging()` uses `try_init` for test fixture compatibility
- `POUR_LOG` env var for level filter (default `info,pour=info,tower_http=info`)
- `TraceLayer` wraps the api subrouter (outermost layer — sees post-middleware status)
- Custom logs at: startup, auth outcomes (debug/info/warn per §14 slugs), submit success/failure (module + code, NO field values), captures (debug/warn), idempotency cache (replay/in-flight/eviction with `key_short` first 8 chars)
- §14 compliance: no tokens, no request bodies, no user-supplied content in any log line

### 2½.6 Test Isolation (POUR_HOME race)

`tests/server_submit.rs` previously used bare `unsafe { set_var(POUR_HOME, …) }` without an `ENV_LOCK`. Concurrent test functions raced — when one test's `EnvGuard` dropped mid-other-test, POUR_HOME unset and `History::load()` fell through to `~/.pour/`, polluting users' real history files with test fixture entries (`vault_path: "Coffee/note.md"`, `first_field: "Collision Bean"`, etc.).

Fix: added file-level `ENV_LOCK` + `EnvGuard` matching the pattern in `server_captures.rs` / `server_history.rs` / `server_integration.rs`. New regression test `submit_history_written_to_pour_home_not_real_home` asserts the entry lands in the tempdir and POUR_HOME restores after guard drops.

User remediation: a backup-and-filter cleanup script for polluted `~/.pour/cache/history.jsonl` is documented in conversation history; affected users should run it.

### 2½.7 Other Small Phase 1 Polish (not yet scheduled)

- README "Capturing from your phone" diagram
- `mobile_visible` status pill in PWA header (so the user can confirm the PWA understands which modules are exposed)
- `/api/v1/config` re-fetch chattiness — PWA fetches config on every dashboard return; could cache with ETag/If-None-Match
- Surface `Idempotency-Key` retry count to the user on transient failures (only matters when offline queue lands in Phase 2.1)
- Idempotency cache is FIFO not true LRU per its docstring (drift; not user-visible; clarify or upgrade)
- `to_bytes(usize::MAX)` on response body before caching — fine for current JSON handlers, would need bounding if a future handler streams large bodies

## 3. Phase 2 — Offline-first PWA

The original deferral list per the plan and [[ADR-005-PWA-Companion]] §Decision:

### 3.1 Offline Submit Queue (the Festival case) — DONE (2026-04-27)

- __TASK-2.1.1__ — IDB schema: `pour-queue` DB, `pending_submits` store, auto-increment `id`, indexes on `module_key` + `queued_at`. Schema in `web/queue.js` (page context) and inlined in `web/sw.js` (SW context). Migration rule: NEVER `deleteObjectStore('pending_submits')` — data loss = capture loss. Each record carries: `id, module_key, body, idempotency_key, auth_header, queued_at, attempt_count, last_error`.
- __TASK-2.1.2__ — SW intercepts `POST /api/v1/submit/*`. Network unreachable or 5xx → queue + synthetic 202. 4xx → pass through (client-fixable, never queued). Synthetic 202 body: `{ queued, queue_id, captured_at }`. `Idempotency-Key` captured at queue time, reused on every drain retry. `captured_at` from original body, never drain time. `QuotaExceededError` → 507 "queue full" response to page.
- __TASK-2.1.3__ — Background Sync drain (`pour-queue-drain` tag). FIFO by `queued_at` ASC, tiebreak by `id` ASC. On 2xx: delete record, postMessage page `DRAINED`. On 4xx: keep, increment `attempt_count`, set `last_error` (code only — §14). On 5xx/network: keep, reschedule. Safari fallback: page fires `DRAIN_NOW` postMessage on `window.online`. Both paths call identical `drainQueue()`.
- __TASK-2.1.4__ — "Queued (n)" badge in header (hidden when n=0). Tap → expandable panel listing pending records: module key + relative `queued_at` only (no field values, §14). Panel shows Retry/Edit/Discard actions per record. SW postMessages update badge without polling.
- __TASK-2.1.5__ — Conflict UX: Discard (single tap confirm), Retry now (triggers drain), Edit (pre-fills form with queued body, retains `Idempotency-Key`). If resubmit returns `Idempotency-Replay: true`, shows "This was already saved — showing the original." `captured_at` stays original on edit. IDB record cleaned up after successful 201.

### 3.2 Service Worker — App-shell Cache — DONE (2026-04-27)

- __TASK-2.2.1__ — `/sw.js` served at root scope with `Cache-Control: no-cache, max-age=0, must-revalidate` and `Content-Type: application/javascript`. Route in `src/server/mod.rs`. Tests in `tests/server_static.rs` (6 new tests). `/static/sw.js` returns 404 (scope-poisoning guard).
- __TASK-2.2.2__ — App-shell cache: pre-caches `/, /app.js, /queue.js, /styles.css, /manifest.json, /static/icon.svg`. Cache-first for shell; network-only for `/api/v1/*` (no fallback — always live). SW registered in `DOMContentLoaded` after token bootstrap; failure degrades silently.
- __TASK-2.2.3__ — Update flow: new SW → postMessage `SW_UPDATED` to page → soft refresh banner ("New version available — tap to refresh"). User-initiated `skipWaiting()` only (never auto-claim mid-form). Dismiss hides banner for this session.

### 3.3 `create_template` Sub-form Overlay — DONE (2026-04-27)

- TASK-2.3.1: `<section id="subform-overlay" role="dialog" aria-modal>` in `index.html`. Focus trap (Tab/Shift-Tab cycles inside panel), ESC → cancel, scrim tap → cancel. Mobile-fit (360px minimum, `100dvh` with safe-area awareness). No `<select>` in overlay.
- TASK-2.3.2: Template fields rendered from `/api/v1/config` `templates.<name>.fields[]`. `text`/`number` → standard inputs; `static_select` → inline cycling control with ◂ ▸ chevrons (`role="combobox"` + hidden `<ul role="listbox">` for a11y). Up/Down navigate fields; Left/Right cycle static_select options. Default values prefill. Required validation with per-field error pills. Overlay header shows parent field prompt + typed novel value.
- TASK-2.3.3: Confirm wires `auto_create_inputs[field]` into `_pendingAutoCreateInputs`. Parent submit includes map when non-empty. Novel-value check mirrors `src/autocreate.rs is_existing_option` (case-insensitive, trimmed). If user edits parent field back to an existing option, the map entry is dropped before submit. Map cleared on form reset and 2xx success.
- TASK-2.3.4: Cancel reverts parent input to its open-time value. Map entry dropped. Focus returns to parent input.
- TASK-2.3.5: Server 400 `auto_create_input_required` → overlay re-opens with top-level banner. Server `validation_failed` with template-field names → overlay re-opens with per-field errors pinned. 201 with `warnings[].code === "autocreate_failed"` → summary view shows non-fatal warning chip ("Saved, but note creation failed: …"). No user content logged (contract §14).

### 3.4 Preset Mutation UI — DONE (2026-04-27)

- __TASK-2.4.1__ — "Save as preset" inline affordance below chip row. Tap "+ Save as preset" to reveal inline name input. Name validated 1–64 chars, no `/`. Values snapshot excludes `composite_array`, `preset_exclude=true`, and `show_when`-hidden fields (mirrors TUI). `PUT /api/v1/presets/{module}/{name}` with `description: ""`. On 200/201: re-fetch canonical list, re-render chip row from response. Inline error for 400; toast for other failures.
- __TASK-2.4.2__ — Long-press (~500ms) or right-click on any named chip opens an edit panel inline (not modal). Panel shows name (editable) + description (editable). Save: PUT new name first; if name changed, DELETE old after PUT success (best-effort — page-close-mid-rename leak documented as known minor issue). Panel shows helper text explaining values are not re-snapshotted (delete + re-save for that). `-webkit-touch-callout: none` on chips suppresses iOS native context menu. Pointer events detect long-press vs short-tap; movement > 8px cancels the long-press (drag threshold).
- __TASK-2.4.3__ — Delete button in edit panel. Single-tap-confirm: first tap → "Tap again to confirm" (2s window). Second tap → `DELETE /api/v1/presets/{module}/{name}`. 204 or 404 → remove chip, close panel, clear `_activePreset` if it matched. All via `refreshPresetChipRowFromList` which checks `_activePreset` against fresh list.
- __TASK-2.4.4__ — Drag-and-hold reorder. Pointer events. `dragCommitted` flag: move > 8px from pointerdown = committed drag (otherwise falls through to short-tap → apply). Ghost clone follows pointer; placeholder shows insertion point in real time. On drop: read DOM order (exclude `<none>`), fire `PUT /api/v1/presets/{module}/order` with full canonical list. Re-render from server response. Concurrency: "latest-wins" serialized — only one PUT in flight; further drops queue the latest order and fire after the in-flight settles (`_reorderInFlight` + `_reorderPendingOrder`). Coexists with horizontal scroll-snap via `touch-action: pan-x` on chips, `touch-action: none` only during committed drag.
- __TASK-2.4.5__ — Error paths: all mutations show toast on failure with server's `error.message`. Offline (`!navigator.onLine`) → toast "Offline — preset changes need a connection", NO mutation queued. 401 → existing global handler in `apiFetch` (clear token, jump to token-gate). Always re-render chips from response, never from optimistic local state.

### 3.5 History view + Heatmap — DONE (2026-04-27)

__Data source decision (Open Q2 closed):__ Path (a) — client-side rollup from `/api/v1/history`. Rationale: zero contract cost; no new endpoint or contract amendment required. The summary block (streak, today/week counts) comes from the first no-cursor call. The heatmap rolls up `entries[].timestamp` into a local-date → count map client-side.

__Heatmap pagination loop termination:__ fetches pages at `limit=1000` (HEATMAP_FETCH_LIMIT) using the server's `next_cursor` field until: (a) `has_more === false`, or (b) the oldest entry in the latest page falls before the 90-day window (remaining entries guaranteed older), or (c) a network error (renders with partial data). Covers ~90 days of 10/day captures in a single request for typical usage.

__Cursor discipline:__ Always `next_cursor` from server; never derived from `entries[last].timestamp`. Same-millisecond entries from offline-queue replay would be silently dropped by a timestamp-only cursor — the id-based cursor is immune.

- __TASK-2.5.1__ — History view shell + bottom-tab nav. `<section id="history">` in `<main>`. Bottom-tab nav (`role="tablist"`, `role="tab"`, `aria-selected`). Lazy-loaded on first tab tap. Module key + relative time + first_field per entry. Empty state copy. Tab nav hidden on form/summary views (sticky submit owns the bottom).
- __TASK-2.5.2__ — Cursor pagination. First load: `GET /api/v1/history?limit=50` (no cursor — returns `summary` for heatmap). Scroll within ~200px of bottom + `has_more === true` → append with `?limit=50&cursor=<next_cursor>`. One request in flight at a time (`_historyFetchInFlight` guard). Inline spinner + "Couldn't load more — tap to retry" on failure.
- __TASK-2.5.3__ — Tap entry → `GET /api/v1/captures/{history_id}` → panel with vault_path + transport_mode header. Content rendered via `<pre>` + `textContent` only (never `innerHTML` — XSS guard). 404: "This capture's file no longer exists in the vault." 502: "Couldn't read — vault unreachable." No content logged (contract §14).
- __TASK-2.5.4__ — Heatmap renderer. Past 90 days, columns-of-weeks, cyan accent gradient. Count tiers: 0=bg, 1=light, 2-3=mid, 4+=darkest. Each cell has `aria-label="<long-date>: <count> captures"`. Tap → popover with date + count. `prefers-reduced-motion` respected (transitions only when no-preference). Day-of-week labels (M/W/F). Month labels not included (complex for the gain — omitted in v1). Sits above history list.
- __TASK-2.5.5__ — Decision documented above and in `web/app.js` comment block.
- __TASK-2.5.6__ — Tab nav final: lowercase mono labels, cyan active underline indicator. Hidden on form/summary, shown on dashboard/history. `history.pushState`/`popstate` wired: entering history pushes `{ view: "history" }` state; `popstate` to any other state returns to dashboard. Queue badge (Stream A) coexists in header at 360px — tab nav is fixed at bottom, badge at top.

### 3.6 Capture Trim from PWA — INTENTIONALLY DEFERRED

`pour trim` stays desktop-only per [[pour-api-contract]] §15. The "type 'trim' to confirm" gate is manifesto-aligned and the destructive-action UX doesn't fit a phone screen.

## 4. Phase 3 — Polish and Reach

### 4.1 TLS

- Self-signed cert generation in `pour serve --tls`
- Setup guide for trusting the cert on iOS/Android (the part most users will hate)
- Once TLS lands, `crypto.randomUUID()` works natively (currently using a non-secure-context fallback; see Step E iteration)

### 4.2 mDNS / `pour.local`

- Advertise the server via mDNS so the phone can resolve `pour.local` instead of a raw LAN IP
- Cross-platform mDNS in Rust is annoying; defer until TLS is sorted (since the certs and the hostname both need to align)

### 4.3 MCP Companion (`pour mcp`)

- Per [[pour-api-contract]] §15: a thin Model Context Protocol server that wraps the HTTP API and exposes `pour_submit_<module>` tools dynamically derived from `/api/v1/config`
- Same binary or sibling crate; reuses the engine
- Documented intent only — no implementation slot until the HTTP API has stabilized in production usage

## 5. Phase 2 Utoipa Migration — SCHEDULED

Per [[pour-api-contract]] §15.1: replace the hand-written `pour-openapi.yaml` with `utoipa`-derived output once we're confident the contract is stable. Annotate handlers with `#[utoipa::path(…)]` and types with `#[derive(ToSchema)]`. The hand-written YAML deletes in the same PR.

__When to trigger:__ after Phase 2 ships and we've gone ~1 month without a contract amendment. Premature automation re-creates the drift problem from a different angle.

## 6. Out of Scope (probably forever)

- Multi-tenant `pour serve` (one binary, one config, one vault, one token)
- Cloud sync (violates manifesto's "Plaintext is Forever" — files are the source of truth)
- A native iOS/Android app (the PWA is the answer; app stores violate "the tool gets out of the way")
- Cross-device preset sync via cloud (presets live in `~/.pour/presets.json`; if you want sync, use a vault sync tool)

## 7. Change Log

- __2026-04-26__ — initial roadmap. Phase 1.5 surfaced from real-device test-drive: preset selector + `show_when` cascade. Both in flight.
- __2026-04-26__ — Phase 1.5 landed: preset selector (read-only chips, tap-to-apply mirroring TUI), `show_when` cascade fix (DOM-creation root cause + collectValues stale-snapshot fix), scenario8 captured_at timezone test fix.
- __2026-04-27__ — Phase 1.5++ landed across visual identity (cyan palette, ▽ icon, header redesign, status-pill colors, `[key]` subtext on tiles), mobile UX (sticky submit + safe-area, select chevron, composite stack on narrow, skeleton loading, empty states), token rotation (401-anywhere → token-gate), a11y essentials, structured logging (`tracing` + TraceLayer + §14-compliant custom logs), idempotency cacheability fix (contract §9 round-5 amendment + client-side key persistence), and test isolation fix (POUR_HOME race in `tests/server_submit.rs`).
- __2026-04-27__ — Phase 2 Stream B landed: `create_template` sub-form overlay (TASK-2.0.1, TASK-2.3.1–2.3.5). Overlay shell + focus trap + ESC/scrim cancel; template field rendering with inline cycling (◂ ▸, `role="combobox"`, hidden listbox); novel-value check mirroring `is_existing_option` (case-insensitive); `auto_create_inputs` map wired into parent submit body; cancel-revert; server-error relay (overlay re-open on `auto_create_input_required` or `validation_failed` template fields; non-fatal `autocreate_failed` warning chip on summary). TASK-2.0.1 (PWA test infra gap note) also written. No Rust changes; no new dependencies.
- __2026-04-27__ — Phase 2 Stream A landed: service worker + offline submit queue (TASK-2.2.1–2.2.3, TASK-2.1.1–2.1.5). Contract §6.4 round-6 amendment (synthetic 202 clarification). New files: `web/sw.js`, `web/queue.js`. New route `/sw.js` (+ `/queue.js`) in `src/server/mod.rs`. 6 new tests in `tests/server_static.rs`. No new Rust dependencies; vanilla JS + native IDB/Service Worker APIs only.
- __2026-04-27__ — Phase 2 Stream C landed: preset mutation UI (TASK-2.4.1–2.4.5). Inline save-as affordance; long-press/right-click edit panel (name + description); PUT-then-DELETE rename with known minor leak documented; single-tap-confirm delete; pointer-event drag-reorder with latest-wins serialization. No Rust changes; no new dependencies. §3.4 closed.
- __2026-04-27__ — Phase 2 Stream D landed: history view + heatmap (TASK-2.5.1–2.5.6). Bottom-tab nav (capture/history, `role="tablist"`, cyan active indicator, hidden on form/summary); lazy history list with cursor pagination; tap-to-read capture panel (`<pre>` + `textContent`, never `innerHTML`); 90-day heatmap (client-side rollup from `/api/v1/history`, columns-of-weeks, count tiers, `aria-label` per cell, `prefers-reduced-motion`); `pushState`/`popstate` browser-back wiring. Open Q2 (heatmap data source) closed: path (a) client-side rollup. No Rust changes; no new dependencies. §3.5 closed. Phase 2 complete.
- __2026-04-27__ — Stream C post-inspector fixes (4 CRITICAL + 3 MAJOR findings): (1) `collectPresetValues` now skips empty-string fields, mirroring TUI `!val.is_empty()` predicate exactly; (2) "order" reserved-name guard added in save-as + rename validation (client) and `put_handler` belt-and-suspenders (server, `400 validation_failed reserved_name`); contract §6.8 round-8 amendment + OpenAPI note; 3 regression tests in `tests/server_presets.rs`; (3) rename clobber blocked client-side before PUT fires; (4) long-press with cache desync re-fetches before opening edit panel, aborts with toast if still missing; (5) delete/rename of active preset now resets form fields to defaults via `applyPreset(mod, null)`; rename also updates `_activePreset` to new name before chip re-render; (6) reorder 400 error now re-fetches canonical order and re-renders chip row; (7) save-as single-tap-confirm pattern guards silent overwrite of existing preset name. No new dependencies. 21 `server_presets` tests all passing (was 18).
- __2026-04-27__ — Stream B post-inspector fixes (5 MAJOR findings): (1) error routing — all `validation_failed` fields[] now go to parent form only; (2) focus trap — Tab when focus is outside panel forces recapture to first/last focusable; (3) static_select required-validation bypass — `_userInteracted` flag on cycling wrapper gates required check; (4) iOS 400-race guard — `openSubformOverlay` call in `auto_create_input_required` handler now no-ops if `_overlayContext` already set; (5) server warning messages changed to code-only strings (`src/server/handlers/submit.rs`); internal error now logged via `tracing::warn!`. 780 tests passing.
- __2026-04-27__ — Stream A post-inspector fixes (5 CRITICAL findings): (1) dead 507 handler moved before the 202 branch in `handleSubmit`; (2) `_editingQueueId` leak patched — cleared in `openForm`, `loadDashboard`, and `btn-pour-another`; (3) fresh-install SW_UPDATED suppressed — `activate` only posts when old cache keys existed; (4) stale auth token eliminated — `auth_header` removed from IDB, `drainQueue` uses `getAuthFromClient()` (MessageChannel to page) for a fresh token at drain time, defers if no client open; (5) `#queue-sync-status` wired to drain state machine (DRAIN_STARTED/DRAIN_FINISHED postMessages, window online/offline, panel open/close); CSS modifier classes added. Body limit in `tests/server_static.rs` bumped to 524288. 780 tests passing.
- __2026-04-27__ — Stream D post-inspector fixes (4 CRITICAL findings): (1) heatmap fetch loop now has a hard `HEATMAP_MAX_PAGES=50` iteration cap + empty-page guard — prevents infinite loop on malformed server response (empty entries with `has_more=true`); (2) `renderHeatmap` reuses a single `#heatmap-popover` element (create-once via `getElementById`, reuse on re-render) and attaches the global dismiss click listener exactly once via `_heatmapClickListenerAttached` flag — eliminates popover/listener accumulation across Dashboard ⇄ History round-trips; (3+4) two-level pushState/popstate state machine — `_historyViewPushed` module-level flag replaces the stale `history.state.view` guard (fixes reload bug where history.state survives page reload and guard skips the push); `openCapturePanel` pushes Level 2 state `{view:"history",panel:id}`; `closeCapturePanel(fromPopstate)` distinguishes button-close (calls `history.back()`) from popstate-close (no double-pop); popstate handler routes on new state shape: panel→close panel only, no-panel history view→close panel, no-history-view→go to dashboard + reset `_historyViewPushed`. 783 tests passing.
