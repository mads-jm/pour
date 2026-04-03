---
tags:
  - concept
  - data
  - fallback
date created: Tuesday, March 31st 2026, 10:03:56 pm
date modified: Friday, April 3rd 2026, 4:11:42 am
---

# The 3-Tier Data Fallback Pipeline

Used for populating `dynamic_select` UI fields without perceived latency in the [[ratatui]] form flow.

When the TUI initializes a form that requires dynamic data, it executes `fetch_options()` using a strict three-tier degradation path:

1. __Tier 1 (Transport):__ Attempts to read the live directory via the active [[ADR-001-Hybrid-Transport-Layer|transport layer]] (`API` or `FS`).
2. __Tier 2 (Cache):__ If transport fails or is slow, it queries the atomic local JSON cache at `~/.cache/pour/state.json`.
3. __Tier 3 (Empty Fallback):__ If cache is empty or corrupt, it returns an empty vector, dynamically shifting the UI to accept free-text input rather than a strict select list.

This fallback behavior is part of the broader [[System-Architecture-Overview]] and is called out in [[sprint-4-data-fetching-report]].

*Note: Results are always normalized to file stems (stripping `.md`) before being cached.*

## Output Side: Inline Creation

When `allow_create = true` on a `dynamic_select` field, the fallback pipeline gains a write path. Novel values entered via freetext (at any tier) trigger auto-creation of bare notes on form submit, seeding the cache for future sessions. See [[Inline-Note-Creation]] for details.




