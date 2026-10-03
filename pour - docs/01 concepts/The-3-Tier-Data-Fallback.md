---
tags:
  - concept
  - data
  - fallback
date created: Tuesday, March 31st 2026, 10:03:56 pm
date modified: Saturday, October 3rd 2026, 6:47:21 am
---

# The 3-Tier Data Fallback Pipeline

Used for populating `dynamic_select` UI fields in the [[ratatui]] form flow, so a select still has options when the transport can't list its folder.

When the TUI opens a form that requires dynamic data, it calls `fetch_options()` (`src/data/mod.rs`) for each source and waits for it before showing the options. There is no cache-first render and no background refresh. The degradation path has three tiers:

1. __Tier 1 (Transport):__ Attempts to read the live directory via the active [[ADR-001-Hybrid-Transport-Layer|transport layer]] (`API` or `FS`).
2. __Tier 2 (Cache):__ If the transport fails or returns no files, it queries the atomic local JSON cache at `~/.pour/cache/state.json`. Every non-empty transport result overwrites that source's cache entry.
3. __Tier 3 (Empty Fallback):__ If cache is empty or corrupt, it returns an empty vector, dynamically shifting the UI to accept free-text input rather than a strict select list.

This fallback behavior is part of the broader [[System-Architecture-Overview]].

*Note: Results are always normalized to file stems (stripping `.md`) before being cached.*

## Output Side: Inline Creation

When `allow_create = true` on a `dynamic_select` field, the fallback pipeline gains a write path. Novel values entered via freetext (at any tier) trigger auto-creation of bare notes on form submit, seeding the cache for future sessions. See [[Inline-Note-Creation]] for details.




