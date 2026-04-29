---
tags: [spec, pwa, phase2, backlog]
date created: Monday, April 27th 2026, 4:35:31 pm
status: COMPLETE — all streams A/B/C/D done 2026-04-27
aliases:
  - phase 2 backlog
  - pwa phase 2 tasks
date modified: Wednesday, April 29th 2026, 5:31:47 pm
---

# Phase 2 Task Backlog

> Atomic task cards for Phase 2 of the [[pour-pwa-roadmap]]. Source-of-truth for the architect's execution streams. Complements [[pour-api-contract]] (wire shape) and [[ADR-005-PWA-Companion]] (architectural envelope). Phase 2 §3.6 (capture trim from PWA) is intentionally deferred per contract §15 and is NOT in this backlog.
>
> __Scope window:__ roadmap §3.1 (offline submit queue), §3.2 (service worker app-shell), §3.3 (`create_template` sub-form), §3.4 (preset mutation UI), §3.5 (history view + heatmap).

---

## Streams

Architect runs four parallel streams. Each task card declares its stream.

| Stream | Roadmap sections | File-conflict zone |
|---|---|---|
| A | §3.2 then §3.1 (chained — SW must exist before queue can hook it) | `web/sw.js` (new); `web/app.js` submit path + boot path; `src/server/mod.rs` for `/sw.js` route + headers; `src/server/static_files.rs` if it exists |
| B | §3.3 `create_template` sub-form overlay | `web/app.js` `buildFieldInput` `dynamic_select` branch + `handleSubmit` body assembly; `web/styles.css` overlay rules; `web/index.html` overlay `<section>` |
| C | §3.4 preset mutation UI | `web/app.js` `buildPresetChipRow` + `applyPreset` + new mutation handlers; `web/styles.css` chip long-press / drag affordances; `pour-api-contract.md` §6.7–6.10 if any shape gap surfaces; `pour-openapi.yaml` matching paths |
| D | §3.5 history view + heatmap | `web/app.js` new history view module + heatmap renderer; `web/index.html` new `<section>` + bottom-tab nav; `web/styles.css` heatmap grid; `src/server/handlers/history.rs` only if a heatmap aggregate endpoint is added |

__Cross-stream conflict zones to sequence around:__

- `web/app.js` boot path (`DOMContentLoaded`) — A (SW registration) and D (route to history tab) both touch. A lands first; D rebases.
- `web/index.html` `<main>` children — B (overlay), D (history section + tab nav) both append. Land in alphabetical-stream order.
- `web/styles.css` — every stream adds rules. No conflict if each stream namespaces its own block (`/* ── Phase 2: <stream> ── */`).
- `web/app.js` `handleSubmit` — A (intercept POST and queue) and B (assemble `auto_create_inputs`) both modify. B lands first because the body shape must be settled before A wraps it.
- `applyPreset` / `buildPresetChipRow` — C only. A separate stream from B (`renderForm` overlay launch is in `buildFieldInput`).

__Recommended landing order:__ B → A.2 (SW shell) → A.1 (offline queue) → C → D. B has no dependents and unblocks the contract for `auto_create_inputs` mutation flow on the client. C and D are independent of A/B.

---

## Dependency Graph

```
TASK-2.0.1 (test-infra-flag) ─┐
                              │ (does not block any task; flags a gap)
                              ▼

Stream B
  TASK-2.3.1 (subform-overlay-shell)
        └─ TASK-2.3.2 (subform-field-rendering)
              └─ TASK-2.3.3 (subform-submit-wiring)
                    └─ TASK-2.3.4 (subform-cancel-revert)
                          └─ TASK-2.3.5 (subform-error-relay)

Stream A (chained — SW first, then queue)
  TASK-2.2.1 (sw-route-headers)
        └─ TASK-2.2.2 (sw-shell-cache)
              └─ TASK-2.2.3 (sw-update-flow)
                    └─ TASK-2.1.1 (idb-schema)
                          └─ TASK-2.1.2 (sw-submit-intercept)
                                └─ TASK-2.1.3 (background-sync-drain)
                                      └─ TASK-2.1.4 (queue-badge-ui)
                                            └─ TASK-2.1.5 (queue-conflict-ux)

Stream C
  TASK-2.4.1 (preset-save-as)
        ├─ TASK-2.4.2 (preset-edit)
        ├─ TASK-2.4.3 (preset-delete)
        └─ TASK-2.4.4 (preset-reorder)
              └─ TASK-2.4.5 (preset-mutation-error-paths)

Stream D
  TASK-2.5.1 (history-view-shell)
        └─ TASK-2.5.2 (history-cursor-pagination)
              └─ TASK-2.5.3 (history-tap-to-read)
                    └─ TASK-2.5.4 (heatmap-renderer)
                          └─ TASK-2.5.5 (heatmap-data-source-decision)
                                └─ TASK-2.5.6 (mobile-tab-nav)
```

Cross-stream: TASK-2.1.x assumes B's `auto_create_inputs` shape is settled (so the queued body has a stable schema). TASK-2.5.6 (tab nav) coordinates with TASK-2.1.4 (queue badge in header) — both touch `web/index.html`'s top region.

---

## Task Cards

### TASK-2.0.1 — Flag PWA Test Infra Gap (no implementation)

__Stream:__ meta (no stream — produces a doc note only)
__Depends on:__ none
__Touches:__ `pour - docs/05 notes/PWA-Test-Infra-Gap.md` (new)
__Acceptance criteria:__
- [ ] A short note exists at the path above documenting that `web/` has no JS test infrastructure today (no Jest, no Playwright, no Vitest), and that all Phase 2 PWA logic ships untested by automated harness — only by manual mobile-device verification.
- [ ] Note enumerates which Phase 2 tasks would benefit from JS-side tests if infra existed: offline queue persistence (IDB schema correctness), service worker fetch interception, sub-form state-machine, heatmap aggregation.
- [ ] Note explicitly states this gap is NOT being closed in Phase 2 (no Jest install, no Playwright tests). Closure deferred to a future phase decision.
- [ ] Existing Rust-side `tests/server_*.rs` continue to cover any contract amendment landed in this phase. Tests live in `tests/` mirroring `src/`, not inline (per CLAUDE.md + memory `feedback_test_structure.md`).
- [ ] `00 index/notes.md` (or equivalent) links the new note.
__Out of scope (this task):__
- Installing a JS test framework.
- Writing any test, Rust or JS.
__Risk / inspector hot-spots:__ that the note pretends the gap doesn't exist. The note must be honest about the blast radius.

---

### Stream B — `create_template` Sub-form Overlay (§3.3)

The novel-value path on a `dynamic_select` field with `allow_create + create_template` already validates server-side: the submit handler returns 400 `auto_create_input_required` if `auto_create_inputs[field]` is missing for a templated field with a novel value (see `src/server/handlers/submit.rs` lines 328–342). Phase 2 makes the client supply that map via an overlay sub-form.

#### TASK-2.3.1 — Sub-form Overlay Shell

__Stream:__ B
__Depends on:__ none
__Touches:__ `web/index.html` (new `<section id="subform-overlay">` with `aria-modal`), `web/app.js` (open/close functions, focus trap, ESC handler), `web/styles.css` (overlay positioning, scrim, mobile-fit)
__Acceptance criteria:__
- [ ] An overlay `<section>` exists in `index.html`, hidden by default, with `role="dialog"` and `aria-modal="true"`.
- [ ] Open function takes a parent field reference + a typed novel value; renders the overlay over the form view; saves and traps focus; restores focus to the originating input on close.
- [ ] Close function dismisses the overlay, releases focus trap, restores scroll position.
- [ ] ESC closes the overlay (treated as cancel — see TASK-2.3.4).
- [ ] Overlay scrim covers the form view; tap on scrim outside the modal triggers cancel.
- [ ] Overlay fits within a 360 px-wide viewport with no horizontal scroll. Long template names truncate with ellipsis.
- [ ] Overlay does NOT use a `<select>` for any control (per memory `feedback_subform_ux.md` — no dropdowns in overlays).
- [ ] Affected docs updated: `pour - docs/04 architecture/System-Architecture-Overview.md` PWA subsystem note (overlay is a new view), `pour-pwa-roadmap.md` change log entry under "Phase 2".
__Out of scope (this task):__
- Rendering template fields (TASK-2.3.2).
- Wiring submit (TASK-2.3.3).
- Cancel-revert behavior on parent field (TASK-2.3.4).
__Risk / inspector hot-spots:__ focus trap leaks (tab past the modal), scrim z-index fight with sticky submit bar, iOS Safari `100dvh` quirks under keyboard.

