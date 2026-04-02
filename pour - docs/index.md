---
tags:
  - index
date created: Tuesday, March 31st 2026, 12:12:23 am
date modified: Thursday, April 2nd 2026, 8:17:02 am
---

# Pour Documentation

> __For LLMs__: Start here. Navigate to a directory index for scoped exploration, or jump directly to a core document below.

> __For Humans:__ See above… but use the graph to guide your exploration!

---

![[the_pour_manifesto#The Vision Writing More About What Matters]]

---

## Vault Structure

| Directory | Index | Contents |
|-----------|-------|----------|
| `00 index/` | `this note` | Root navigation hub for the vault |
| `01 concepts/` | - | Atomic concept notes and durable project knowledge |
| `02 references/` | [[REFERENCES]] | Library API references and external docs |
| `03 guides/` | - | Developer workflow and implementation guides |
| `04 architecture/` | [[ARCHITECTURE]] | Design spec, ADRs |
| `05 notes/` | [[NOTES]] | Legacy fleeting notes and pre-atomic working notes |
| `06 reports/` | - | Sprint reports and progress snapshots |
| `07 stories/` | [[STORIES]] | Vision and manifesto |
| `08 specs/` | [[SPECS]] | Feature and component specifications |
| `09 milestones/` | - | Release and milestone summaries |
| `99 meta/` | - | Templates and vault maintenance material |

`.obsidian/` is vault configuration and snippet state, not part of the documentation corpus.

---

## Core Documents

- __[[pour-design-spec]]__ — Complete design specification (source of truth)
- __[[the_pour_manifesto]]__ — Why we build Pour
- __[[System-Architecture-Overview]]__ — Concise subsystem map
- __[[v1.0.0-Release]]__ — Current release milestone

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
| Config schema | `~/.config/pour/config.toml` |
| Cache | `~/.cache/pour/state.json` |

---

## Architecture Overview

Pour writes to Obsidian via a __hybrid transport layer__:
1. __API__ — HTTPS via [[reqwest]] to [[obsidian-local-rest-api|Obsidian Local REST API]] (`https://127.0.0.1:27124`, accepts self-signed certs)
2. __File System__ — Direct `std::fs` fallback if API unavailable

### Dynamic Data Fetching (3-tier fallback)

API query -> disk scan -> `~/.cache/pour/state.json` cache -> freetext input

---

__Last Updated__: 2026-03-31
__Documentation Version__: v0.1.0




