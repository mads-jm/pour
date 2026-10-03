---
tags:
  - spec
  - decomposition
  - v1
date created: Wednesday, April 29th 2026
date modified: Saturday, October 3rd 2026, 6:47:10 am
status: complete — phases 0–8 done, shipped in v1.0.0
---

# Pour V0.3 → V1.0 Decomposition Plan

The v0.3 → v1.0 window is the *last cheap moment* to decompose god-modules and dedup before the public-surface lock-in. After v1, breaking the foundation has a cost.

User decisions (2026-04-29):
- __Slice 8 (transactional `Config::edit`) is in scope for v1.0.0__ — symptom and root-cause fixes ship together.
- __Tests-first__ for TUI Phase 4 splits.
- __File-size budget enforced by CI__ with `// LINTOK: oversized: <reason>` escape hatch.

---

## Status

__Phases 0 – 4 landed (2026-04-29).__ Phase 5 remains. *[Superseded: Phases 0–8 all landed and shipped as v1.0.0 "The Freeze" on 2026-04-29. See the table below and `CHANGELOG.md`.]*

| Phase | Slices | State |
|-------|--------|-------|
| 0 — Test-first | 0a, 0b, 0c | ✅ done |
| 1 — Foundations | 1, 4, 6, 16, 18 | ✅ done |
| 2 — Dedup | 2, 3, 5, 15 | ✅ done |
| 3 — Server | 7, 9, 10 | ✅ done |
| 4 — TUI | 11a, 11b, 11c, 12, 13, 14 | ✅ done |
| 5 — Lock-in | 8, 17 | ✅ done |
| 6 — v1 Hardening | #1–#6 | ✅ done |
| 7 — Mechanical follow-ups | README sweep, FieldType freeze, size-budget spec entry, FsWriter::base_path doc, cargo doc CI | ✅ done |
| 8 — Judgment-heavy follow-ups | toast feature, project-standards doc, ADR-006, PWA test-infra accept | ✅ done |

`cargo test` after Phase 8: __891 passing, 0 failing, 0 ignored__. Build, clippy, fmt, `cargo doc --no-deps`, and CI file-size check all clean. v1.0.0 is tag-ready pending the user's CHANGELOG / Cargo.toml version bump.

### What Landed (High-leverage Moves)

- __`atomic_replace`__ moved to `src/transport/atomic.rs`; 24 callsites collapsed via the helper there and via `Config::write_atomic` (18 sites in config.rs → 1).
- __Generic `JsonStore<T>`__ at `src/data/json_store.rs` with migration hook; `cache.rs`, `presets.rs`, `field_presets.rs` all delegate.
- __`build_*_updates` dedup__ — main.rs (-197) + configure.rs (-190) → `src/config_updates.rs` (canonical).
- __`src/tui/form.rs` 3852 → form/mod 523__ + `render/{mod,fields,composite}.rs` + `key/{mod,text,select,composite,navigation,submit}.rs` + `overlays/{mod,preset_picker,sub_form,small}.rs`.
- __`src/tui/configure.rs` 2478 → configure/mod 91__ + `render.rs` + `autosave.rs` + `init.rs` + `key/{mod,fields,sub_fields,vault,modules,presets}.rs`.
- __`src/main.rs` 1945 → 205__; event loop + 21 handlers → `src/tui/loop_.rs`.
- __`src/server/mod.rs` 528 → 302__ + `routing.rs` + `static_assets.rs`.
- __`src/server/dto.rs` 627__ → `dto/{mod,response,requests,mapping}.rs`. `mapping.rs` isolates Config→DTO so response.rs is Config-free.
- __`src/server/handlers/submit.rs` 607__ → `submit/{mod,validate,idempotency_lookup,autocreate_step,write_step,history_step}.rs` with a `SubmitContext` struct.
- __`src/data/history.rs` 673 → 474__ + `history_summary.rs` + `history_legacy.rs`.
- __App init helpers moved__ from `src/app.rs` (-298 LOC) to `src/tui/configure/init.rs`; thin wrappers stay on `App` for test compat.

