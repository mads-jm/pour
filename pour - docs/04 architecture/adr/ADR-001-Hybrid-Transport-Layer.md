---
tags:
  - architecture
  - adr
  - transport
date created: Tuesday, March 31st 2026, 10:02:51 pm
date modified: Thursday, April 2nd 2026, 9:18:41 am
---

# ADR 001: Hybrid Transport Layer (API with FS Fallback)

__Date:__ 2026-03-31  
__Status:__ Accepted

__Context:__  
The core ethos of `pour` is killing friction. Relying solely on the [[obsidian-local-rest-api|Obsidian Local REST API]] means data entry fails if the Obsidian Electron client is closed. Relying solely on the filesystem means missing out on advanced API features when the vault is open.

__Decision:__  
Implement a dual-pronged `Transport` dispatcher.
1. Attempt an HTTP connection to the Local REST API first using [[reqwest]] with a 5-second timeout.
2. If the connection is refused, automatically fall back to direct filesystem writes via `std::fs`.

__Consequences:__  
* __Positive:__ Maximum resilience. The user never loses a log entry due to application state.
* __Negative:__ Feature asymmetry. The API backend returns raw filenames (for example, `latte.md`) while the FS backend returns file stems (`latte`). This requires normalization in [[The-3-Tier-Data-Fallback]].

See also [[System-Architecture-Overview]], [[pour-design-spec]], and [[sprint-2-transport-report]].



