---
tags:
  - index
date created: Tuesday, April 7th 2026, 3:14:07 am
date modified: Saturday, October 3rd 2026, 6:51:38 am
---

# Specs

Feature and component specifications.

## Specifications

- [[pour-design-spec]] — Aspirational design spec; annotate deviations inline
- [[pour-api-contract]] — REST API contract for the PWA companion server (`/api/v1/`). Ratified and shipped in v0.3.0; v1.1.0 additions annotated as deviations.
- [[pour-append-target-recovery]] — Append-target recovery when the daily note is missing (`create_note`, dashboard `⚠ missing`, confirm-on-submit). Not started.
- [[pour-preset-hierarchy]] — Hierarchical drilldown preset picker: `preset_axes` config, data model, UX, overwrite confirm, validation. TUI shipped in v0.3.0; PWA drilldown not started.
- [[pour-lookup-fields]] — Computed/lookup field type that resolves frontmatter from a linked vault note (e.g. `days_off_roast` from bean's `roast_date`). Draft, not started.
- [[pour-review-priors]] — Inline "Priors" review panel: config-declared match/rank/summary of prior captures at capture time. L1 and L1.5 reference columns shipped (TUI, unreleased); L2/L3 not started.
- [[pour-pwa-contract-typing]] — TypeScript contract types for the PWA via JSDoc + OpenAPI codegen. Not started.
- [[pour-pwa-roadmap]] — PWA companion roadmap and phase plan. Phases 1, 1.5 and 2 shipped in v0.3.0; Phase 3 and the utoipa migration not started.
- [[pour-phase2-task-backlog]] — Task cards for PWA Phase 2 (offline queue, service worker, sub-form, preset mutation, history heatmap). Complete, shipped in v0.3.0. Six acceptance criteria deviate, annotated inline.
- [[pour-tui-serve-handoff]] — Press `s` on the dashboard to run `pour serve` inline. Shipped in v0.3.0 with deviations, including a known bug: the 5 s drain timeout stops the server 5 s after it starts.
- [[pour-v1-decomposition]] — v0.3 → v1.0 god-module decomposition, dedup and hardening plan. Complete, shipped in v1.0.0.
- [[pour-fit-module-brief]] — Design brief for a `pour fit` workout module. Draft, not started.
- [[pour-project-standards]] — Project standards: error discipline, pub visibility, file-size budget, write paths, test conventions, workflow, release strategy. Active.
- [[pour-roots-and-hooks]] — Capture beyond the vault: per-module `base_path` override + `post_write_shell` write hooks (auto commit+push), plus a potential `file_select` file-picker expansion. Part 1 shipped in v1.1.0 with deviations annotated; the file picker is not started.
- [[pour-habit-capture]] — Frontmatter mutation primitives: `update` write mode, `toggle`/`counter` field types with goals, date-target resolution (`rollover`/`--date`), one-shot argv capture. Includes the ambient-state-vs-event-note creed. v1 shipped in v1.1.0; v1.1 date targeting and v2 limits not started.

## Index

![[SPECS.base]]