---
tags:
  - index
date created: Tuesday, April 7th 2026, 3:14:07 am
date modified: Wednesday, April 29th 2026, 5:31:40 pm
---

# Concepts

Atomic concept notes — durable, reusable knowledge about the patterns, mental models, and primitives that make [[the_pour_manifesto|Pour]] what it is. ADRs record *decisions*; concepts record the *vocabulary* those decisions are stated in.

## Atomic Concepts

- [[The-3-Tier-Data-Fallback]] — How [[field-types|`dynamic_select`]] populates without perceived latency: transport → cache → empty/freetext. The read side of the dynamic-data pipeline.
- [[Inline-Note-Creation]] — The write side of the same pipeline: novel values entered into [[field-types|`dynamic_select`]] fields auto-create stub or template-driven notes in the vault before the parent capture is written.
- [[Pour-Types]] — The three kinds of pour, one per write mode: entry (`append`), event (`create`), ambient state (`update`). Carries the creed that decides which kind a signal gets, and why the field name must equal the frontmatter key.

## Foundational Concept Anchors

Some core Pour concepts live in [[ARCHITECTURE|ADRs]] or [[SPECS|specs]] rather than here. Treat these as the canonical reference for the concept; this index just makes the cross-link explicit.

- [[ADR-001-Hybrid-Transport-Layer]] — API-first writes with [[reqwest]] to the [[obsidian-local-rest-api|Obsidian Local REST API]], falling back to `std::fs`. The transport layer is the boundary every other subsystem treats as a black box.
- [[ADR-005-PWA-Companion]] — The phone-pocket companion to the terminal capture surface. Same engine, different shell.
- [[pour-design-spec]] — Source of truth for the capture loop, dashboard, summary view, the three write modes (see [[Pour-Types]]), and field → output mapping.
- [[pour-preset-hierarchy]] — Hierarchical drilldown picker concept (`preset_axes`), as distinct from the legacy linear preset cycler.
- [[pour-api-contract]] — Wire-shape contract that the [[ADR-005-PWA-Companion|PWA companion]] and any future client speaks.
- [[field-types]] — The vocabulary of capture: `text`, `number`, `static_select`, `dynamic_select`, `textarea`, `composite_array`, `toggle`, `counter`, plus modifiers like `wikilink`, `allow_create`, `show_when`, `preset_exclude`, `list`.

## Concept Seeds

Unresolved wikilinks below are intentional — they mark concepts worth promoting to atomic notes when the next mention lands. Click any of them in [[index|Obsidian's graph]] to scaffold the note.

- [[Capture-Reflex]] — The ethos: capture must be a reflex, not a workflow. Sub-second latency from intent to written file.
- [[Atomic-Note-Fallback]] — When [[ADR-001-Hybrid-Transport-Layer|the API is down]] in append mode, Pour writes a timestamped standalone note instead of failing — zero data loss as a non-negotiable.
- [[Sub-Form-Overlay]] — The modal form that opens inside a parent form for [[Inline-Note-Creation|template-driven creation]]. Preserves keyboard flow; never spawns a separate process or window.
- [[Conditional-Visibility]] — `show_when` rules and the [[System-Architecture-Overview|`visible_field_indices`]] discipline that keeps navigation, validation, and output bounded to visible fields.
- [[Idempotency-Key]] — How the [[ADR-005-PWA-Companion|PWA]] survives offline replay without duplicating captures. See [[pour-api-contract]].

## About `CONCEPTS.base`

`CONCEPTS.base` in the `00 index/` directory is an Obsidian Bases database definition that provides a table view of all concepts, filtered and grouped by tag. It is not a markdown document — open it via the Obsidian Bases plugin or view below :)

![[CONCEPTS.base]]