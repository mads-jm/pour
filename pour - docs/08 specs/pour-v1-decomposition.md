---
tags:
  - spec
  - decomposition
  - v1
date created: Wednesday, April 29th 2026
date modified: Wednesday, April 29th 2026
status: phase-4-complete
---

# Pour v0.3 → v1.0 Decomposition Plan

The v0.3 → v1.0 window is the *last cheap moment* to decompose god-modules and dedup before the public-surface lock-in. After v1, breaking the foundation has a cost.

User decisions (2026-04-29):
- **Slice 8 (transactional `Config::edit`) is in scope for v1.0.0** — symptom and root-cause fixes ship together.
- **Tests-first** for TUI Phase 4 splits.
- **File-size budget enforced by CI** with `// LINTOK: oversized: <reason>` escape hatch.

---

## Status

**Phases 0 – 4 landed (2026-04-29).** Phase 5 remains.

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

`cargo test` after Phase 8: **891 passing, 0 failing, 0 ignored**. Build, clippy, fmt, `cargo doc --no-deps`, and CI file-size check all clean. v1.0.0 is tag-ready pending the user's CHANGELOG / Cargo.toml version bump.

### What landed (high-leverage moves)

- **`atomic_replace`** moved to `src/transport/atomic.rs`; 24 callsites collapsed via the helper there and via `Config::write_atomic` (18 sites in config.rs → 1).
- **Generic `JsonStore<T>`** at `src/data/json_store.rs` with migration hook; `cache.rs`, `presets.rs`, `field_presets.rs` all delegate.
- **`build_*_updates` dedup** — main.rs (-197) + configure.rs (-190) → `src/config_updates.rs` (canonical).
- **`src/tui/form.rs` 3852 → form/mod 523** + `render/{mod,fields,composite}.rs` + `key/{mod,text,select,composite,navigation,submit}.rs` + `overlays/{mod,preset_picker,sub_form,small}.rs`.
- **`src/tui/configure.rs` 2478 → configure/mod 91** + `render.rs` + `autosave.rs` + `init.rs` + `key/{mod,fields,sub_fields,vault,modules,presets}.rs`.
- **`src/main.rs` 1945 → 205**; event loop + 21 handlers → `src/tui/loop_.rs`.
- **`src/server/mod.rs` 528 → 302** + `routing.rs` + `static_assets.rs`.
- **`src/server/dto.rs` 627** → `dto/{mod,response,requests,mapping}.rs`. `mapping.rs` isolates Config→DTO so response.rs is Config-free.
- **`src/server/handlers/submit.rs` 607** → `submit/{mod,validate,idempotency_lookup,autocreate_step,write_step,history_step}.rs` with a `SubmitContext` struct.
- **`src/data/history.rs` 673 → 474** + `history_summary.rs` + `history_legacy.rs`.
- **App init helpers moved** from `src/app.rs` (-298 LOC) to `src/tui/configure/init.rs`; thin wrappers stay on `App` for test compat.

### Tests added (Phase 0)

