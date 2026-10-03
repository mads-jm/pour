---
tags:
  - index
date created: Tuesday, April 7th 2026, 3:14:07 am
date modified: Saturday, October 3rd 2026, 6:51:05 am
---

# Architecture Decision Records

Chronological log of significant architectural decisions.

## Records

- [[ADR-001-Hybrid-Transport-Layer]] — Use Obsidian Local REST API as primary transport with automatic `std::fs` fallback when the vault is closed.
- [[ADR-002-Custom-YAML-Serialization]] — Write custom YAML frontmatter generation instead of `serde_yaml` to guarantee Obsidian Properties compatibility.
- [[ADR-003-Synchronous-TUI-Async-Operations]] — Block the UI thread during network operations in v1; true async TUI is deferred.
- [[ADR-004-API-Append-Read-Modify-Write]] — Replace heading-targeted PATCH append with a GET + in-memory splice + PUT cycle to eliminate the unwanted `***` separator inserted by the API plugin.
- [[ADR-005-PWA-Companion]] — Ship a local-first PWA as a second front door in the same binary: `pour serve`, axum on the existing tokio runtime, assets embedded with `rust-embed`, LAN-only with bearer-token auth.
- [[ADR-006-V1-Lock-In-Patterns]] — Make three patterns the v1.0.0 floor: transactional `Config::edit`, generic `JsonStore<T>`, and a CI-enforced 800-line file-size budget.
- [[ADR-007-Frontmatter-Reader]] — Hand-roll the frontmatter *reader* (companion to ADR-002), scoped to Pour's constrained subset with graceful degradation on richer YAML; no new dependency.

## Index

![[ADR.base]]