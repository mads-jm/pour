---
tags:
  - architecture
  - adr
  - output
date created: Tuesday, March 31st 2026, 10:03:09 pm
date modified: Saturday, October 3rd 2026, 6:47:15 am
---

# ADR 002: Custom YAML Frontmatter Generation

__Date:__ 2026-03-31  
__Status:__ Accepted

__Context:__  
Obsidian Properties and Dataview require highly specific YAML frontmatter formatting. Standard serialization crates like `serde_yaml` introduce a runtime dependency for writing and often format arrays or special characters in ways that Obsidian handles poorly.

__Decision:__  
Write a custom `generate_frontmatter` pipeline instead of relying on generic serializers.
* Auto-injects and prioritizes the `date` field.
* Automatically double-quotes values containing YAML-special characters.
* Detects comma-separated string inputs and expands them into YAML lists.

__Consequences:__  
Higher initial maintenance for the formatting logic, but guarantees Obsidian-compatible properties without bloating the binary.

__Note (2026-10-02):__ The third bullet changed in v0.2.1. Comma expansion is opt-in: only a field with `list = true` is split into a YAML list, and every other value is written as one escaped string. Since v1.1.0 the injected `date` takes the module's `frontmatter_date_format` (default `%Y-%m-%d`). The same release added `update` mode, which edits existing frontmatter one line at a time with `patch_frontmatter_line` rather than regenerating the block. Pour still never parses YAML and re-emits it.

See also [[System-Architecture-Overview]], [[pour-design-spec]], and [[v0.1.0-report]] (sprint 3).