### Tests Added (Phase 0)

- `tests/tui_configure.rs` — 52 cases pinning render dispatch, key routing per mode, auto-save, scroll sync, build_*_updates round-trips, browser nav, confirm dialog. Largest pre-existing test gap, now closed.
- `tests/output/template_snapshot.rs` — 30 cases pinning `render_path` and `render_append_template`. Captured __8 intentional divergences__ between the two functions (see [Template divergences](#template-divergences) below).
- `tests/util_atomic.rs` — 7 cases (1 ignored — Windows non-atomicity demo).
- `tests/data_json_store.rs` — 8 cases.
- `tests/init.rs` — 5 new `module_order` cases.

102 new tests total, all hermetic via `tempfile::tempdir()`.

---

## Phase 5 — Lock-in (Remaining)

*[Done. Both slices shipped in v1.0.0, with the deviations noted below.]*

These are the v1.0.0 surface-freeze deliverables. Both block the v1 tag.

### Slice 8 — `Config::edit()` Transactional Entry point

__Goal:__ add a single load → mutate → validate → persist atomic operation. The 32 pub mutator methods on `Config` become free functions taking `&mut ConfigDraft` that don't persist.

__Pre-condition:__ Slice 3 done ✅ (write_atomic helper exists). Slice 5 done ✅.

__Mechanism:__
- New `src/config_edit.rs` (file, not directory — config.rs stays a flat file): `pub fn edit<F>(&mut self, f: F) -> Result<(), ConfigError> where F: FnOnce(&mut ConfigDraft) -> Result<(), ConfigError>` — load TOML doc → run closure → validate → write_atomic.
- `ConfigDraft` is the in-memory mutable view (likely a wrapper around `toml_edit::DocumentMut` plus parsed `Config`).
- The 32 existing mutators (`update_module_on_disk`, `add_field_on_disk`, etc.) become thin facades: `pub fn update_module_on_disk(…) -> Result<…> { self.edit(|draft| draft.update_module(…)) }`.
- For one release, the facades stay `pub` to avoid breaking external callers; v1.1 demotes them to `pub(crate)` once internal callers all migrate to direct `edit(|…|)` use.

__Risk:__ HIGHEST in plan. Changes Config's mutation API surface. Mitigation: keep the 32 mutators as thin wrappers (no caller-visible churn); only the *implementation* gains the transactional guarantee.

__Reversibility:__ HARD. Once `edit()` is canonical, removing it requires re-spreading 32 callsites.

__Verification:__
- `cargo test` — full suite green. The 18 collapsed atomic-write sites become 1 transactional path.
- New property test: two `edit()` calls cannot interleave on shared `&mut`.
- Spot-check each of the 32 mutators preserves observable behavior.

__Sites to migrate__ (all in `src/config.rs` post-Slice-3): the methods that currently call `Self::write_atomic(…)` directly. Grep `Self::write_atomic` for the canonical list.

*[Deviation: `edit` shipped as an associated function, `Config::edit(path: &Path, f)`, in `src/config_edit.rs`, not a `&mut self` method. The closure gets a `ConfigDraft { doc, parsed }`: the `toml_edit` document to mutate and a parsed snapshot taken at the start, for lookups. 15 public `*_on_disk` mutators route through it, not 32, and they are still `pub` at v1.1.0.]*

### Slice 17 — Curate `lib.rs` Public Surface

__Goal:__ lock the v1.0.0 public surface. Currently every `mod X` in `src/lib.rs` is `pub mod X`, leaking server internals, transport internals, DTOs, etc.

__Pre-condition:__ ALL other slices done (Slice 8 included).

__Mechanism:__
- Demote most `pub mod` to `pub(crate) mod` in `src/lib.rs`.
- Add `pub use` re-exports for items external integration tests legitimately need (`tests/*.rs` will reveal these — each test import becomes either an accepted leak or a `pub use`).
- Gate test-only items (`ApiClient::base_url`, `FsWriter::base_path`, similar) behind `#[cfg(any(test, feature = "test-utils"))]`.

*[Deviation: no `test-utils` feature was added. `FsWriter::base_path` stays `pub` with a doc comment saying it is test-only (Phase 7 below). The demotion to `pub(crate)` landed for `config_updates`, `transport::atomic`, `server::{dto,routing,static_assets}` and `tui::{loop_,render}`.]*

__Risk:__ Will surface accidental coupling in `tests/*.rs`. That's the point.

__Verification:__
- `cargo build --release` and `cargo test` both green.
- `cargo doc --no-deps` shows a curated surface — no accidental internals.
- Audit: every `pub use` in lib.rs is intentional, named, and has a one-line justification comment OR is obviously the public API.

---

## File-size Budget (Active CI Policy)

Enforced by `scripts/check-file-size.sh`, wired into the `file-size` job in `.github/workflows/ci.yml`.

| Tier              | Budget | Notes                                                |
|-------------------|--------|------------------------------------------------------|
| Default cap       | 400    | Advisory                                             |
| Render/UI files   | 600    | Advisory                                             |
| Schema/DTO        | 500    | Advisory                                             |
| __Hard ceiling__  | __800__| __Enforced__: requires `// LINTOK: oversized: <reason>` |

### Files Annotated (Deferred to V1.1)

| File | LOC | Annotation reason |
|---|---|---|
| `src/app.rs` | 1011 | state container + factory; further App split deferred to v1.1 |
| `src/config.rs` | 2759 | Slice 8 (transactional `Config::edit`) target — drops after Phase 5 |
| `src/tui/configure/render.rs` | 1057 | render-tier; cohesive; kept whole |
| `src/tui/loop_.rs` | 1547 | event loop + 21 handlers colocated; per-handler split deferred to v1.1 |

The annotation count is itself a v1.0.0 → v1.1 health metric. Drop them as their files come under budget.

*[As of 2026-10-02 (v1.1.0 plus unreleased work) the same four files carry the annotation, and all four have grown: `app.rs` 1104, `config.rs` 3301, `tui/configure/render.rs` 1059, `tui/loop_.rs` 1786. `config.rs` did not drop under budget after Phase 5.]*

---

## Template Divergences (Preserve in Any Future Template Work)

`render_path` and `render_append_template` share a `substitute_keys()` kernel since Slice 4, but each callsite configures it differently. __All eight divergences are intentional and load-bearing__ — the 30 snapshot tests at `tests/output/template_snapshot.rs` pin them.

| # | Behavior | render_path | render_append_template |
|---|---|---|---|
| 1 | Unknown placeholder | Stripped → empty | Left as-is (literal `{{ghost}}`) |
| 2 | `{{date}}` default format | `%Y%m%d` (configurable) | `%Y-%m-%d` (hardcoded) |
| 3 | Filename sanitization | `sanitize_path_filename` | None |
| 4 | Backslash normalization | `\` → `/` | None |
| 5 | Hidden fields | No concept | Declared-but-hidden → empty |
| 6 | `{{callout}}` token | No support | Resolves from module.callout_type |
| 7 | Composite array expansion | No support | Markdown table |
| 8 | `{{}}` empty placeholder | Stripped → dashes collapsed | Literal `{{}}` |

---

## Out of Scope for V1.0.0

- __`FieldType` as trait__ — freeze the enum; trait migration is v2.
- __`splice_under_heading` extraction__ — not actually duplicated (assessment claim was stale).
- __`Action` enum unification__ — already singular at `src/tui/mod.rs:60`.
- __DTO codegen__ — keep DTOs hand-rolled until contract is ratified.
- __Splitting `dashboard.rs`__ (593 LOC) — within budget and cohesive.
- __`App` as trait__ — Slice 17 narrows the surface; that's enough.
- __Async dynamic-select refresh__ — per [[ADR-003-Synchronous-TUI-Async-Operations]].
- __`insta` snapshot tests__ for golden TUI screens — worth doing post-tag.

---

## Workflow Reminders

- __No commits during execution.__ Slice work lands as unstaged changes; user batch-commits.
- Run `cargo test` after every slice; a failing test halts the slice.
- Each slice is a logical unit of unstaged change — keep slice work separable so the user can stage in coherent batches.

---

## Verification (End-to-end Sign-off before V1.0.0 Tag)

1. `cargo build --release` clean — no warnings.
2. `cargo test` — full suite green. Pre-decomp: 783. Phase-4-end: 871. Post-Slice-8 target: ≥875 (new property tests).
3. `cargo clippy --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. `bash scripts/check-file-size.sh` green; only intentional `// LINTOK: oversized` annotations remain.
6. Manual smoke: `pour init` → `pour me` → submit → file written. `pour serve` → load PWA → submit → history appears. `pour configure` → edit module path → save → reload.
7. `cargo doc --no-deps` — curated surface; no accidental internals leaked.

---

## Phase 6 — V1 Hardening Pass (Post-decomp, Pre-tag)

Inspector audit (2026-04-29) of the `decomp` branch identified __6 merge-blockers__ orthogonal to the decomposition. None are fixed by Slice 8 or 17. All must close before tagging v1.0.0.

*[All six closed in v1.0.0. See `CHANGELOG.md` "Fixed — v1 hardening". The line numbers below are from before the fixes.]*

### Merge Blockers

1. __FS-transport path traversal on the write path__ — `src/transport/fs.rs::resolve_path:31` has zero `..` rejection. `create_file:43`, `append_to_file:65`, `append_under_heading:105`, __and the new `read_file:332`__ (decomposition added a fourth unchecked surface) all use it. Fix: ~10-line rejection of `..` and absolute paths in `resolve_path`, plus 4 tests (one per public method). Carry-over from v0.2.0; Gate 1 blocker.
2. __`atomic_replace` non-atomicity on Windows__ — `src/transport/atomic.rs:27-38`. Slice 1 only relocated; doc comment + ignored test in `tests/util_atomic.rs::windows_non_atomicity_window_exists` still pin the bug. Fix: adopt `fs_err::rename` OR `windows-sys` `MoveFileExW MOVEFILE_REPLACE_EXISTING` gated by `#[cfg(windows)]`; un-ignore the demo test and flip its assertion. Gate 1 blocker.
3. __Cursor arithmetic is byte-indexed__ — emoji/CJK in textareas panics. Now spread across `src/tui/form/key/{text.rs:21,41,59,77, composite.rs:253,360, mod.rs:73,99}`. `move_cursor_vertically` survives intact at `key/mod.rs:99-122`. Fix: convert `cursor_position` to a char-index everywhere (or `floor_char_boundary`). Assessment __High__.
4. __Strftime injection__ — `src/output/template.rs:75, 137`. Both `render_path` and `render_append_template` call `now.format(template)` on the raw user template before placeholder expansion; literal `%X` re-interpreted by chrono. Slice 4 unified the substitution kernel but did not fix the strftime entry point. Fix: substitute placeholders into a sentinel-escaped buffer first, then strftime-expand only known specifiers. Add to `tests/output/template_snapshot.rs`.
5. __CHANGELOG `[Unreleased]` empty__ — `CHANGELOG.md:9-10`. Two commits of decomposition (Phases 0–5: 18→1 atomic-write collapse via `Config::edit`, generic `JsonStore<T>`, three god-module splits, 1547-line event-loop extraction, 102 new tests, file-size CI check) unrecorded. Fix: 30-line writeup; draft v1.0.0 entry. Gate 1 deliverable.
6. __`expect("…verified above")` discipline__ — `src/config.rs:1257, 1881, 2232` plus `src/transport/api.rs:83` (`reqwest client build failed`). Survived Slice 3's collapse. Programming-by-coincidence: a future early-return turns these into panics. Fix: propagate as `Err` (error path already exists upstream).

### Phase 7 — Closed Mechanically (2026-04-29)

- ✅ __README tech-stack sweep__ — added `axum`, `tower`, `tower-http`, `qrcode`, `local-ip-address`, `subtle`, `tracing`, `tracing-subscriber`, `unicode-width`, `uuid`, `dirs`, `anyhow`. Replaced stale "Phase 2 (deferred)" prose with a closeout reference.
- ✅ __Design-spec deviation annotation__ for the file-size-budget CI policy added at `pour - docs/08 specs/pour-design-spec.md` §4.2.
- ✅ __FieldType freeze decision__ recorded at `pour - docs/08 specs/pour-design-spec.md` §4 callout — closed enum at v1.0.0; trait migration deferred to v2.0.0.
- ✅ __`FsWriter::base_path` documented__ as test-only at `src/transport/fs.rs:21-26` — kept `pub` for v1.0.0 (no `test-utils` feature added per earlier decision); revisit at v1.1.
- ✅ __`cargo doc --no-deps` CI job__ — already wired in `.github/workflows/ci.yml::doc` with `RUSTDOCFLAGS=-D warnings`.

### Phase 8 — Judgment-heavy Follow-ups (Need User Direction)

These were flagged by the inspector audit as acceptable to defer past the merge but should land before the v1.0.0 tag. Each needs a scoping call.

*[All four resolved before the tag. Persistence failures now surface as a 5-second status-bar toast. The standards doc is [[pour-project-standards]]. ADR-006 is [[ADR-006-V1-Lock-In-Patterns]]. The PWA test-infra gap was accepted as a known gap in [[v1.0.0-Release]].]*

- __Silent persistence failure UX__ — sites at `src/tui/loop_.rs:402, 429, 451, 646, 793` and `src/data/history.rs:79, 138, 156` swallow save errors via `let _ = …`. `app.deferred_stderr` exists as the outlet but loop_.rs doesn't use it. Decision needed: surface as a status-bar toast on next render? Plumb to a dedicated overlay? Skip until v1.1?
- __Project-standards doc__ at `pour - docs/08 specs/pour-project-standards.md` — Gate 3 deliverable. Suggested scope: (a) error-handling policy (`?` everywhere; `// SAFETY:` for the surviving `expect`s); (b) `pub` discipline post-Slice-17; (c) the 800-LOC ceiling + LINTOK escape hatch; (d) `Config::edit` + `JsonStore<T>` + `transport::atomic::atomic_replace` as the only sanctioned write paths; (e) test-mirroring convention (`tests/<module>.rs` not inline `#[cfg(test)]`). One-page deliverable.
- __ADR-006__ covering this decomposition's load-bearing decisions: `Config::edit()` transactional entry, `JsonStore` migration hook, file-size budget CI policy. Or fold into project-standards doc rather than spinning up a new ADR.
- __PWA test-infra gap__ — Phase 2 closeout Open Q9. v1.0.0 yes/no decision was deferred to tag.

### Trailing Housekeeping (Low Priority)

- LINTOK-annotated oversized files (`src/app.rs`, `src/config.rs`, `src/tui/configure/render.rs`, `src/tui/loop_.rs`) — drop annotations as files come under budget; the count is a v1 → v1.1 health metric.
- `list_directory` and `list_directory_all` in `transport/fs.rs` use the weaker `contains("..")` substring guard rather than the new `resolve_path_validated`'s component-split approach. Inconsistency, not a bug. *[Still true as of 2026-10-02.]*

### V1.0.0 Milestone Gate Status (post-Phase-5 Projected)

*[All three gates closed and v1.0.0 was tagged on 2026-04-29. The test-only `pub` audit in Gate 2 never happened. See [[v1.0.0-Release]].]*

- __Gate 1 (Stability):__ blocked on Hardening #1, #2, #5.
- __Gate 2 (Shape Lock-in):__ closes when Slice 17 lands + FieldType freeze decision is recorded.
- __Gate 3 (Self-sufficient):__ needs README tech-stack sweep + project-standards doc; existing keyboard-shortcut reference and design-spec deviation markers cover the rest.
