---
tags:
  - architecture
  - adr
  - v1
  - lock-in
date created: Wednesday, April 29th 2026
date modified: Wednesday, April 29th 2026
---

# ADR 006: v1.0.0 Lock-In Patterns

__Date:__ 2026-04-29
__Status:__ Accepted

__Context:__
The v0.3 → v1.0 decomposition (see [[pour-v1-decomposition]]) reshaped Pour's foundation across 21 slices and introduced three load-bearing patterns that didn't exist before: a transactional config-write entry point, a generic JSON-backed store, and a CI-enforced file-size budget. Each one closes a structural-debt item from [[v1.0.0-pre-release-assessment]]. Each one outlives a single slice and becomes part of the contract for v1.0.0+ contributors (current and future, human and AI). They are recorded together because the alternative — three separate ADRs — would fragment what is, in practice, one cohesive shift in how the codebase is maintained.

__Decision:__
Adopt three patterns as the v1.0.0 floor. Each replaces an established anti-pattern that was sustainable in 0.x but would calcify after the surface freeze.

### Pattern 1 — Transactional `Config::edit`

`src/config_edit.rs` defines `pub fn edit(path, |draft| { ... }) -> Result<(), ConfigError>`. The closure receives a `ConfigDraft<'_>` holding `&mut DocumentMut` (mutate the TOML doc) and `&Config` (read-only snapshot for validation). The function: reads disk, parses both shapes, runs the closure, validates the resulting Config, atomically writes via `Config::write_atomic`. On any error inside the closure or during validation, the file on disk is unchanged.

The 15 `*_on_disk` mutators on `Config` are thin facades that call `Self::edit(...)`. Their signatures stayed identical for v1.0.0 to avoid caller churn; demoting them to `pub(crate)` is a v1.1 concern.

__Why this matters:__ before this pattern, 18 atomic-write blocks were copy-pasted across `config.rs`, only one of which cleaned up the orphan `.tmp` on rename failure. A bug in any one was a bug in 17 places that would never be found by inspection. With one chokepoint, atomicity, validation, and orphan cleanup are guaranteed by construction.

__Consequence:__ all future config mutations route through `edit()`. Direct `fs::write` to `config.toml` is forbidden. The two `secrets.toml` writers (`write_secret_api_key`, `write_mobile_token`) are intentional exceptions — different file, Unix `chmod 0600` side effect, and migration-path concerns put them outside the `Config` invariant.

### Pattern 2 — Generic `JsonStore<T>` with migration hook

`src/data/json_store.rs` defines `pub struct JsonStore<T>` with `load`, `load_with_migration<F>`, and `save`. Backs `Cache`, `Presets`, `FieldPresets` — three concrete `<T>` instantiations with one shared implementation.

The migration hook (`load_with_migration<F: FnOnce(&str) -> Option<T>>`) is the explicit answer to schema evolution: when the on-disk shape changes, the closure parses the legacy bytes and produces the new `T`. `Presets::load_from` already passes a closure (currently a no-op since no migration has been needed); the wiring is in place for the day it is.

__Why this matters:__ before this pattern, three near-identical implementations of `read_to_string → from_str → unwrap_or_default` and `to_string_pretty → tmp → atomic_replace` existed. Drift between them was inevitable as features (e.g., versioned presets) landed asymmetrically.

__Consequence:__ all future JSON-backed state in `~/.pour/` uses `JsonStore<T>`. Direct `serde_json::to_string` + write is forbidden. The migration hook means a schema bump never silently corrupts state — the closure is the explicit recovery path.

### Pattern 3 — File-size budget, CI-enforced

`scripts/check-file-size.sh` walks `src/**/*.rs`. Any file >800 LOC must carry `// LINTOK: oversized: <reason>` near its top, or CI fails. Tier guidance (advisory, not enforced):

| Tier | Budget |
|---|---|
| Default | 400 LOC |
| Render/UI | 600 LOC |
| Schema/DTO | 500 LOC |
| Hard ceiling | 800 LOC (enforced) |

The annotation count is the maintainer-facing health metric: each annotation is a deferred decomposition the codebase has been honest about. Slices that close one drop the annotation. The count trends down over time or it doesn't — either signal is useful.

__Why this matters:__ before this pattern, three god-modules (`tui/form.rs` 3852, `tui/configure.rs` 2478, `main.rs` 1945) compounded across years. The decomposition closed them; the budget keeps them closed.

__Consequence:__ adding new code that pushes a file over 800 LOC requires either (a) decomposing as part of the change, or (b) adding a LINTOK annotation with a written reason and a planned-decomposition reference. There is no third option.

__Consequences:__
- **For maintainers (human and AI):** three patterns are the *only* sanctioned write paths and the *only* discipline that constrains file growth. Bypassing any of them is a code-review red flag, not a stylistic preference.
- **For the public surface:** `Config::edit` is `pub`; `JsonStore<T>` is `pub`; `transport::atomic::atomic_replace` is `pub`. These are the v1.0.0 contract for any future tool that builds on top of Pour's library shape.
- **For schema evolution:** when a config or JSON state shape changes, the migration path is wired (in `Config::edit` via TOML doc fluency, in `JsonStore::load_with_migration` via the closure). Neither is automatic, but both are explicit.
- **Trade-off:** `Config::edit` adds a small overhead (re-serialize for validation, then again for write — twice per call). For a config mutation path that runs a handful of times per session, this is acceptable. If it ever becomes a hot path, the validation can be moved inside the closure with `&new_content` already in hand.

__Alternatives considered:__
- **`FieldType` as a trait** (instead of the existing closed enum) was on the table for v1.0.0 but rejected — see `pour-design-spec.md` §4. Trait migration is v2.0.0 work.
- **Three separate ADRs** (one per pattern) was the initial framing. Rejected because all three landed together as one cohesive shift; splitting them would fragment the rationale and dilute the "v1.0.0 lock-in floor" framing.
- **Test-utils Cargo feature flag** to gate `FsWriter::base_path`-style accessors was deferred to v1.1 — the v1.0.0 surface keeps them `pub` with documenting comments.

__References:__
- [[pour-v1-decomposition]] — the slice plan that introduced these patterns.
- [[pour-project-standards]] — the contributor-facing rule list that operationalizes them.
- [[v1.0.0-pre-release-assessment]] — the audit that named the anti-patterns these patterns replace.
- [[v1.0.0-Release]] — the milestone gates that reference this lock-in.
- `src/config_edit.rs`, `src/data/json_store.rs`, `scripts/check-file-size.sh` — the implementations.
