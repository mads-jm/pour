---
tags:
  - index
date created: Tuesday, April 7th 2026, 3:14:07 am
date modified: Wednesday, April 29th 2026, 5:31:40 pm
---

# Architecture Decision Records

Chronological log of significant architectural decisions.

## Records

- [[ADR-001-Hybrid-Transport-Layer]] — Use Obsidian Local REST API as primary transport with automatic `std::fs` fallback when the vault is closed.
- [[ADR-002-Custom-YAML-Serialization]] — Write custom YAML frontmatter generation instead of `serde_yaml` to guarantee Obsidian Properties compatibility.
- [[ADR-003-Synchronous-TUI-Async-Operations]] — Block the UI thread during network operations in v1; true async TUI is deferred.
- [[ADR-004-API-Append-Read-Modify-Write]] — Replace heading-targeted PATCH append with a GET + in-memory splice + PUT cycle to eliminate the unwanted `***` separator inserted by the API plugin.

## Index

![[ADR.base]]