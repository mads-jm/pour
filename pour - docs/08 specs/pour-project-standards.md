---
tags:
  - spec
  - standards
  - v1
date created: Friday, May 1st 2026, 6:12:10 am
date modified: Monday, May 4th 2026, 11:17:47 pm
status: active
---

# Pour Project Standards

Maintainer reference for code discipline, workflow, and release process. Audience: future-me and any AI assistant invoked on this codebase. See `CLAUDE.md` for the quick-start build/test/run reference — this doc covers the *why* behind conventions.

---

## 1. Error and Panic Discipline

`?` propagation is the default everywhere in production code paths. Reserve `unwrap()` and `expect()` for compiler-provable invariants only; each surviving call requires a `// SAFETY:` comment above it stating the invariant:

```rust
// SAFETY: split() on a non-empty string always yields at least one element.
let head = parts.first().unwrap();
```

__Boundary errors__ (user input, transport, filesystem, config parse) get typed enum variants so callers can match exhaustively. The model is `ConfigError` at `src/config.rs:327` — typed variants, `Display`, `std::error::Error`, no lossy string conversion at the boundary.

__Programmer errors__ (violated preconditions, logic bugs that should never reach production) get `panic!` directly with a clear message. Do not paper over them with `unwrap_or_default`.

Rule of thumb: if a caller can recover from it, it belongs in a `Result`. If it represents a bug in our code, it's a panic.

---

## 2. `pub` Discipline

Default visibility is `pub(crate)`. Escalate to `pub` only when there is a named external consumer — the test binary, the `pour` binary entry point, or a documented library API surface.

`lib.rs` re-exports are intentional. Demoting one is a breaking change (patch-or-minor depending on context) and requires a deliberate decision, not an incidental cleanup.

Test-only public items carry a doc comment saying so. Example — `FsWriter::base_path` at `src/transport/fs.rs:27` is `pub` to allow integration test assertions; its doc comment says "Exposed for tests." Do not remove it silently.

---

## 3. File-Size Budget

__800 LOC hard ceiling.__ Enforced by `scripts/check-file-size.sh` in CI.

Advisory tier guidance:

| Tier | Limit |
|---|---|
| Default module | 400 LOC |
| Render / TUI draw | 600 LOC |
| Schema / config | 500 LOC |

These are advisory. The hard ceiling is 800. Exceeding any advisory limit is a signal to decompose, not a block.

__Escape hatch:__ place `// LINTOK: oversized: <reason>` near the top of the file. The reason must either reference a tracked decomposition issue or accept the size with explicit rationale (e.g., "generated dispatch table — split would add indirection without clarity").

Files annotated at v1.0.0 are tracked in [[pour-v1-decomposition]] Phase 6.

---

## 4. Sanctioned Write Paths

All durable writes route through one of three primitives. Any other write path needs a written justification (in a code comment or PR description).

__Atomic file replace__ — `crate::transport::atomic::atomic_replace`, re-exported as `crate::util::atomic_replace` (`src/util.rs:1–5`). Single primitive for all raw file replacement. Never call `std::fs::write` directly on a file that another process may read.

__TOML config writes__ — `Config::edit(&path, |draft| { … })` defined at `src/config_edit.rs:25`. Pattern: load → mutate draft → validate → atomic write. The 15 `*_on_disk` mutators on `Config` are thin facades over this; add new ones there, do not open-code the load/write cycle.

__JSON state__ — `crate::data::json_store::JsonStore<T>` (`src/data/json_store.rs:33`) with an optional migration hook. Backs `Cache`, `Presets`, and `FieldPresets`. Use it for any structured JSON under `~/.pour/`.

---

## 5. Test Conventions

Tests live in `tests/<file>.rs` mirroring `src/<file>.rs`. No inline `#[cfg(test)] mod tests` blocks (per `CLAUDE.md`).

__Hermetic state:__ use `tempfile::tempdir()` for every test that touches the filesystem. Never write to `~/.pour/` or any other persistent path from a test.

__Fixture redirection:__ use `POUR_HOME` to redirect the state directory and `POUR_CONFIG` to redirect the config file. Both are documented in `CLAUDE.md`.

__Error assertions:__ match typed variants, not substrings.