#### TASK-2.3.2 — Sub-form Field Rendering with Up/Down + Left/Right Semantics

__Stream:__ B
__Depends on:__ TASK-2.3.1
__Touches:__ `web/app.js` template-field renderer, `web/styles.css` field stack inside overlay
__Acceptance criteria:__
- [ ] Given a template name (from the parent `field.create_template`), the overlay renders one input per template field as defined in `/api/v1/config` `templates.<name>.fields[]`.
- [ ] Each field renders per its `field_type`: `text` → text input; `number` → number input; `static_select` → an inline-cycling control with visible `◂ ▸` chevrons (per memory `feedback_subform_ux.md` and `feedback_inline_cycling.md`). NO `<select>` element in this overlay.
- [ ] Up/Down arrow keys on a focused field move focus to the previous/next field in tab order.
- [ ] Left/Right arrow keys on a focused `static_select` cycle through options (wrapping); on `text`/`number`, Left/Right do their default caret movement.
- [ ] `required` semantics from the template field config are respected: empty required fields show a per-field error pill on submit attempt; cycling control with no value selected counts as empty if `required=true`.
- [ ] Default values from the template field config prefill the inputs.
- [ ] The overlay header shows the field's `prompt` text and the typed novel value (e.g. "Create bean: 'Ethiopia Guji'") so the user has a label.
- [ ] Affected docs updated: `field-types.md` reference checked — if any new client-only semantic emerges (it should not), it gets a note. Otherwise: confirmation that overlay rendering does not change the field-type contract.
__Out of scope (this task):__
- Submit wiring (TASK-2.3.3).
- Server-side validation echo (TASK-2.3.5).
__Risk / inspector hot-spots:__ the inline-cycling control's a11y story (`role="spinbutton"` vs `role="combobox"` vs custom — pick one and document); arrow-key event handlers swallowing native input behavior on text fields.

#### TASK-2.3.3 — Sub-form Submit Wires `auto_create_inputs[field]` into Parent Body

__Stream:__ B
__Depends on:__ TASK-2.3.2
__Touches:__ `web/app.js` `handleSubmit` body assembly + new overlay-confirm path
__Acceptance criteria:__
- [ ] When the user confirms the overlay, the typed novel value remains in the parent field's input AND a `_pendingAutoCreateInputs[fieldName] = { …templateValues }` map accumulates in client state.
- [ ] On parent form submit, the request body includes `auto_create_inputs` exactly when the map is non-empty (key = parent field name, value = template field map). Empty values are sent as empty strings (matching contract §6.4 wire convention: all values are strings).
- [ ] Confirm path triggers ONLY when the typed value is novel (i.e. not in the cached `_optionsCache` for that module/field). Existing values do NOT open the overlay.
- [ ] `_pendingAutoCreateInputs` clears on form reset (Pour Another, dashboard navigation, success).
- [ ] Idempotency-Key behavior unchanged: the parent submit's `Idempotency-Key` is the same one already persisted across retries (contract §9 round 5). Reopening the overlay between retry attempts does not rotate the key.
- [ ] If the user types a novel value, opens the overlay, confirms, then EDITS the parent field back to a non-novel (existing) value, the auto-create map for that field is dropped before submit.
- [ ] Affected docs updated: `pour-pwa-roadmap.md` §3.3 marked done with the change-log line; `System-Architecture-Overview.md` PWA section notes the auto_create_inputs client path.
__Out of scope (this task):__
- Cancel/revert UX (TASK-2.3.4).
- Server error relay (TASK-2.3.5).
__Risk / inspector hot-spots:__ the "novel-vs-existing" check must mirror server-side (case-sensitive substring? exact match? trimmed?). Read `src/autocreate.rs` `is_existing_option` and verify the client check matches. Mismatched novelty checks are a duplicate-write hazard.

#### TASK-2.3.4 — Cancel Reverts the Typed Parent Value

