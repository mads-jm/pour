---
tags:
  - architecture
  - plan
  - v1
aliases:
  - implementation plan
  - v1 plan
  - task backlog
date created: Monday, March 30th 2026, 12:00:00 am
date modified: Thursday, April 2nd 2026, 8:17:10 am
---

# V1 Implementation Plan

## Guiding Principles

- __Config first, transport second, TUI last__ — each layer is testable independently
- __No task depends on unverified prior work__ — the inspector must sign off before dependents start
- __Atomic tasks__ — each can be implemented and tested in isolation

## Critical Path

```ts
Cargo.toml -> Config parsing -> Transport (API + FS) -> Output pipeline -> Data fetching -> TUI (form -> dashboard -> summary) -> CLI routing (main.rs)
```

## Sprint Overview

| Sprint | Focus | Tasks |
|--------|-------|-------|
| 1 | Foundation | TASK-001 through TASK-003 (Cargo.toml, config types, config parsing + validation) |
| 2 | Transport | TASK-004 through TASK-006 (API client, filesystem writer, transport dispatcher) |
| 3 | Output | TASK-007 through TASK-009 (frontmatter gen, template rendering, write modes) |
| 4 | Data | TASK-010 through TASK-011 (cache layer, dynamic data fetching) |
| 5 | TUI | TASK-012 through TASK-015 (app state, form view, dashboard view, summary view) |
| 6 | Integration | TASK-016 through TASK-017 (CLI routing in main.rs, end-to-end wiring) |

## Task Details

See the Project State maintained by the Governor agent for current status of each task.








