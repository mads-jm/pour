---
tags:
  - architecture
  - adr
  - transport
date created: Friday, April 3rd 2026, 12:36:05 am
date modified: Friday, April 3rd 2026, 4:11:42 am
---

# ADR 004: API Append via Read-Modify-Write

__Date:__ 2026-04-02  
__Status:__ Accepted

__Context:__  
The Obsidian Local REST API's heading-targeted PATCH append (`Operation: append`, `Target-Type: heading`) inserts a `***` thematic break between the existing section content and the appended content. This is a server-side behavior that cannot be suppressed via headers or body formatting. The result is visible clutter in daily notes that accumulates with every `pour me` entry.

The previous implementation used two requests: a GET for the document map (to resolve the full `::` delimited heading path) followed by the PATCH append.

__Decision:__  
Replace the heading-targeted PATCH with a read-modify-write cycle: GET the file as `text/markdown`, splice the new content under the target heading in-memory (using the same insertion logic as the filesystem transport), then PUT the full file back.

__Consequences:__

- __Positive:__ Eliminates the unwanted `***` separator. Append output is now identical regardless of whether the API or filesystem transport is used. Also removes the document-map resolution step, simplifying the code.
- __Negative:__ Introduces a read-modify-write race — if another process writes to the same file between the GET and PUT, that write is lost. This is acceptable because pour is a single-user CLI tool and concurrent writes to the same heading are not a realistic scenario.
- __Neutral:__ Request count stays at two (was GET map + PATCH, now GET file + PUT file). No measurable performance difference.

