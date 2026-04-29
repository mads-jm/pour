---
tags:
  - architecture
  - adr
  - mobile
  - pwa
date created: Sunday, April 26th 2026, 6:06:13 pm
date modified: Wednesday, April 29th 2026, 5:31:50 pm
---

# ADR 005: Local-First PWA Companion via `pour serve`

__Date:__ 2026-04-25  
__Status:__ Accepted

__Context:__  
Pour's manifesto opens with an unambiguous premise: *"the time between having a thought and executing the capture must be near zero."* That promise holds at the desk. It evaporates the moment the laptop lid is closed.

The terminal was never the goal — frictionless structured capture was. When the developer is dialing in a shot, standing in a festival crowd, or waking up with a thought at 2am, the terminal is a room (or a bag) away. The gap between "I want to capture this" and "I am at my desk" is exactly the failure mode Pour exists to prevent.

Four manifesto pillars frame what an away-from-desk capture surface must honor: __Velocity is a Feature__ (1-tap PWA cold-open, sub-second); __Capture First, Synthesize Later__ (same fields, same frontmatter, no mobile-specific format); __Plaintext is Forever__ (identical Markdown output, same vault); __Fluidity__ (the phone is a new pour spout, not a new app). A fifth structural fact makes the decision tractable: the engine is already library-shaped. `output::write_create`, `output::write_append`, `Transport::connect`, and every data layer beneath them are pure async functions, entirely detached from the TUI. A second presentation layer over the same engine is a small target.

__Decision:__  
Ship a local-first PWA companion as a second front door in the existing `pour` binary. The architectural choices, in order:

__One binary, two front doors.__ `pour serve` is a new subcommand alongside the existing TUI paths. No separate process, no separate install.

__axum on the existing tokio runtime.__ axum is the idiomatic choice here — lighter than actix-web, composable with Tower middleware, and a natural fit for a codebase already running tokio. The HTTP server and the TUI reuse the same async runtime at the binary level (the subcommand branches before TUI init).

__Static PWA assets embedded via `rust-embed`.__ The single `pour` binary contains both the engine and the browser UI. No `npm install`, no CDN, no asset pipeline for the user. `rust-embed!("web/")` serves from memory; the PWA shell and service worker are present even on an airgapped machine.

__Vanilla HTML/CSS/JS for the frontend, not Rust→WASM.__ The form is fully data-driven from `/api/v1/config`. View logic is small — approximately 300 lines of JS. Rust→WASM frameworks add 200–500 KB of bundle weight, slow first paint on mobile networks, and buy no ergonomic payoff when there is nothing non-trivial to share between the TUI (`ratatui`) and the DOM. Velocity is a feature; the half-second WASM init tax is not acceptable. See [[Rust-WASM-Frontend-Tradeoffs]] for the full analysis.

__LAN-only by design.__ `pour serve` binds `0.0.0.0:<port>`. Off-LAN access is the user's responsibility (Tailscale, ZeroTier). Pour owns the local capture surface, not the network topology.

__Bearer token auth with QR bootstrap.__ On first `pour serve` startup, the server generates a UUIDv4 `mobile_token` (122 bits, URL-safe), stores it in `~/.pour/secrets.toml`, and prints a QR code to the terminal. The phone scans once. After that, the PWA stores the token in `localStorage` and sends it as `Authorization: Bearer <token>` on every request. The `?token=<token>` query parameter is bootstrap-only — consulted only when the `Authorization` header is absent, keeping tokens out of access logs after first contact. All comparisons use `subtle::ConstantTimeEq`; plain `==` on secrets is forbidden.

__All endpoints at `/api/v1/*`.__ `Cache-Control: no-store` on every JSON response. Standard `{"error":{"code","message"}}` envelope on all non-2xx responses. Full endpoint reference in [[pour-api-contract]].

__Idempotency-Key support in Phase 1.__ `POST /api/v1/submit/{module}` accepts an optional `Idempotency-Key` header, backed by an in-memory LRU (capacity ~1024, TTL 5 minutes). This closes the offline-replay correctness story before the offline queue ships in Phase 2: a service-worker retry after a dropped connection replays the same key and gets the original response rather than a duplicate write.

__`captured_at` threaded through every write path.__ The submit body carries the instant the user tapped Submit on the phone. History timestamps, file `date` frontmatter, and all strftime tokens in `path`/`append_template` resolve against `captured_at`, not the server's receipt time. A festival capture queued at 11pm and synced Monday morning is still dated Friday night. When absent, the server falls back to `Local::now()` — identical to TUI behavior today.

__Consequences:__

- __Positive:__ The phone reuses the same engine, the same transport fallback (API→FS), the same `autocreate` pipeline, and the same `History` record. The vault cannot distinguish a TUI submission from a PWA submission — they produce identical Markdown. Every manifesto pillar is upheld. The single-binary deployment means `pour serve` is a first-class install artifact, not a separate service. The MCP companion (Phase 4 roadmap, §15 of [[pour-api-contract]]) can wrap the same HTTP API with a thin shell — the contract was designed with agent-friendly schema discovery (`/api/v1/config`), idempotent submits, and read-back via `/api/v1/captures/{id}` from day one.

- __Negative:__ Binary footprint grows with axum, tower-http, rust-embed, and bundled web assets. The LAN-only design means users who want off-LAN access must configure their own tunnel — this is friction for less technical users, though it is consistent with Pour's target demographic. The options handler loads and saves the cache on every request; under rapid concurrent requests (unusual for a single-user LAN tool) this is nondeterministic. Flagged for Step D follow-up.

- __Mitigations:__ TLS is deferred to Phase 3 with a setup guide for self-signed cert trust on iOS/Android — HTTP on LAN is acceptable for Phase 1 given the single-user, private-network context. The offline submit queue (IndexedDB + service worker drain) is deferred to Phase 2; Phase 1 proves the full engine path before adding the offline complexity layer. The OpenAPI 3.1 spec is hand-written in Phase 1 (`pour - docs/02 references/pour-openapi.yaml`) and replaced in Phase 2 by `utoipa`-derived output from annotated handlers, eliminating drift between the human-readable contract and the runtime types.

__Related:__

- [[ADR-001-Hybrid-Transport-Layer]] — the API/FS hybrid engine the PWA reuses without modification. `Transport::connect` is called in the submit handler exactly as it is called in `main.rs`.
- [[ADR-003-Synchronous-TUI-Async-Operations]] — the blocking-UI tradeoff that was acceptable for v1 TUI. The PWA sidesteps this constraint entirely: axum handlers are natively async, and the phone user has no event loop to block.
- [[pour-api-contract]] — the binding HTTP contract. All endpoint shapes, auth rules, error envelopes, idempotency semantics, and `captured_at` behavior are specified there. This ADR captures the *why*; the contract captures the *what*.
- [[Rust-WASM-Frontend-Tradeoffs]] — companion practitioner's note on when Leptos/Dioxus/Yew/Sycamore would be the right call, and why Pour's current shape does not meet those criteria.