```rust
// Correct
assert!(matches!(err, ConfigError::ModuleNotFound { .. }));

// Wrong — fragile, opaque
assert!(err.to_string().contains("not found"));
```

__Snapshot tests__ are encouraged for any rendering or transformation pipeline. See `tests/output/template_snapshot.rs` as the existing pattern. Pin behaviour before refactoring; update snapshots deliberately, not reflexively.

---

## 6. Workflow — Kanban and Branches

__Kanban surface:__ `pour - docs/00 index/BACKLOG.kanban.md` (Obsidian Kanban plugin). Lanes: Inbox → Scoping → Ready → In Progress → In Review → Done.

Each Kanban card maps to one branch. Branch naming: `<type>/<short-desc>`.

| Prefix | Use |
|---|---|
| `feat/` | New user-visible behaviour |
| `fix/` | Bug fix |
| `refactor/` | Internal restructure, no behaviour change |
| `docs/` | Documentation only |
| `chore/` | Tooling, CI, dependency bumps |

Avoid `wip/` — it signals permanence and makes the branch list noisy.

__Branch lifecycle:__ open from `main` → commit incrementally → squash-merge or rebase-merge to `main` → delete branch. No long-lived feature branches except major release tracks (e.g., `web` for the v0.3.0 PWA surface, `decomp` for the v1.0.0 decomposition).

__Card state reflects branch state.__ In Progress = open branch. Done = merged. Keep these in sync.

__AI-assisted work:__ each user-instructed slice produces one or more atomic commits. The user commits everything (never the AI). See `feedback_no_commits` in the project memory.

---

## 7. Release Strategy

Versioning follows SemVer intent for a personal project: `0.x.y` while foundations are moving; `1.0.0+` when stability is promised to future-me-the-maintainer.

__CHANGELOG-driven.__ Every release has an entry in `CHANGELOG.md` under its version heading. The `[Unreleased]` block accumulates between releases. Entries follow Keep a Changelog conventions (`Added`, `Changed`, `Fixed`, `Removed`).

__Tag flow:__

1. All `[Unreleased]` items complete.
2. Write the version section in `CHANGELOG.md`.
3. Bump `version` in `Cargo.toml`.
4. `git tag vX.Y.Z && git push --tags`.
5. Release notes mirror the CHANGELOG section.

__Version bump criteria:__

| Bump | Trigger |
|---|---|
| __Major__ (1.0 → 2.0) | Breaking config schema without migration; removing a public CLI flag; changing `~/.pour/` on-disk layout; incompatible reshape of `lib.rs` public surface |
| __Minor__ (1.0 → 1.1) | New field types; new modules or commands; additive config keys with sensible defaults; new dependencies |
| __Patch__ (1.0.0 → 1.0.1) | Bug fixes; doc-only changes; internal refactors with no observable behaviour change; performance improvements; dependency bumps |

__Pre-release tags__ (`-alpha.N`, `-beta.N`, `-rc.N`) are allowed for v1.0.0 itself. None were used in 0.x.

__`config_version` is decoupled from app version.__ They start aligned at `1.0.0` but drift on purpose — schema lives at a different cadence than the app. Patch digits float freely (`1.0.x` schema vs `1.0.y` app, no required correspondence). Minor digits track each domain's own additive changes. Major digits bump together when a breaking schema change ships in a major app release. The schema constant lives at `Config::CURRENT_CONFIG_VERSION` in `src/config.rs`. See [[pour-design-spec]] §4.1 "Versioning policy" for the full rule.

---

## 8. AI-Assisted Development

`CLAUDE.md` is the source of truth for what any AI assistant needs to know to build in this codebase. Keep it current; this doc cross-references but does not duplicate it.

User preferences that survive across sessions live in the memory files at `~/.claude/projects/D--dev-mads-pour/memory/`. Do not duplicate them here.

Branch/slice plans for AI-driven work live as `pour - docs/08 specs/pour-<topic>.md` per the `feedback_plans_in_repo` convention. An AI assistant that is handed a task should look for an existing spec doc before asking for requirements.

ADR-006 (being drafted concurrently) captures the architectural decision behind this standards document's scope. See `pour - docs/04 architecture/adr/ADR-006-*` once merged.

---

*This document is Gate 3 of the v1.0.0 release criteria. See [[pour-v1-decomposition]] Phase 8 for context.*
