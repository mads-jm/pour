---
tags:
  - note
  - pwa
  - testing
  - phase2
date created: Monday, April 27th 2026, 3:15:33 pm
status: open — gap acknowledged, closure deferred
date modified: Wednesday, April 29th 2026, 5:31:49 pm
---

# PWA Test Infrastructure Gap

> This note is the artifact for __TASK-2.0.1__. It documents a real, unmitigated gap in test coverage for the PWA layer. It does not propose to close the gap in Phase 2.

## The Gap

`web/` has no automated JS test infrastructure today. There is no:

- __Jest__ (or Vitest, Jasmine) for unit and module-level JS tests
- __Playwright__ (or Puppeteer, Cypress) for end-to-end browser interaction tests
- __Any stub/mock layer__ for `fetch` or IndexedDB in a headless context

Every Phase 2 PWA task ships untested by an automated harness. Verification is manual: real-device or browser-devtools exercising of the feature by the developer.

## Blast Radius

This gap is non-trivial. Phase 2 introduces four distinct complex subsystems in `web/`:

1. __Offline submit queue__ — IndexedDB schema correctness; queue write/drain ordering; `captured_at` preservation; idempotency-key reuse on retry. A schema bug here means silent capture loss — the exact failure mode the manifesto exists to prevent.
2. __Service worker fetch interception__ — the SW must queue on offline/5xx but pass through on 4xx; getting this inverted duplicates discards or swallows real errors. The interception logic has no automated oracle.
3. __Sub-form state machine__ — the overlay open/close/confirm/cancel cycle across five tasks (2.3.1–2.3.5). Novel-value detection, `_pendingAutoCreateInputs` accumulation, cancel-revert, and server-error relay all live in uncovered JS state transitions.
4. __Heatmap aggregation__ — client-side rollup of history entries into a date→count grid. Off-by-one on week boundaries or same-day grouping will produce a silently wrong rendering.

Without an automated harness, a regression in any of these can only be caught by a developer noticing it during manual use.

## What IS Covered

The existing Rust-side `tests/server_*.rs` files DO cover any server contract enforced by this phase:

- `tests/server_submit.rs` — submit validation, `auto_create_input_required` 400 path (the path Stream B satisfies client-side), idempotency cacheability
- `tests/server_integration.rs` — end-to-end health, config, options, history endpoints
- `tests/server_static.rs` — static asset serving

These tests follow the project convention: dedicated files under `tests/` mirroring `src/`, never inline `#[cfg(test)]` blocks. No Rust tests are modified by Stream B (client-only changes).

## Phase 2 Tasks That Would Benefit Most From JS Tests

In priority order, if infra existed:

| Task | What to test |
|------|-------------|
| TASK-2.1.1 | IDB schema: correct store/index creation; record shape; `onupgradeneeded` fires cleanly |
| TASK-2.1.2 | SW fetch interception: offline → queue → synthetic 202; online → passthrough; 4xx → no-queue; 5xx → queue |
| TASK-2.3.x | Sub-form state machine: open/close focus trap; confirm accumulates `_pendingAutoCreateInputs`; cancel reverts parent value; novel-value check matches `is_existing_option` semantics |
| TASK-2.5.4 | Heatmap rollup: correct day-bucket grouping; week boundary edge cases; empty-range output |

## What This Gap Is NOT

This gap does not justify blocking Phase 2 delivery. Manual verification on a real device is the current quality gate, and that is the same gate that shipped all of Phase 1 and Phase 1.5. The risk is __regression risk__, not initial-correctness risk, because there is no automated harness to run after each change.

## Closure Criteria (for a Future phase)

This gap is __NOT being closed in Phase 2__. No Jest install, no Playwright, no Vitest. Reasons:

- No build step in `web/` (vanilla JS with no module system). Adding a test runner requires adopting a bundler or writing the tests against a synthetic DOM — both are non-trivial decisions that deserve their own ADR.
- The PWA is currently a single-person tool used by its author. The friction cost of a broken test environment exceeds the regression cost at current team size.

Closure is deferred to a future phase decision, gated on one of:
1. Team size grows beyond the author (external contributors need CI-level confidence); or
2. A specific high-severity regression occurs that automated tests would have caught.

When that decision is made, it should be preceded by an ADR that picks the test runner, module system (or lack thereof), and coverage targets.

## Related

- [[pour-phase2-task-backlog]] — TASK-2.0.1 (this note's source task)
- [[pour-pwa-roadmap]] — Phase 2 overall
- [[ADR-005-PWA-Companion]] — architectural envelope for the PWA
- [[feedback_test_structure]] — project convention: tests in `tests/` mirroring `src/`, not inline