- `tests/tui_configure.rs` — 52 cases pinning render dispatch, key routing per mode, auto-save, scroll sync, build_*_updates round-trips, browser nav, confirm dialog. Largest pre-existing test gap, now closed.
- `tests/output/template_snapshot.rs` — 30 cases pinning `render_path` and `render_append_template`. Captured **8 intentional divergences** between the two functions (see [Template divergences](#template-divergences) below).
- `tests/util_atomic.rs` — 7 cases (1 ignored — Windows non-atomicity demo).
- `tests/data_json_store.rs` — 8 cases.
- `tests/init.rs` — 5 new `module_order` cases.

102 new tests total, all hermetic via `tempfile::tempdir()`.

---

## Phase 5 — Lock-in (remaining)

These are the v1.0.0 surface-freeze deliverables. Both block the v1 tag.

### Slice 8 — `Config::edit()` transactional entry point

**Goal:** add a single load → mutate → validate → persist atomic operation. The 32 pub mutator methods on `Config` become free functions taking `&mut ConfigDraft` that don't persist.

**Pre-condition:** Slice 3 done ✅ (write_atomic helper exists). Slice 5 done ✅.

**Mechanism:**
- New `src/config_edit.rs` (file, not directory — config.rs stays a flat file): `pub fn edit<F>(&mut self, f: F) -> Result<(), ConfigError> where F: FnOnce(&mut ConfigDraft) -> Result<(), ConfigError>` — load TOML doc → run closure → validate → write_atomic.
- `ConfigDraft` is the in-memory mutable view (likely a wrapper around `toml_edit::DocumentMut` plus parsed `Config`).
- The 32 existing mutators (`update_module_on_disk`, `add_field_on_disk`, etc.) become thin facades: `pub fn update_module_on_disk(...) -> Result<...> { self.edit(|draft| draft.update_module(...)) }`.
- For one release, the facades stay `pub` to avoid breaking external callers; v1.1 demotes them to `pub(crate)` once internal callers all migrate to direct `edit(|...|)` use.

**Risk:** HIGHEST in plan. Changes Config's mutation API surface. Mitigation: keep the 32 mutators as thin wrappers (no caller-visible churn); only the *implementation* gains the transactional guarantee.

**Reversibility:** HARD. Once `edit()` is canonical, removing it requires re-spreading 32 callsites.

**Verification:**
- `cargo test` — full suite green. The 18 collapsed atomic-write sites become 1 transactional path.
- New property test: two `edit()` calls cannot interleave on shared `&mut`.
- Spot-check each of the 32 mutators preserves observable behavior.

**Sites to migrate** (all in `src/config.rs` post-Slice-3): the methods that currently call `Self::write_atomic(...)` directly. Grep `Self::write_atomic` for the canonical list.

### Slice 17 — Curate `lib.rs` public surface

**Goal:** lock the v1.0.0 public surface. Currently every `mod X` in `src/lib.rs` is `pub mod X`, leaking server internals, transport internals, DTOs, etc.

**Pre-condition:** ALL other slices done (Slice 8 included).

**Mechanism:**
- Demote most `pub mod` to `pub(crate) mod` in `src/lib.rs`.
- Add `pub use` re-exports for items external integration tests legitimately need (`tests/*.rs` will reveal these — each test import becomes either an accepted leak or a `pub use`).
- Gate test-only items (`ApiClient::base_url`, `FsWriter::base_path`, similar) behind `#[cfg(any(test, feature = "test-utils"))]`.

**Risk:** Will surface accidental coupling in `tests/*.rs`. That's the point.

**Verification:**
- `cargo build --release` and `cargo test` both green.
- `cargo doc --no-deps` shows a curated surface — no accidental internals.
- Audit: every `pub use` in lib.rs is intentional, named, and has a one-line justification comment OR is obviously the public API.

---

## File-size budget (active CI policy)

Enforced by `scripts/check-file-size.sh`, wired into the `file-size` job in `.github/workflows/ci.yml`.

| Tier              | Budget | Notes                                                |
|-------------------|--------|------------------------------------------------------|
| Default cap       | 400    | Advisory                                             |
| Render/UI files   | 600    | Advisory                                             |
| Schema/DTO        | 500    | Advisory                                             |
| **Hard ceiling**  | **800**| **Enforced**: requires `// LINTOK: oversized: <reason>` |

### Files annotated (deferred to v1.1)

| File | LOC | Annotation reason |
|---|---|---|
| `src/app.rs` | 1011 | state container + factory; further App split deferred to v1.1 |
| `src/config.rs` | 2759 | Slice 8 (transactional `Config::edit`) target — drops after Phase 5 |
| `src/tui/configure/render.rs` | 1057 | render-tier; cohesive; kept whole |
| `src/tui/loop_.rs` | 1547 | event loop + 21 handlers colocated; per-handler split deferred to v1.1 |

The annotation count is itself a v1.0.0 → v1.1 health metric. Drop them as their files come under budget.

---

## Template divergences (preserve in any future template work)

`render_path` and `render_append_template` share a `substitute_keys()` kernel since Slice 4, but each callsite configures it differently. **All eight divergences are intentional and load-bearing** — the 30 snapshot tests at `tests/output/template_snapshot.rs` pin them.

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

## Out of scope for v1.0.0

- **`FieldType` as trait** — freeze the enum; trait migration is v2.
- **`splice_under_heading` extraction** — not actually duplicated (assessment claim was stale).
- **`Action` enum unification** — already singular at `src/tui/mod.rs:60`.
- **DTO codegen** — keep DTOs hand-rolled until contract is ratified.
- **Splitting `dashboard.rs`** (593 LOC) — within budget and cohesive.
- **`App` as trait** — Slice 17 narrows the surface; that's enough.
- **Async dynamic-select refresh** — per [[ADR-003-Synchronous-TUI-Async-Operations]].
- **`insta` snapshot tests** for golden TUI screens — worth doing post-tag.

---

## Workflow reminders

- **No commits during execution.** Slice work lands as unstaged changes; user batch-commits.
- Run `cargo test` after every slice; a failing test halts the slice.
- Each slice is a logical unit of unstaged change — keep slice work separable so the user can stage in coherent batches.

---

## Verification (end-to-end sign-off before v1.0.0 tag)

1. `cargo build --release` clean — no warnings.
2. `cargo test` — full suite green. Pre-decomp: 783. Phase-4-end: 871. Post-Slice-8 target: ≥875 (new property tests).
3. `cargo clippy --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. `bash scripts/check-file-size.sh` green; only intentional `// LINTOK: oversized` annotations remain.
6. Manual smoke: `pour init` → `pour me` → submit → file written. `pour serve` → load PWA → submit → history appears. `pour configure` → edit module path → save → reload.
7. `cargo doc --no-deps` — curated surface; no accidental internals leaked.

---

## Phase 6 — v1 Hardening Pass (post-decomp, pre-tag)

Inspector audit (2026-04-29) of the `decomp` branch identified **6 merge-blockers** orthogonal to the decomposition. None are fixed by Slice 8 or 17. All must close before tagging v1.0.0.

### Merge blockers

1. **FS-transport path traversal on the write path** — `src/transport/fs.rs::resolve_path:31` has zero `..` rejection. `create_file:43`, `append_to_file:65`, `append_under_heading:105`, **and the new `read_file:332`** (decomposition added a fourth unchecked surface) all use it. Fix: ~10-line rejection of `..` and absolute paths in `resolve_path`, plus 4 tests (one per public method). Carry-over from v0.2.0; Gate 1 blocker.
2. **`atomic_replace` non-atomicity on Windows** — `src/transport/atomic.rs:27-38`. Slice 1 only relocated; doc comment + ignored test in `tests/util_atomic.rs::windows_non_atomicity_window_exists` still pin the bug. Fix: adopt `fs_err::rename` OR `windows-sys` `MoveFileExW MOVEFILE_REPLACE_EXISTING` gated by `#[cfg(windows)]`; un-ignore the demo test and flip its assertion. Gate 1 blocker.
3. **Cursor arithmetic is byte-indexed** — emoji/CJK in textareas panics. Now spread across `src/tui/form/key/{text.rs:21,41,59,77, composite.rs:253,360, mod.rs:73,99}`. `move_cursor_vertically` survives intact at `key/mod.rs:99-122`. Fix: convert `cursor_position` to a char-index everywhere (or `floor_char_boundary`). Assessment **High**.
4. **Strftime injection** — `src/output/template.rs:75, 137`. Both `render_path` and `render_append_template` call `now.format(template)` on the raw user template before placeholder expansion; literal `%X` re-interpreted by chrono. Slice 4 unified the substitution kernel but did not fix the strftime entry point. Fix: substitute placeholders into a sentinel-escaped buffer first, then strftime-expand only known specifiers. Add to `tests/output/template_snapshot.rs`.
5. **CHANGELOG `[Unreleased]` empty** — `CHANGELOG.md:9-10`. Two commits of decomposition (Phases 0–5: 18→1 atomic-write collapse via `Config::edit`, generic `JsonStore<T>`, three god-module splits, 1547-line event-loop extraction, 102 new tests, file-size CI check) unrecorded. Fix: 30-line writeup; draft v1.0.0 entry. Gate 1 deliverable.
6. **`expect("…verified above")` discipline** — `src/config.rs:1257, 1881, 2232` plus `src/transport/api.rs:83` (`reqwest client build failed`). Survived Slice 3's collapse. Programming-by-coincidence: a future early-return turns these into panics. Fix: propagate as `Err` (error path already exists upstream).

### Phase 7 — Closed mechanically (2026-04-29)

- ✅ **README tech-stack sweep** — added `axum`, `tower`, `tower-http`, `qrcode`, `local-ip-address`, `subtle`, `tracing`, `tracing-subscriber`, `unicode-width`, `uuid`, `dirs`, `anyhow`. Replaced stale "Phase 2 (deferred)" prose with a closeout reference.
- ✅ **Design-spec deviation annotation** for the file-size-budget CI policy added at `pour - docs/08 specs/pour-design-spec.md` §4.2.
- ✅ **FieldType freeze decision** recorded at `pour - docs/08 specs/pour-design-spec.md` §4 callout — closed enum at v1.0.0; trait migration deferred to v2.0.0.
- ✅ **`FsWriter::base_path` documented** as test-only at `src/transport/fs.rs:21-26` — kept `pub` for v1.0.0 (no `test-utils` feature added per earlier decision); revisit at v1.1.
- ✅ **`cargo doc --no-deps` CI job** — already wired in `.github/workflows/ci.yml::doc` with `RUSTDOCFLAGS=-D warnings`.

### Phase 8 — Judgment-heavy follow-ups (need user direction)

These were flagged by the inspector audit as acceptable to defer past the merge but should land before the v1.0.0 tag. Each needs a scoping call.

- **Silent persistence failure UX** — sites at `src/tui/loop_.rs:402, 429, 451, 646, 793` and `src/data/history.rs:79, 138, 156` swallow save errors via `let _ = ...`. `app.deferred_stderr` exists as the outlet but loop_.rs doesn't use it. Decision needed: surface as a status-bar toast on next render? Plumb to a dedicated overlay? Skip until v1.1?
- **Project-standards doc** at `pour - docs/08 specs/pour-project-standards.md` — Gate 3 deliverable. Suggested scope: (a) error-handling policy (`?` everywhere; `// SAFETY:` for the surviving `expect`s); (b) `pub` discipline post-Slice-17; (c) the 800-LOC ceiling + LINTOK escape hatch; (d) `Config::edit` + `JsonStore<T>` + `transport::atomic::atomic_replace` as the only sanctioned write paths; (e) test-mirroring convention (`tests/<module>.rs` not inline `#[cfg(test)]`). One-page deliverable.
- **ADR-006** covering this decomposition's load-bearing decisions: `Config::edit()` transactional entry, `JsonStore` migration hook, file-size budget CI policy. Or fold into project-standards doc rather than spinning up a new ADR.
- **PWA test-infra gap** — Phase 2 closeout Open Q9. v1.0.0 yes/no decision was deferred to tag.

### Trailing housekeeping (low priority)

- LINTOK-annotated oversized files (`src/app.rs`, `src/config.rs`, `src/tui/configure/render.rs`, `src/tui/loop_.rs`) — drop annotations as files come under budget; the count is a v1 → v1.1 health metric.
- `list_directory` and `list_directory_all` in `transport/fs.rs` use the weaker `contains("..")` substring guard rather than the new `resolve_path_validated`'s component-split approach. Inconsistency, not a bug.

### v1.0.0 milestone gate status (post-Phase-5 projected)

- **Gate 1 (Stability):** blocked on Hardening #1, #2, #5.
- **Gate 2 (Shape Lock-in):** closes when Slice 17 lands + FieldType freeze decision is recorded.
- **Gate 3 (Self-sufficient):** needs README tech-stack sweep + project-standards doc; existing keyboard-shortcut reference and design-spec deviation markers cover the rest.
