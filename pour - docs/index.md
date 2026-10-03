---
tags:
  - index
date created: Tuesday, March 31st 2026, 12:12:23 am
date modified: Friday, October 2nd 2026, 11:00:00 pm
---

# Pour Documentation

> __For LLMs__: Start here. Navigate to a directory index for scoped exploration, or jump directly to a core document below.

> __For Humans:__ See above… but use the graph to guide your exploration!
> [[BACKLOG.kanban]] is a good place to see where you can help :)

---

![[the_pour_manifesto#The Vision Writing More About What Matters]]

---

## Vault Structure

| Directory          | Index            | Contents                                                                    |
| ------------------ | ---------------- | --------------------------------------------------------------------------- |
| `00 index/`        | `this note`      | Root navigation hub for the vault                                           |
| `01 concepts/`     | [[CONCEPTS]]     | Atomic concept notes and durable project knowledge                          |
| `02 references/`   | [[REFERENCES]]   | Library API references and external docs                                    |
| `03 guides/`       | [[GUIDES]]       | Developer workflow and implementation guides                                |
| `04 architecture/` | [[ARCHITECTURE]] | System overview, ADRs                                                       |
| `05 notes/`        | [[NOTES]]        | Legacy fleeting notes and pre-atomic working notes                          |
| `06 reports/`      | -                | Release reports and assessments (frozen)                                    |
| `07 stories/`      | [[STORIES]]      | Vision and manifesto                                                        |
| `08 specs/`        | [[SPECS]]        | Feature and component specifications                                        |
| `09 milestones/`   | -                | Release and milestone summaries — [[v0.2.0-Foundation]], [[v1.0.0-Release]] |
| `10 PRs/`          | [[PRS]]          | Plaintext persisted GitHub PRs with Context and linking                     |
| `99 meta/`         | -                | Templates and vault maintenance material                                    |

`.obsidian/` is vault configuration and snippet state, not part of the documentation corpus.

---

## Core Documents

- __[[pour-design-spec]]__ — Complete design specification (source of truth)
- __[[the_pour_manifesto]]__ — Why we build Pour
- __[[System-Architecture-Overview]]__ — Concise subsystem map
- __[[pour-preset-hierarchy]]__ — Preset hierarchy drilldown spec (TUI picker, `preset_axes`, ungrouped, validation)
- __[[v0.2.0-Foundation]]__ — Capture-loop foundation (shipped)
- __[[v1.0.0-Release]]__ — Aspirational freeze criteria

---

![[the_pour_manifesto#The Ethos of Pour]]

---

## Quick Reference

### Common Commands

```bash
cargo build              # compile
cargo run                # run dashboard
cargo run -- coffee      # run a specific module
cargo test               # run all tests
cargo clippy             # lint
cargo fmt                # format
```

### Key File Locations

| Area | File |
|------|------|
| Entry point | `src/main.rs` |
| Config schema | `~/.pour/config.toml` (override with `POUR_CONFIG`) |
| State root | `~/.pour/` (override with `POUR_HOME`) |
| Cache | `~/.pour/cache/state.json` |

---

## Architecture Overview

Pour writes to Obsidian via a [[ADR-001-Hybrid-Transport-Layer|__hybrid transport layer__]]:
1. __API__ — HTTPS via [[reqwest]] to [[obsidian-local-rest-api|Obsidian Local REST API]] (`https://127.0.0.1:27124`, accepts self-signed certs)
2. __File System__ — Direct `std::fs` fallback if API unavailable. Append mode splices the entry under its heading on either transport (on disk via a temp file and an atomic rename), and fails if the note or heading is missing. Update mode changes one frontmatter key at a time: a `PATCH` over the API, or on disk a one-line edit behind a stat-before-read guard and an atomic replace. See [[Pour-Types]].

### Dynamic Data Fetching ([[The-3-Tier-Data-Fallback|3-tier fallback]])

Folder listing over the active transport (API, or disk when the API is down) -> `~/.pour/cache/state.json` cache -> freetext input. The form waits for the listing; the cache is a fallback, not a first paint. Novel values entered into [[field-types|`dynamic_select`]] fields trigger [[Inline-Note-Creation|inline note creation]] back into the vault.

---

__Last Updated__: 2026-10-02
__Documentation Version__: v0.1.0






