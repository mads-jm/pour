---
tags:
  - architecture
  - adr
  - data
date created: Monday, July 13th 2026, 3:00:00 pm
date modified: Monday, July 13th 2026, 3:00:00 pm
---

# ADR 007: Hand-Rolled Frontmatter Reader (companion to ADR-002)

__Date:__ 2026-07-13
__Status:__ Accepted

__Context:__
Until the Priors Review Panel (L1), Pour *wrote* YAML frontmatter (hand-rolled per [[ADR-002-Custom-YAML-Serialization]]) but never *read* it back. The priors FS-scan fallback path (and, next, [[pour-lookup-fields]]) needs to parse the `---`-fenced frontmatter block of on-disk notes into key/value data to filter captures on `match_on` (equality / wikilink). This introduces a frontmatter *reader* for the first time, and with it a reader/writer asymmetry to reconcile against ADR-002.

The API `/search/` path is not affected: Obsidian returns pre-parsed frontmatter via `application/vnd.olrapi.note+json`, so the reader is exercised only on the filesystem-fallback path.

__Decision:__
Hand-roll the reader (`src/data/frontmatter_read.rs`) rather than add a YAML-parser crate (`serde_yaml` or an alternative).

Rationale:
- **Round-trip fidelity is the real risk, and hand-rolling minimises it.** The reader targets the *exact* constrained subset Pour's own writer emits — top-level `key: value` scalars, double-quoted YAML-special values, and block sequences from the writer's comma-expansion. It unescapes precisely the sequences `format_scalar` produces (`\\`, `\"`, `\n`, `\r`), so writer-output → reader-input is faithful by construction. A general crate could parse richer YAML the writer cannot reproduce, re-opening the ADR-002 premise; keeping reader and writer symmetric in scope keeps the asymmetry to a minimum.
- **Maintenance status / dependency weight.** `serde_yaml` is archived/unmaintained upstream; maintained alternatives exist but pull a parser stack for a job the writer already constrains. The hand-rolled reader is ~200 lines with no new dependency.
- **Robustness contract.** Externally-edited notes (Obsidian, manual) can carry richer YAML. The reader must *not crash* on such input but need not fully model it: unrecognised lines (nested mappings, flow collections, anchors, multi-doc) are skipped rather than errored. This is a deliberately narrow, total function — not a general-purpose YAML parser.

__Consequences:__
- **ADR-002 stands unamended.** Its premise (Obsidian-formatting control for *writing*) is unaffected; this ADR is a companion recording the *reader* decision, not a revision. The writer remains authoritative for output format; the reader is scoped to consume that same format plus degrade gracefully on anything richer.
- A reader/writer asymmetry now exists but is bounded: both are hand-rolled, both target the same constrained subset, and their escape/unescape logic is a mirror pair.
- Full YAML-spec coverage is explicitly out of scope. If a future feature needs to read arbitrary YAML frontmatter faithfully, this decision should be revisited (and a maintained crate reconsidered) — flagged here so that trigger is visible.
- Shared foundation: `src/data/frontmatter_read.rs` and `src/data/wikilink.rs` are general utilities reused by [[pour-review-priors]] (L1) and [[pour-lookup-fields]] (next), not private to either.

See also [[ADR-002-Custom-YAML-Serialization]], [[pour-review-priors]], [[System-Architecture-Overview]].