__Stream:__ B
__Depends on:__ TASK-2.3.3
__Touches:__ `web/app.js` overlay cancel handler
__Acceptance criteria:__
- [ ] The overlay records the parent field's value at open-time. On cancel (ESC, scrim tap, explicit Cancel button), the parent field's input value is restored to the open-time value.
- [ ] If the parent field had no prior value, cancel leaves it empty.
- [ ] `_pendingAutoCreateInputs[fieldName]` is dropped on cancel.
- [ ] After cancel, focus returns to the parent input.
- [ ] Affected docs updated: roadmap change log mentions cancel-revert as part of §3.3 closure.
__Out of scope:__ Server error relay.
__Risk / inspector hot-spots:__ if the user typed AFTER opening (which shouldn't be possible because the parent input is disabled while overlay is up — verify), the revert must still pick the open-time value.

#### TASK-2.3.5 — Surface Server-side template/auto-create Errors in the Overlay

__Stream:__ B
__Depends on:__ TASK-2.3.3
__Touches:__ `web/app.js` submit error handling, overlay error region
__Acceptance criteria:__
- [ ] If the parent submit returns 400 `validation_failed` with `details.fields[]` entries whose `field` matches a template field name, the overlay re-opens with those errors pinned per-field.
- [ ] If the parent submit succeeds (201) but the response includes `warnings[]` with `code: "autocreate_failed"` for the parent field, the success view shows a non-fatal warning chip ("Saved, but bean note creation failed: <message>"). Capture is not lost.
- [ ] Overlay-relayed errors do NOT echo user-supplied content into log lines (contract §14). The client-side overlay can render the message; the server log does not.
- [ ] If the server returns 400 `auto_create_input_required` (the existing extra error code) for a field the client thought it had filled, the overlay re-opens with a top-level "Please complete the template" banner. This is a defensive code path — should be unreachable if TASK-2.3.3 is correct.
__Out of scope:__ Adding new error codes to the contract. None should be needed.
__Risk / inspector hot-spots:__ error mapping from the flat `details.fields[]` (which uses parent-form field names) to the overlay's template-field names. If a parent field happens to share a name with a template field, the wrong message lands.

---

### Stream A — Service Worker + Offline Queue (§3.2 then §3.1)

#### TASK-2.2.1 — Serve `/sw.js` with Correct Cache Headers and Scope

__Stream:__ A
__Depends on:__ none
__Touches:__ `web/sw.js` (new, empty stub initially), `src/server/mod.rs` static-asset routing or `src/server/static_files.rs`, `pour-api-contract.md` §12 (already specifies `/sw.js: no-cache, max-age=0, must-revalidate` — verify), `pour-openapi.yaml` (no change — `/sw.js` is not under `/api/v1/`)
__Acceptance criteria:__
- [ ] `web/sw.js` exists and is served at `/sw.js` (root scope — required for SW to control `/`).
- [ ] Response headers include `Cache-Control: no-cache, max-age=0, must-revalidate` per contract §12.
- [ ] Response `Content-Type: application/javascript; charset=utf-8`.
- [ ] No auth required to fetch `/sw.js` (it's a static asset; the SW itself does not call `/api/*` until the page passes its token).
- [ ] `pour serve` startup logs do NOT log the SW request body or path on every fetch (contract §14 — keep info-level chatter low).
- [ ] Integration test in `tests/server_integration.rs` (or `tests/server_static.rs`) asserts the response code, headers, and content-type. Test follows the existing `tests/` mirror-of-`src/` pattern.
- [ ] Affected docs updated: `pour-api-contract.md` §12 confirmed (no amendment needed); `System-Architecture-Overview.md` PWA section notes the SW route; `pour-pwa-roadmap.md` change log.
__Out of scope:__ Cache logic (TASK-2.2.2), update flow (TASK-2.2.3).
__Risk / inspector hot-spots:__ scope. The SW must be served at `/sw.js`, not `/static/sw.js`, or it cannot intercept root-level navigation. Existing `rust-embed` config may need adjustment.

#### TASK-2.2.2 — App-shell Cache (network-first /api/*, Cache-first shell)

__Stream:__ A
__Depends on:__ TASK-2.2.1
__Touches:__ `web/sw.js` install + activate + fetch handlers, `web/app.js` SW registration in boot path
__Acceptance criteria:__
- [ ] On `install`, the SW pre-caches: `/`, `/app.js`, `/styles.css`, `/manifest.json`, `/static/icon.svg` (and any other current shell assets — read `web/index.html` for the live list).
- [ ] On `fetch`:
  - Requests to `/api/v1/*` use __network-only__ with NO cache fallback. Per contract §12 (`Cache-Control: no-store`) — these MUST always be live, not cached. (This is distinct from offline-queue interception, which is TASK-2.1.2.)
  - Requests to shell assets (`/`, `/app.js`, `/styles.css`, `/manifest.json`, `/static/*`) use __cache-first__ with a network revalidate.
  - Requests to `/sw.js` itself bypass the SW (browser default).
- [ ] On `activate`, old caches are pruned (only the current shell version's cache survives).
- [ ] `app.js` registers the SW exactly once on `DOMContentLoaded`, after token bootstrap (so the page works even if SW registration fails).
- [ ] SW registration failure does NOT block the form from rendering. Failure is logged to the browser console only.
- [ ] Cold-tap on a registered home-screen icon while offline shows the shell within ~1 s; `/api/v1/health` will still fail (no cache fallback for API), and the existing "Offline" status pill renders.
- [ ] Affected docs updated: roadmap §3.2 marked done; `System-Architecture-Overview.md` PWA section gains an SW row; `v1.0.0-Release.md` Phase-2-deferred bullet for "service worker app-shell cache" struck through with closure note.
__Out of scope:__ Submit interception (TASK-2.1.2). Update prompt UX (TASK-2.2.3).
__Risk / inspector hot-spots:__
- Cache key naming — must include a version stamp so TASK-2.2.3 update flow can invalidate.
- iOS Safari intermittent SW eviction; the spec MUST work even if the SW is dropped between sessions.
- Asset list drift: new files added to `web/` after this task lands won't be cached unless the SW asset list is updated. Document this in a code comment in `sw.js`.

#### TASK-2.2.3 — Update Flow: New Deploy Invalidates the Cache, Prompts Soft Refresh

__Stream:__ A
__Depends on:__ TASK-2.2.2
__Touches:__ `web/sw.js` (version constant, `skipWaiting`/`clients.claim` handling), `web/app.js` (postMessage listener for "update available", soft refresh banner), `web/styles.css` (banner)
__Acceptance criteria:__
- [ ] SW carries a `CACHE_VERSION` constant. Bumping the version on a new build is enough to trigger a re-install.
- [ ] When a new SW activates and the user has an open tab with the old version, the page shows a non-blocking banner: "New version available — tap to refresh." Tap reloads.
- [ ] User can dismiss the banner; dismissal does not block the next session from showing the banner again until reload.
- [ ] Closing and reopening the PWA after a new deploy without seeing the banner still serves the new version (because the SW activates on the next cold start).
- [ ] Cache version bumping is documented in a comment in `web/sw.js`: any change to `web/` requires a `CACHE_VERSION` bump.
- [ ] Affected docs updated: roadmap change log; `System-Architecture-Overview.md` SW section gains the version-bump rule.
__Out of scope:__ Auto-bumping `CACHE_VERSION` from build metadata (manual is fine for v1).
__Risk / inspector hot-spots:__ `skipWaiting()` foot-guns — with multiple tabs open, an aggressive `skipWaiting + claim` can break in-flight forms. The soft refresh banner must wait for the user to act, not silently swap clients.

#### TASK-2.1.1 — IndexedDB Schema for the Offline Queue

__Stream:__ A
__Depends on:__ TASK-2.2.1 (SW must be servable before the queue lives in it)
__Touches:__ `web/app.js` (or new `web/queue.js` module), `pour-pwa-roadmap.md` (change log entry naming the schema version)
__Acceptance criteria:__
- [ ] An IDB database `pour-queue` exists with one object store: `pending_submits`. Key path: `id` (auto-increment integer or UUID).
- [ ] Each record carries: `id`, `module_key`, `body` (the JSON-serializable submit payload — `field_values`, `composite_data`, `auto_create_inputs`, `callout_overrides`, `callout_titles`, `captured_at`, `client_id`), `idempotency_key` (string, generated client-side at queue-time and reused on every retry per contract §9 round 5), `queued_at` (ISO 8601 — wall clock at queue-time, distinct from `body.captured_at`), `attempt_count` (int, starts at 0), `last_error` (string or null).
- [ ] An index on `module_key` for filtering, and on `queued_at` for chronological drain order.
- [ ] DB version is `1`. An `onupgradeneeded` migration story is documented in a code comment: future schema bumps should add migrations, not destroy `pending_submits` (data loss = capture loss, and the manifesto's whole point is no capture loss).
- [ ] Open question logged for the architect (see "Open questions" below): if the user clears site data, the queue is lost. Acceptable for v1; flag for future "warn before clear" treatment.
- [ ] No field values appear in any log line (contract §14). Queue debug output (if any) shows record id and module key only.
- [ ] Affected docs updated: roadmap §3.1 references the new schema; `System-Architecture-Overview.md` gains an "offline queue" row; `pour-api-contract.md` §10 (captured_at) cross-referenced.
__Out of scope:__ SW interception (TASK-2.1.2), drain (TASK-2.1.3), UI (TASK-2.1.4).
__Risk / inspector hot-spots:__
- `Idempotency-Key` is generated at QUEUE time, not at SUBMIT time, and is REUSED on every retry. This is contract §9 round 5 compliant: same key, recoverable error → re-execute. The inspector should verify the key is NOT rotated by any other code path.
- `captured_at` in the body must be the wall-clock instant the user tapped Submit on the phone (the original tap), not the re-drain time. This is contract §10 — the festival case is the whole reason §10 exists.

#### TASK-2.1.2 — Service Worker Intercepts `POST /api/v1/submit/*` when Offline, Queues, Returns Synthetic 202

__Stream:__ A
__Depends on:__ TASK-2.1.1
__Touches:__ `web/sw.js` fetch handler, `web/app.js` submit response handling (must accept 202 as "queued, not 201")
__Acceptance criteria:__
- [ ] When the SW catches a `POST /api/v1/submit/{module}` and the network is unreachable (fetch rejects, OR navigator.onLine is false at attempt time), the SW writes the request body + headers (specifically `Idempotency-Key`) to IDB `pending_submits` and returns a synthetic `202 Accepted` to the page.
- [ ] Synthetic 202 body: `{ "queued": true, "queue_id": "<idb-id>", "captured_at": "<from-body>" }`. NOT a 201 — distinguishes from server-issued create.
- [ ] When the network IS reachable, the SW passes the request through unchanged. Same `Idempotency-Key` reaches the server. Contract §9 round 5: the server caches only 2xx; on a 4xx (validation_failed), the client retries the same key.
- [ ] If the request is reachable but the server returns 4xx, the SW does NOT queue — that's a client-fixable error, not an offline condition. The 4xx flows through to the page as normal.
- [ ] If the request is reachable but the server returns 5xx or the connection drops mid-flight, the SW queues and returns 202. This matches the "festival case" — server is down, capture must not be lost.
- [ ] `pour-api-contract.md` is amended with a §6.4 note: "the synthetic 202 is a CLIENT-SIDE construct emitted by the PWA service worker. The server itself does not return 202 from `/api/v1/submit/*`. Clients written against the contract directly will never see 202." This amendment lands BEFORE the handler change (per memory `feedback_contract_first.md`).
- [ ] `pour-openapi.yaml` updated to reflect the contract amendment text. Phase 2 still hand-written; utoipa migration is a separate roadmap item.
- [ ] PWA submit handler treats 202 as "queued — show 'Queued' summary view, NOT 'Saved' summary view." User sees a different success message: "Queued — will sync when online."
- [ ] Logging §14: SW MUST NOT log the request body. Server `tracing::info!` from the eventual drained submit is identical to a fresh submit (module + vault path on success, module + code on failure).
- [ ] Affected docs updated: contract §6.4 amendment; OpenAPI yaml; roadmap §3.1 progress; `System-Architecture-Overview.md` adds queue-write entry.
__Out of scope:__ Drain logic (TASK-2.1.3), badge UI (TASK-2.1.4).
__Risk / inspector hot-spots:__
- Distinguishing "offline" vs "reachable but server 5xx" — both are queueable. The fetch promise rejecting is the offline signal; a 5xx response is a separate signal. Both must queue, but errors with 4xx must NOT queue.
- The SW intercepting the same `Idempotency-Key` on both queue-write and eventual drain. Verify by integration: take a payload offline, queue it, come online, drain → server sees the key once, responds 201, future replays from the queue (none should occur) would see the cache hit if any.
- Body size — IDB has per-origin quotas (~50% of free disk on Chromium, ~1 GB on iOS Safari). A 1 MiB submit (max per contract §13) × 1024 queued = 1 GiB. The PWA must show a "queue full" failure mode if IDB write rejects with `QuotaExceededError`.

#### TASK-2.1.3 — Background Sync Drains the Queue when Network Returns

__Stream:__ A
__Depends on:__ TASK-2.1.2
__Touches:__ `web/sw.js` (`sync` event listener with fallback to `online` event), `web/app.js` (UI listener for drain progress messages)
__Acceptance criteria:__
- [ ] SW registers a Background Sync tag (`pour-queue-drain`) when a record is queued (TASK-2.1.2).
- [ ] On `sync` event firing (network returns AND browser fires the event), SW iterates `pending_submits` in `queued_at` ascending order. For each record:
  - Fires `POST /api/v1/submit/{module}` with the original body and `Idempotency-Key`.
  - On 2xx: deletes the record from IDB; postMessages the page with `{ type: "drained", queue_id, history_id, vault_path }`.
  - On 4xx: keeps the record; increments `attempt_count`; sets `last_error` to the error code (NOT the message — per §14, code only); postMessages the page with `{ type: "drain_error", queue_id, code }`. The user can review and edit/discard via TASK-2.1.5.
  - On 5xx or network error: keeps the record; increments `attempt_count`; reschedules sync.
- [ ] Background Sync API is feature-detected. If unavailable (Safari today), fall back to a `window.online` event listener that triggers the same drain logic from the page side. SW message-passes drain results.
- [ ] Drain order is FIFO — earliest `queued_at` first. This preserves the order the user captured.
- [ ] `Idempotency-Key` is reused on every retry, never rotated by the drain logic. Contract §9 round 5 compliant.
- [ ] `captured_at` in each drained body is the original capture time, not the drain time. The whole point.
- [ ] Logging §14: server-side `tracing::info!` for each drained submit is identical to a fresh submit. No "drain" marker in the server log (the server doesn't know it's a drain).
- [ ] Affected docs updated: roadmap §3.1 progress; `v0.2.0-Foundation.md` known-limitations: if "captures lost on offline" was implicit, strike it through with the queue's closure note.
__Out of scope:__ Badge UI (TASK-2.1.4), conflict UX (TASK-2.1.5).
__Risk / inspector hot-spots:__
- A "poison pill" record that consistently 4xxs (e.g. user typed an invalid number, then went offline and queued). Without TASK-2.1.5 the queue jams. Verify TASK-2.1.5 is sequenced before declaring §3.1 closed.
- iOS Safari has historically not supported Background Sync. The fallback `online` event MUST work — verify on a real iOS device, not just Chrome desktop devtools "offline" toggle.
- Race: page is open during drain. The page must accept postMessage updates to update the badge count without re-fetching the queue from IDB on every message.

#### TASK-2.1.4 — Queue Badge in Header + Expandable pending List

__Stream:__ A
__Depends on:__ TASK-2.1.3
__Touches:__ `web/index.html` header markup, `web/app.js` badge state + expand panel, `web/styles.css` badge + panel
__Acceptance criteria:__
- [ ] Header shows a "Queued (n)" badge when `n > 0` records exist in IDB. Badge is hidden when `n = 0`.
- [ ] Tap on badge opens an expanded panel listing each pending record: module key, `queued_at` (relative — "3m ago"), and a discard button. NO field values shown in the panel (per memory `feedback_pour_aesthetic.md` — low-friction wins, but also per §14 we shouldn't be cavalier with capture content even on the user's own device — though this is more aesthetic than security-critical, since the data IS the user's own).
- [ ] Panel shows a sync status indicator: "Syncing…" while drain is in flight, "Synced" briefly after a successful drain, "Network unreachable" when offline.
- [ ] Page subscribes to SW postMessage events to update the badge count without polling.
- [ ] Badge state survives a page reload (re-reads IDB on init).
- [ ] Badge does NOT block the form view when shown — sticks to the header, doesn't push other content.
- [ ] On a 360 px viewport, the badge fits within the header without truncating the wordmark "pour › <vault>".
- [ ] Affected docs updated: roadmap §3.1 closure; `System-Architecture-Overview.md` PWA section notes the badge.
__Out of scope:__ Discard / edit UX (TASK-2.1.5).
__Risk / inspector hot-spots:__ the badge competing for header real estate with the existing status pill on a small viewport.

#### TASK-2.1.5 — Conflict UX: Discard, Retry, and "edit and resubmit"

__Stream:__ A
__Depends on:__ TASK-2.1.4
__Touches:__ `web/app.js` panel actions, `web/styles.css` action buttons inside panel
__Acceptance criteria:__
- [ ] Each pending record in the panel has: a "Discard" button (deletes from IDB, no resubmit), a "Retry now" button (re-fires drain immediately for this record only), and "Edit" (opens the form view pre-populated with the record's `body`, with the same `Idempotency-Key` retained — this is critical for §9 compliance: the resubmit on the SAME key WILL replay the cached 2xx if a prior attempt succeeded server-side, otherwise execute fresh).
- [ ] Edit-and-resubmit flow:
  - Form opens prefilled with `body.field_values`, `composite_data`, `auto_create_inputs`, `callout_overrides`, `callout_titles`, `captured_at` (display as "captured 3 days ago" so the user knows the timestamp will be preserved).
  - Submitting the edited form deletes the original IDB record and submits via the normal path. The same `Idempotency-Key` is used so a cached server response is honored.
  - If the user navigates away without submitting, the original record stays in the queue.
- [ ] Discard requires a single-tap confirm step (not a multi-step dialog — per memory `feedback_pour_aesthetic.md` low-friction wins). A small "Tap again to confirm" pattern is acceptable.
- [ ] Affected docs updated: roadmap §3.1 closure with discard/edit semantics noted; `System-Architecture-Overview.md` PWA section.
__Out of scope:__ Anything beyond the queue.
__Risk / inspector hot-spots:__
- The Idempotency-Key reuse on edit-and-resubmit. If the cached server response is a 201 from a prior attempt the user didn't realize succeeded, the "edited" body is silently NOT executed (server replays the original). Surface this clearly: if the resubmit returns `Idempotency-Replay: true` header, show a toast "This was already saved — opened the original."
- The `captured_at` value when the user edits — does it stay the original or update to the edit time? Spec it: STAY ORIGINAL. The user is editing the contents, not the moment.

---

### Stream C — Preset Mutation UI (§3.4)

The presets read path (Phase 1.5 chip row) and the server-side mutation endpoints (`PUT /api/v1/presets/{module}/{name}`, `DELETE /api/v1/presets/{module}/{name}`, `PUT /api/v1/presets/{module}/order`) are already shipped per contract §6.7–6.10.

#### TASK-2.4.1 — "Save Current Values as preset" Affordance

__Stream:__ C
__Depends on:__ none
__Touches:__ `web/app.js` form-actions area, `web/styles.css` save-as button + name-input UI
__Acceptance criteria:__
- [ ] A "Save as preset" button exists in or near the preset chip row. Tap opens an inline name-input (NOT a modal — keep low-friction).
- [ ] Name input enforces: 1–64 characters, no `/` (matches contract §6.8 path-segment rule). Invalid input shows inline error.
- [ ] On submit, the client calls `PUT /api/v1/presets/{module}/{encodeURIComponent(name)}` with the body `{ description: "", values: <currentVisibleFieldValues> }`. Description left blank for v1 (TASK-2.4.2 handles edit).
- [ ] Saved presets EXCLUDE `composite_array` fields (mirroring TUI behavior — verify by reading `src/data/presets.rs`). Preset values are field_name → string.
- [ ] Fields with `preset_exclude = true` are omitted from the saved preset (mirroring TUI; verify `src/data/presets.rs`).
- [ ] Hidden fields (per `show_when` evaluating to false) are also omitted.
- [ ] On 200/201, the chip row is re-rendered from the response. The newly saved preset is immediately tappable.
- [ ] On 400 `validation_failed` (e.g. duplicate name with different shape, server-rejected name): show error inline.
- [ ] Idempotency: this is a `PUT` (upsert), so retries are naturally idempotent at the server level. No `Idempotency-Key` needed.
- [ ] Affected docs updated: roadmap §3.4 progress; `pour-api-contract.md` §6.8 — verify the existing spec accommodates the empty `description` (it does — `description` is in the body but contract treats it as a free-form string; if the server rejects empty, that's a contract-vs-implementation inspector finding); `pour-openapi.yaml` body schema cross-checked.
__Out of scope:__ Edit (TASK-2.4.2), delete (TASK-2.4.3), reorder (TASK-2.4.4).
__Risk / inspector hot-spots:__
- Empty `description` round-tripping. If the server normalizes "" to null, the client must accept either on subsequent reads.
- URL-encoding the name in the path. Names with spaces, unicode, or odd ASCII MUST encode correctly. The server's axum matcher already rejects `/` per §6.8.

#### TASK-2.4.2 — Edit Existing Preset (long-press chip)

__Stream:__ C
__Depends on:__ TASK-2.4.1
__Touches:__ `web/app.js` chip long-press handler + edit panel, `web/styles.css` edit panel
__Acceptance criteria:__
- [ ] Long-press (~500 ms) on a preset chip opens an edit panel showing: current name (editable), description (editable, free text), and a "save" button that fires `PUT /api/v1/presets/{module}/{newName}` with the updated body.
- [ ] If the name changed, the OLD preset is also `DELETE`d in the same flow (two requests; sequence: PUT new, then DELETE old; on PUT failure, do not DELETE — atomicity is best-effort).
- [ ] Edit panel preserves the preset's existing field values — the user is editing metadata (name, description), not the captured values. To "re-snapshot" current form values into an existing preset, the user must delete and re-save (TASK-2.4.1 + TASK-2.4.3). Document this in the panel's helper text.
- [ ] Touch-and-drag on a chip that does NOT cross the long-press threshold falls back to "tap-to-apply" (existing Phase 1.5 behavior).
- [ ] Long-press also works via desktop right-click (some users will hit this from a desktop browser).
- [ ] Affected docs updated: roadmap §3.4 progress.
__Out of scope:__ Delete (TASK-2.4.3), reorder (TASK-2.4.4).
__Risk / inspector hot-spots:__
- The PUT-new-then-DELETE-old rename can leak a duplicate if the page closes between the two requests. Document this as a known minor leak; the user can manually clean up via the next mutation. Acceptable for v1 because preset names are user-meaningful and duplicates are visible.
- Long-press conflicting with native iOS Safari context menus (the "share" sheet on long-press of a link). Verify chips have `-webkit-touch-callout: none` and the long-press handler suppresses the default.

#### TASK-2.4.3 — Delete Preset

__Stream:__ C
__Depends on:__ TASK-2.4.2 (the edit panel hosts the delete button)
__Touches:__ `web/app.js` edit panel delete handler
__Acceptance criteria:__
- [ ] The edit panel includes a "Delete" button. Single-tap-to-confirm (per `feedback_pour_aesthetic.md` — low friction wins).
- [ ] Confirm fires `DELETE /api/v1/presets/{module}/{encodeURIComponent(name)}`. On 204 the chip is removed from the row immediately.
- [ ] On 404 (race: another client already deleted): silently treat as success and remove the chip.
- [ ] If the deleted preset was `_activePreset`, clear the active state (chips return to `<none>` mode).
- [ ] Affected docs updated: roadmap §3.4 progress.
__Out of scope:__ Reorder (TASK-2.4.4).
__Risk / inspector hot-spots:__ the `_activePreset` state must be cleared in client memory, not just visually, or the next form action references a deleted preset.

#### TASK-2.4.4 — Reorder Presets via Drag-and-hold

__Stream:__ C
__Depends on:__ TASK-2.4.3
__Touches:__ `web/app.js` chip drag handlers, `web/styles.css` drag affordance + drop indicator
__Acceptance criteria:__
- [ ] Drag-and-hold on a chip lifts it visually; dragging across other chips reorders them in real time within the chip row.
- [ ] On drop, the client fires `PUT /api/v1/presets/{module}/order` with the full ordered list of names per contract §6.10. Missing or extra names → server returns 400 `validation_failed`; client must always send the full canonical list (read from the current chip-row DOM).
- [ ] Server response (200 with the resulting `{ presets: […] }`) becomes the new source of truth — chips re-render from it. If the server reordered (it shouldn't), the client respects.
- [ ] The `<none>` chip is NOT included in the reorder list (it's not a preset). Verify by reading the chip-row builder.
- [ ] Touch and mouse both supported. Pointer events preferred where available.
- [ ] On a 360 px viewport, drag still works inside the horizontally-scrolling chip row (the existing scroll-snap from Phase 1.5 must coexist with drag — verify).
- [ ] Affected docs updated: roadmap §3.4 closure with reorder semantics noted.
__Out of scope:__ Drag-to-other-module (out of scope forever).
__Risk / inspector hot-spots:__
- Reorder + scroll-snap interaction in the horizontally-scrolling chip row.
- A drag drop that completes before the previous reorder PUT returns. Either serialize requests (queue them) or always send the latest known order on the most recent drop. Pick one and document.

#### TASK-2.4.5 — Mutation Error Paths and Offline Behavior

__Stream:__ C
__Depends on:__ TASK-2.4.4
__Touches:__ `web/app.js` preset mutation handlers, error toast wiring
__Acceptance criteria:__
- [ ] All preset mutations show a per-action error toast on failure with the server's `error.message` (which is sanitized server-side per §14).
- [ ] If the user is offline and attempts a preset mutation, a "Offline — preset changes need a connection" toast shows. NO offline queueing for preset mutations in Phase 2 (out of scope — keep the offline queue scoped to submits).
- [ ] On 401 anywhere in the preset flow, the existing global 401 handler in `apiFetch` (Phase 1.5++) takes over — clear token, jump to token-gate.
- [ ] Affected docs updated: roadmap §3.4 closure; `System-Architecture-Overview.md` notes preset-mutation client surface.
__Out of scope:__ Queueing preset mutations (deferred to a future phase).
__Risk / inspector hot-spots:__ silent UI desync when a mutation 200s but the response shape differs from cache. Always re-render chips from the response.

---

### Stream D — History view + Heatmap (§3.5)

#### TASK-2.5.1 — History view Shell with Bottom-tab Navigation

__Stream:__ D
__Depends on:__ none
__Touches:__ `web/index.html` (new `<section id="history">`, bottom-tab nav element), `web/app.js` (showView extension, history loader), `web/styles.css` (bottom-tab nav, history list)
__Acceptance criteria:__
- [ ] A bottom-fixed tab nav exists with at minimum two tabs: "Capture" (existing dashboard) and "History". Tab tap switches views without a page reload.
- [ ] The history view is a new section in `<main>`. It loads on first tab tap, not on app boot (lazy load).
- [ ] Tab nav respects `env(safe-area-inset-bottom)` so it doesn't get covered by the iOS home indicator.
- [ ] Tab nav uses `aria-selected` for active state, `role="tablist"` and `role="tab"` for a11y.
- [ ] History list shows: each entry's `module_key`, `timestamp` (rendered as relative — "3m ago"), and `first_field` (if present). NO frontmatter values, NO body content.
- [ ] Empty-state copy when history is empty: "No captures yet. Tap a module to start pouring."
- [ ] Affected docs updated: roadmap §3.5 progress; `System-Architecture-Overview.md` PWA section gains history view.
__Out of scope:__ Pagination (TASK-2.5.2), file read-back (TASK-2.5.3), heatmap (TASK-2.5.4–6).
__Risk / inspector hot-spots:__ the bottom-tab nav must coexist with the existing sticky submit bar (form view). When the form view is active, the bottom tab nav must NOT be shown (the sticky submit owns the bottom of the screen).

#### TASK-2.5.2 — Cursor Pagination via `?cursor=`

__Stream:__ D
__Depends on:__ TASK-2.5.1
__Touches:__ `web/app.js` history loader + scroll listener
__Acceptance criteria:__
- [ ] First load fires `GET /api/v1/history?limit=50` (no `since`/`until`/`cursor`) — this returns `summary` per contract §6.5 (used by TASK-2.5.4–6).
- [ ] When the user scrolls within ~200 px of the list bottom AND `has_more === true`, the client fires `GET /api/v1/history?limit=50&cursor=<next_cursor>` and appends to the list. Per contract §6.5: use `cursor`, NOT `until`, for pagination.
- [ ] Concurrent scroll-triggered loads are debounced — only one in flight at a time.
- [ ] Loading more shows a small inline spinner; failure shows "Couldn't load more — tap to retry."
- [ ] No client-side cache of history rows across sessions — each tab visit fetches fresh. (History is small enough; staleness is more dangerous than fetch cost.)
- [ ] Affected docs updated: roadmap §3.5 progress.
__Out of scope:__ Capture read-back (TASK-2.5.3), heatmap (TASK-2.5.4).
__Risk / inspector hot-spots:__ dropping entries at the boundary — contract §6.5 explicitly warns same-millisecond entries (PWA offline-queue replay can produce them) require id-based cursor, not timestamp. Verify the client uses the `next_cursor` field, never derives a cursor from `entries[last].timestamp`.

#### TASK-2.5.3 — Tap Entry to Read back Capture Content

__Stream:__ D
__Depends on:__ TASK-2.5.2
__Touches:__ `web/app.js` history-entry tap handler + read-back panel, `web/styles.css` panel
__Acceptance criteria:__
- [ ] Tap on an entry fires `GET /api/v1/captures/{history_id}` and shows the returned `content` (full UTF-8 file content, frontmatter + body) in a panel.
- [ ] Content is rendered as a `<pre>` with monospace font (TUI aesthetic — and avoids any markdown rendering attack surface). NO HTML rendering, NO sanitization stripping content — the user is reading their own capture.
- [ ] Panel header shows `vault_path` and `transport_mode` (from the response).
- [ ] On 404 (vault file deleted since capture): show "This capture's file no longer exists in the vault."
- [ ] On 502 transport error: show "Couldn't read — vault unreachable."
- [ ] Logging §14: client does NOT log content. Server already complies (handler logs only the history_id).
- [ ] Affected docs updated: roadmap §3.5 progress.
__Out of scope:__ Editing the file (forever out of scope — Pour writes, doesn't edit).
__Risk / inspector hot-spots:__ XSS via content. Even though the user trusts their own vault, a markdown file with `<script>` rendered via innerHTML would execute. Use textContent only, or render in `<pre>`. Verify no `innerHTML` accepts the content string.

#### TASK-2.5.4 — Heatmap Renderer (mobile-portrait-fit)

__Stream:__ D
__Depends on:__ TASK-2.5.1 (and TASK-2.5.5 for data shape)
__Touches:__ `web/app.js` heatmap module, `web/styles.css` heatmap grid + cell colors
__Acceptance criteria:__
- [ ] Heatmap renders the past N days (default: 90; adjustable) as a grid. On a 360 px viewport, the grid is laid out as columns-of-weeks, scrolling horizontally (NOT a 7-row × 53-col GitHub-style block, which doesn't fit portrait).
- [ ] Each cell is a square sized to fit ~12 weeks visible without horizontal scroll on a 360 px viewport. Tap-target meets ≥ 32×32 CSS px (Apple HIG minimum is 44 px; 32 is acceptable for an info-only cell, but flag this for the architect to confirm).
- [ ] Cells colored by capture count: 0 = bg, 1 = light, 2-3 = mid, 4+ = darkest. Use `--accent` family (cyan) per the visual identity overhaul. Ensure ≥ 4.5:1 contrast on the 0/1 boundary against the background.
- [ ] Tap on a cell shows a small popover: "<date>: <count> captures." Tap-and-hold not required.
- [ ] Heatmap respects `prefers-reduced-motion`: no animated transitions on render or update.
- [ ] Heatmap section sits ABOVE the history list in the History tab.
- [ ] Affected docs updated: roadmap §3.5 progress; `System-Architecture-Overview.md` notes heatmap renderer.
__Out of scope:__ Per-module filtering (deferred). Streak ribbons (deferred — `summary.streak_days` is shown as a number, not visualized).
__Risk / inspector hot-spots:__
- Color contrast in dark mode (the cyan-300 accent on near-black) for the lowest non-zero band.
- 360 px width fit — measure on a real device, not just devtools.
- Per memory `feedback_pour_aesthetic.md`: low-friction and high-clarity beat aesthetic when they conflict. If the GitHub-grid feel doesn't fit, prefer a cleaner mobile-native layout.

#### TASK-2.5.5 — Heatmap Data Source Decision (server Aggregate Vs Client rollup)

__Stream:__ D
__Depends on:__ none (can be done in parallel with TASK-2.5.1 — but TASK-2.5.4 depends on its outcome)
__Touches:__ `pour-api-contract.md` (potentially adds §6.5.1 or amends §6.5), `pour-openapi.yaml` (matching), `src/server/handlers/history.rs` (only if a new endpoint), `web/app.js` (data shape)
__Acceptance criteria:__
- [ ] Decision documented in `pour-pwa-roadmap.md` under §3.5 with rationale: either (a) heatmap pulls from existing `/api/v1/history` and rolls up client-side using `entries[].timestamp`, OR (b) a new `/api/v1/history/heatmap?days=N` endpoint returns a precomputed `{ date: count }` map.
- [ ] If (a): no contract change. Document the chosen `limit` value (probably 1000 to cover ~90 days of 10/day captures), and the cursor-pagination loop the heatmap uses if more than `limit` entries fall in the window.
- [ ] If (b): full contract amendment lands FIRST per memory `feedback_contract_first.md` — request/response shape, error codes, body limit, cache-control. Inspector audit before handler implementation.
- [ ] Whichever path is chosen, the data flow is documented in a comment in `web/app.js` near the heatmap renderer so future maintainers don't have to reverse-engineer.
- [ ] Affected docs updated: roadmap §3.5; if (b), contract §6.x AND OpenAPI yaml.
__Out of scope:__ Per-module heatmap (deferred).
__Risk / inspector hot-spots:__
- Path (a) burns up to a few hundred KB on every History tab visit (entries are small but add up). Measure.
- Path (b) introduces a new endpoint surface — inspector audit cost. The benefit is a much smaller payload. The cost is a new contract surface to maintain.
- Architect MUST pick before TASK-2.5.4 ships, but TASK-2.5.4 acceptance can describe the data ingestion contract without naming the endpoint.

#### TASK-2.5.6 — Mobile Tab Nav Final Wiring + Visual Polish

__Stream:__ D
__Depends on:__ TASK-2.5.1, TASK-2.5.4
__Touches:__ `web/index.html` final tab nav structure, `web/styles.css` tab nav polish, `web/app.js` boot path
__Acceptance criteria:__
- [ ] Tab nav is final design: two tabs (Capture, History), labeled in lowercase mono per the wordmark style.
- [ ] Active tab indicator uses cyan accent.
- [ ] Tab nav hidden on form and summary views (only shown on dashboard and history).
- [ ] Tab nav coexists with the queue badge (TASK-2.1.4) in the header without overlap.
- [ ] Initial app boot routes to "Capture" tab (existing dashboard).
- [ ] Browser back button on history view returns to dashboard. Use `history.pushState`/`popstate`, not full reload.
- [ ] Affected docs updated: roadmap §3.5 closure; `v1.0.0-Release.md` Phase-2-deferred bullet for "history heatmap on mobile dashboard" struck through with closure note.
__Out of scope:__ A third tab (settings, etc.) — deferred.
__Risk / inspector hot-spots:__ browser back button behavior — without `pushState`, the back button leaves the PWA, which is hostile.

---

## Open Questions for the Architect

These need a documented decision before or during the relevant task; they are NOT acceptance criteria — they are flags.

1. __IndexedDB schema versioning + migration story__ (TASK-2.1.1). What happens if a future Pour version changes the queue schema while a phone has pending records on the old schema? Migration path? "Drop and warn"? The manifesto's "no capture loss" pillar makes "drop and warn" weak.

2. __Heatmap data source — aggregate endpoint or client-side rollup?__ (TASK-2.5.5). Decide before TASK-2.5.4 ships. Contract amendment cost vs payload cost.

3. __Background Sync API absence on iOS Safari__ (TASK-2.1.3). The fallback `online` event listener is documented, but does it reliably fire after the PWA returns from background? Real-device verification needed.

4. __Idempotency-Key reuse on edit-and-resubmit__ (TASK-2.1.5). Should the user editing a queued submit's body get a fresh key (so the new payload executes) or keep the original key (so a server-cached 201 takes precedence)? Current proposal: KEEP — preserves the duplicate-write protection the queue exists to provide. Confirm.

5. __Long-press conflict with iOS native gestures__ (TASK-2.4.2). `-webkit-touch-callout: none` on chips is necessary. Are there other native gesture conflicts (3D Touch / haptic touch on iPhone) the architect should design for?

6. __Synthetic 202 wire-shape exposure__ (TASK-2.1.2). The contract amendment says "synthetic 202 is client-side only — clients written against the contract directly will never see it." Is this enforceable? An MCP companion (Phase 4) would never see the 202 because it doesn't run through a service worker. The amendment should be precise: "the PWA service worker emits 202 to the page; no server response carries 202." Confirm wording.

7. __Drain order during clock skew__ (TASK-2.1.3). FIFO is `queued_at`-ordered. If two records have identical `queued_at` (rare but possible — same-millisecond), what's the tiebreak? The IDB auto-increment id is deterministic. Document.

8. __Preset mutation while another client is active__ (TASK-2.4.x). The TUI and the PWA both write `~/.pour/presets.json`. Two concurrent writers can race. Existing `src/data/presets.rs` should be checked for atomic-write semantics; if absent, flag for hardening. NOT a Phase 2 task; flag-only.

9. __PWA test infrastructure gap__ (TASK-2.0.1). Phase 2 ships a service worker, an IndexedDB queue, drag-and-drop, and a heatmap with zero JS-side automated test coverage. Flag-and-defer — but at what point does this become a release blocker for v1.0.0?

10. __Heatmap accessibility__ (TASK-2.5.4). Color-only differentiation fails screen readers. Should each cell carry an `aria-label` with date + count? Probably yes; confirm it doesn't tank performance on 90+ cells.

---

## Inspector Hot-spot Summary (top 3)

1. __Idempotency-Key discipline across the offline queue.__ Same key reused on every retry of the same payload (contract §9 round 5). Rotated only on 2xx success or explicit form reset. Verified across: queue-write (TASK-2.1.1), drain (TASK-2.1.3), edit-and-resubmit (TASK-2.1.5). One leak here = duplicate writes, the entire raison d'être of idempotency defeated.

2. __`captured_at` preservation through the offline queue.__ Original wall-clock instant of the user's tap, not drain time. Festival case (capture Friday night, sync Monday) is the whole point. Verified: TASK-2.1.1 stores it, TASK-2.1.3 sends it unmodified, TASK-2.1.5 keeps it on edit. Plus the server-side validation window (30 days past, 5 min future) — a multi-week-old queue could trip it. Document the user-visible "this capture is too old to sync" failure mode.

3. __Sub-form a11y + UX consistency with the TUI.__ Up/Down navigates, Left/Right cycles, NO `<select>` in overlays. Memory `feedback_subform_ux.md` is non-negotiable. Inline cycling MUST show `◂ ▸` per `feedback_inline_cycling.md`. The inspector should verify these on a real screen reader, not just by reading the code.

---

## Out of Phase 2 Scope (remind the architect)

- __Capture trim from PWA__ (roadmap §3.6 / contract §15) — desktop-only, intentionally deferred.
- __TLS__ (roadmap §4.1) — Phase 3.
- __mDNS / `pour.local`__ (roadmap §4.2) — Phase 3.
- __MCP companion__ (roadmap §4.3) — Phase 4.
- __utoipa migration__ (roadmap §5) — separate Phase 2 work item, scheduled but not in this backlog. Triggered after ~1 month with no contract amendments post-Phase 2.
- __PWA JS test infra__ — flagged in TASK-2.0.1, not closed in Phase 2.
- __Preset mutation offline queue__ (TASK-2.4.5) — keep the offline queue scoped to submits.

---

## Change Log

- __2026-04-27__ — Initial backlog scoped from roadmap §3.1–3.5. 27 task cards across 4 streams plus 1 meta task. Dependency graph + open-questions list captured. Status: pending architect-builder execution.
- __2026-04-27__ — Stream B complete: TASK-2.0.1 (PWA test infra gap note), TASK-2.3.1–2.3.5 (sub-form overlay, field rendering, submit wiring, cancel-revert, server-error relay). No Rust changes, no new dependencies. Inspector finding: `is_existing_option` uses case-insensitive comparison on the server; client mirrors this despite spec saying "case-sensitive" — see submission notes.
- __2026-04-27__ — Stream A complete: TASK-2.2.1–2.2.3 (SW route + shell cache + update flow), TASK-2.1.1–2.1.5 (IDB schema, submit intercept, background sync drain, queue badge UI, conflict UX). Contract §6.4 round-6 amendment (synthetic-202 clarification). Open question Q1 (IDB versioning) documented in `web/queue.js` comment. Open question Q3 (iOS Background Sync) handled via `window.online` fallback. Open question Q4 (key reuse on edit) resolved per locked decision: KEEP original key. Open question Q6 (synthetic 202 wording) resolved per amendment text. Open question Q7 (tiebreak) documented in `web/sw.js` drain loop comment. No new Rust dependencies; vanilla JS + native IDB/SW APIs only.
- __2026-04-27__ — Stream C complete: TASK-2.4.1–2.4.5. Inline save-as affordance; long-press/right-click chip edit panel (name + description); PUT-then-DELETE rename (known minor leak: page-close between requests leaves a duplicate — acceptable for v1, user can manually remove); single-tap-confirm delete (2s window); pointer-event drag-reorder with latest-wins serialized PUT; offline guard on all mutations (toast, no queue); re-render always from server response. Open question Q8 (concurrent TUI+PWA writers) remains flagged — not a Phase 2 task. No Rust changes. No new dependencies.
- __2026-04-27__ — Stream D complete: TASK-2.5.1–2.5.6. Bottom-tab nav (capture/history tabs, `role="tablist"`/`role="tab"`/`aria-selected`, cyan active underline, hidden on form/summary via `.tab-nav--hidden`); lazy history view (loads on first tab tap); cursor pagination (`?limit=50`, appends on scroll within 200px, `_historyFetchInFlight` guard, inline spinner, tap-to-retry); capture read-back panel (`<pre>` + `textContent` only, 404/502 error states, no content logged); 90-day heatmap (client-side rollup, columns-of-weeks CSS grid, count tiers 0/1/2-3/4+, `aria-label` per cell, day-of-week labels, tap-popover, `prefers-reduced-motion`); `pushState`/`popstate` browser-back wiring; heatmap fetch loop termination via window-boundary check. Open Q2 (data source) closed: path (a) client-side rollup. No Rust changes. No new dependencies. __Phase 2 COMPLETE.__

### Post-inspector Fixes — Stream A (2026-04-27)

Inspector audit returned FAIL with 5 CRITICAL findings, all closed:

1. __CRITICAL 1 — Dead 507 handler__ (`web/app.js` `handleSubmit`): The `resp.status === 507` check was nested inside `if (resp.status === 202)` — status can never be both. Moved the 507 branch to a sibling check BEFORE the 202 branch. Toast reads "Queue is full — clear discarded captures or sync existing ones first." Dead code path eliminated.

2. __CRITICAL 2 — `_editingQueueId` leak__ (`web/app.js`): `_editingQueueId` was not cleared when the user navigated away from a queue-edit form session. Added `_editingQueueId = null` to: `openForm()` (module tile taps), `loadDashboard()` (back-to-dashboard navigation), and `btn-pour-another` click handler. The variable is now cleared on every non-`editQueueRecord` path that enters or leaves the form view.

3. __CRITICAL 3 — Fresh-install update banner__ (`web/sw.js` `activate`): The `activate` event unconditionally fired `postMessage({type: 'SW_UPDATED'})`. Fixed: `activate` now detects whether old `pour-shell-*` cache keys existed before pruning them. `SW_UPDATED` is only posted when `oldCacheKeys.length > 0` (true update), not on fresh install.

4. __CRITICAL 4 — Stale auth token bricks queued records__ (`web/sw.js`): Chose approach (a) — stop storing the auth token in IDB entirely. `handleSubmitRequest` no longer writes `auth_header` to the queue record. `drainQueue` calls a new `getAuthFromClient()` helper that uses `MessageChannel` to ask an open page client for its current `localStorage.pour_token`. If no client is open, drain defers (returns early); the page re-triggers drain via `DRAIN_NOW` on `DOMContentLoaded` and `window.online`. Page-side `handleSwMessage` handles `GET_TOKEN` and replies on the port. Also added a boot-time `DRAIN_NOW` trigger when an SW is already controlling on page load.

5. __CRITICAL 5 — Dead `#queue-sync-status`__ (`web/app.js`, `web/styles.css`): Wired `setQueueSyncStatus(state)` to all drain state-machine transitions. `DRAIN_STARTED` → "Syncing…"; `DRAIN_FINISHED` + empty queue → "Synced" (auto-clears after 3 s); `window.offline` + panel open → "Network unreachable"; panel open with no network → "Network unreachable". `sw.js` drainQueue now postMessages `DRAIN_STARTED` before iterating and `DRAIN_FINISHED` after the loop. CSS modifier classes added for the three states.

__Test count:__ body limit in `tests/server_static.rs` bumped from 131072 to 524288 (app.js grew past 128 KiB). All 780 tests passing.

---

### Post-inspector Fixes — Stream B (2026-04-27)

Inspector audit returned PASS-WITH-FIXES with 5 MAJOR findings, all closed:

1. __MAJOR 1 — Error routing (name-collision)__ (`web/app.js`): Removed the `templateFieldNames` filter from the `validation_failed` handler. All `details.fields[]` entries now route to the parent form. The overlay never receives errors from this path — only from the explicit `auto_create_input_required` code (handled upstream at the `autoCreateRequired` guard). Comment updated to document the invariant.

2. __MAJOR 2 — Focus trap leak__ (`web/app.js`): Added `!panel.contains(document.activeElement)` check at the top of the Tab handler. If focus has drifted outside the panel, Tab/Shift-Tab now forces it back to first/last focusable before evaluating wrap-around, then returns early.

3. __MAJOR 3 — `static_select` required-validation bypass__ (`web/app.js`): Added `wrapper.dataset.userInteracted = "false"` to `buildInlineCycleControl`; set to `"true"` inside `cycle()` on first ◂/▸/Left/Right interaction. `confirmSubform` reads the flag: a `required: true` field with no configured `default` treats `userInteracted !== "true"` as empty, surfacing a "Required" error.

4. __MAJOR 4 — iOS Safari race / reopen clobber__ (`web/app.js`): Guarded the `openSubformOverlay` call in the `auto_create_input_required` defensive path with `!_overlayContext`. If the overlay is already open, only `showSubformTopError` fires — `renderSubformFields` (which blanks `innerHTML`) is never reached.

5. __MAJOR 5 — Server warning messages echo user content__ (`src/server/handlers/submit.rs` + `web/app.js`): Line 373 message changed to `"failed to sanitize filename"` (no `'{value}'` interpolation). Line 424 message changed to `"failed to create autocreate file"` with the original error moved to `tracing::warn!(error = %e, …)`. Client comment at the warning chip updated to accurately describe the server's code-only guarantee and `textContent` XSS safety.

780 tests passing after fixes.

---

### Post-inspector Fixes — Stream C (2026-04-27)

Inspector audit returned FAIL with 4 CRITICAL + 4 MAJOR findings. 4 CRITICALs + 3 highest-impact MAJORs closed:

1. __CRITICAL 1 — `collectPresetValues` empty-string divergence__ (`web/app.js:2050`): Changed `el.value || ""` written unconditionally to `if (val !== "") values[f.name] = val` (raw, no trim), exactly mirroring TUI `!val.is_empty()` predicate at `src/tui/form.rs:2731`.

2. __CRITICAL 2 — Preset named "order" unreachable__ (`web/app.js` save-as + rename validators; `src/server/handlers/presets.rs`): (a) Client now rejects name "order" (case-insensitive) in both save-as and rename validation blocks with inline error "'order' is reserved — pick another name." (b) Server `put_handler` rejects the name with `400 validation_failed { code: "reserved_name" }` as a belt-and-suspenders guard. (c) Contract §6.8 amended (round 8) + OpenAPI note added. 3 regression tests in `tests/server_presets.rs`.

3. __CRITICAL 3 — Rename clobbers existing preset__ (`web/app.js:1415`): Added pre-check before PUT: `if (nameChanged && _presetsCache.find(p => p.name === newName))` → inline error "A preset named '…' already exists. Pick a different name." Blocks the PUT entirely.

4. __CRITICAL 4 — Long-press fallback creates panel with empty values__ (`web/app.js:1658`): `longPressTimer` callback is now async. On `!chipPreset`, re-fetches presets, updates `_presetsCache`, retries lookup. If still missing after re-fetch: `showToast("Preset state out of sync — refreshing")`, re-renders chip row, returns without opening the edit panel. The synthetic `{ values: {} }` fallback is eliminated.

5. __MAJOR 5 — Active-preset desync after delete or rename__ (`web/app.js:1300–1304, 1475`): (a) `refreshPresetChipRowFromList` now calls `applyPreset(mod, null)` when `_activePreset` is cleared (preset deleted or no longer in list), resetting form fields to defaults. (b) Rename success path sets `_activePreset = newName` BEFORE calling `fetchPresets`/`refreshPresetChipRowFromList`, so the renamed chip renders highlighted.

6. __MAJOR 6 — Reorder 400 leaves DOM in dragged-but-rejected state__ (`web/app.js:1606`): Added `fetchPresets` + `refreshPresetChipRowFromList` call in the `!resp.ok` branch of `fireReorderPut`. Server-canonical order is restored after toast.

7. __MAJOR 7 — Save-as silent overwrite__ (`web/app.js:1978`): Added single-tap-confirm pattern: first save attempt on a name collision sets `saveBtn.dataset.overwritePending = "true"`, shows inline error "Tap Save again to overwrite", auto-resets after 5 s or on any name-input change. Second tap within the window proceeds normally. Per `feedback_pour_aesthetic.md` — single-tap-confirm only, no multi-step modal.

21 `server_presets` tests passing (3 new). No new dependencies.

---

### Post-inspector Fixes — Stream D (2026-04-27)

Inspector audit returned FAIL with 4 CRITICAL findings, all closed:

1. __CRITICAL 1 — Heatmap fetch loop infinite on malformed response__ (`web/app.js` `fetchAndRenderHeatmap`): Added `HEATMAP_MAX_PAGES = 50` constant (50 × 1000 = 50 000 captures; sufficient for any realistic vault). Loop now increments an `iterations` counter and aborts with `console.warn` if it hits the cap. Also added empty-page guard: if `entries.length === 0` on any page (regardless of `has_more`), loop aborts immediately with a `console.warn` — an empty page with `has_more=true` is a malformed server response.

2. __CRITICAL 2 — Memory + listener leak in `renderHeatmap`__ (`web/app.js` `renderHeatmap`): `renderHeatmap` now reuses a single `#heatmap-popover` element via `document.getElementById('heatmap-popover')` — creates once, reuses on every subsequent call. The global dismiss click listener is now guarded by `_heatmapClickListenerAttached` module-level flag (initialized `false`, set `true` on first attach). The handler references `document.getElementById('heatmap-popover')` at call-time (not a stale closure), so it always operates on the live element.

3. __CRITICAL 3 — Browser back from open capture panel skips closure__ (`web/app.js` `openCapturePanel`, `closeCapturePanel`, popstate handler): `openCapturePanel` now pushes Level 2 state `{ view: "history", panel: historyId }` after the panel is shown. `closeCapturePanel(fromPopstate)` takes a boolean: `false` (button close) calls `history.back()` to consume the Level 2 entry; `true` (popstate close) skips `history.back()` to avoid double-pop. The close button wiring passes an explicit arrow `() => closeCapturePanel(false)` (not the function reference directly, which would pass the MouseEvent as a truthy `fromPopstate`). Popstate handler routes on new state shape: `{ view:"history", panel!=null }` → open panel; `{ view:"history", panel==null }` → close panel, stay on history; otherwise → go to dashboard.

4. __CRITICAL 4 — `pushState` guard breaks on reload__ (`web/app.js` `openHistoryTab`): Replaced `history.state?.view !== "history"` guard with module-level `_historyViewPushed` flag (initialized `false` at declaration). `openHistoryTab` pushes once when `!_historyViewPushed` and sets it `true`. The flag is reset to `false` in the popstate handler when navigating away from the history view (Level 1 pop). Because module-level state is always fresh after a reload, this is immune to the persisted-`history.state` bug.

__State machine summary (two levels):__
- Level 1: `openHistoryTab()` pushes `{ view: "history" }` once per session.
- Level 2: `openCapturePanel(id)` pushes `{ view: "history", panel: id }`.
- Back from Level 2 (panel open): popstate fires with `{ view: "history", panel: null }` → `closeCapturePanel(true)`.
- Back from Level 1 (history view): popstate fires with `null` or non-history state → dashboard + reset `_historyViewPushed`.

783 tests passing. No new dependencies.
