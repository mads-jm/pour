---
tags:
  - architecture
  - adr
  - output
date created: Tuesday, March 31st 2026, 10:03:09 pm
date modified: Tuesday, March 31st 2026, 10:34:06 pm
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

See also [[System-Architecture-Overview]], [[pour-design-spec]], and [[sprint-3-output-pipeline-report]].

