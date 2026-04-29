---
tags:
  - notes
  - architecture
  - frontend
  - wasm
date created: Sunday, April 26th 2026, 6:07:06 pm
date modified: Wednesday, April 29th 2026, 5:31:49 pm
---

# When Rust→WASM Frontends Make Sense (And Why Pour Isn't There Yet)

The natural instinct when you're building a Rust project is to stay in Rust. Leptos, Dioxus, Yew, and Sycamore exist precisely to honor that instinct for the browser layer. They're real tools with real users. This note isn't an argument against them — it's a decision log for why Pour doesn't use them *now*, and a concrete checklist for when that should change.

## The Criteria that Would Tip the Balance toward Rust→WASM

__Sharing non-trivial logic between client and server.__ The canonical case: you have a complex validator, a parser, or a render engine that already exists as Rust code, and you need the exact same behavior in the browser without rewriting it in JS and maintaining two implementations. If Pour ever ships `pour query` — an in-browser dataview-style query engine over the vault — and that engine is a non-trivial Rust crate, WASM is the right call. The alternative is a JS reimplementation that will diverge the first time the query language gains a new operator.

__View logic with rich interactive state that benefits from Rust's type system.__ A real spreadsheet. A graph editor. A timeline scrubber with complex state machines. These are cases where the ergonomics of Rust's type checker pay for themselves in the browser — where `Option<NodeId>` catching a bug at compile time is worth the WASM overhead. A form — even a config-driven form with visibility rules — is not this. The state is a flat `HashMap<String, String>`. JS handles it fine.

__First-class type safety across the network boundary via shared crate types.__ If the server and client can import the same DTOs from a shared crate, you eliminate the serde duplication and get compiler-enforced contract conformance. This is a real productivity win on a team that owns both ends of the wire and values Rust's type system as a design tool. In Pour's current architecture the DTO layer lives in `src/server/dto.rs` and the wire contract is enforced by the inspector audit against the OpenAPI spec — a workable substitute that doesn't require WASM.

__A team where Rust is the first language and JS literacy is a meaningful bottleneck.__ For a solo developer or a small team that thinks in Rust, the cognitive load of context-switching to JS is real. If writing a `<select>` in JS means re-learning how event delegation works, that's a legitimate argument for staying in Rust end-to-end. The "one language" argument is weakest when the JS surface is small and the team knows JS; it's strongest when the alternative is a constant mental mode switch on a large, stateful frontend.

## Why Pour's Current Shape Doesn't Meet Those Criteria

The PWA form is generated entirely from the `/api/v1/config` JSON. The field types are enumerated, the visibility rules are two-operator (`equals`, `one_of`), and the submit shape is a flat `HashMap<String, String>`. The JS to render this is approximately 300 lines. A Rust→WASM framework adds bundle overhead without paying for itself — there is no non-trivial logic to share, no complex state machine to model, no network boundary to close with shared types.

First-paint speed on mobile networks is a manifesto-aligned KPI. "Velocity is a feature" isn't just about tap-to-submit latency — it includes the moment the form appears after the user taps the homescreen icon. WASM initialization adds 200–500 KB to the cold-start payload and a parsing + compilation step before the first pixel renders. On a 4G connection that's a real half-second. Pour's performance bar is "cold-tap to form visible in < 1 second" — WASM makes that target harder, not easier.

The TUI (`ratatui`) and the web view (DOM) are different paradigms. There are no render primitives, no layout concepts, and no event models in common. Sharing Rust types between them would be cosmetic — you'd share a `FieldType` enum but rewrite every display concern from scratch in both contexts anyway. The "shared code" argument collapses to the DTO layer, which the OpenAPI contract already handles.

The engine, server, and binary are already Rust. The "stay in Rust" instinct is honored where it matters — the production surface, the type-safe submit pipeline, the transport layer. The browser is HTML because that's what browsers run natively. Adding WASM doesn't make the project more Rust; it makes the browser layer slower and the build more complex.

## Framework Comparison Snapshot

| Framework | Approx. gzipped bundle | Ecosystem maturity | Build complexity | Debugging story |
|---|---|---|---|---|
| __Leptos__ | ~25–40 KB (SSR-first; CSR adds ~15 KB hydration) | Growing fast; v0.7 stable (2024); real production users | Requires `cargo-leptos` or manual WASM target setup; SSR adds server-side complexity | Browser DevTools + `console_error_panic_hook`; source maps partial; Rust panics become JS exceptions |
| __Dioxus__ | ~100–200 KB gzipped (desktop+web multiplatform focus adds weight) | Active; v0.6 (2025); smaller community than Leptos | `dx` CLI manages WASM build + optional hot-reload; simpler than Leptos for pure CSR | Similar to Leptos; `dioxus-devtools` exists but is early; panic messages are usable |
| __Yew__ | ~200–300 KB gzipped (older runtime, no streaming) | Older and stable; less active development since 2023; production-ready but not fast-moving | Standard `wasm-pack` + `trunk`; well-documented; no surprises | Decent; `yew-devtools` browser extension available; source maps work |
| __Sycamore__ | ~30–60 KB gzipped (fine-grained reactivity similar to Leptos) | Smaller community; v0.9 in development; less battle-tested | `trunk` build tool; straightforward for CSR | Weakest of the four; limited tooling; panics surface as opaque JS errors without extra setup |

Vanilla JS for Pour's current scope: __< 5 KB gzipped__ for the form logic. The delta is not close.

## If Pour Ever Does X, Reconsider This

- __`pour query` ships as an in-browser query engine__ — if the query evaluation logic exists as a Rust crate and needs to run in the browser without a server round-trip per query, that's the moment. Rewriting a parser in JS to avoid WASM is not a trade worth making.

- __In-browser visualization with complex interactive state__ — a simple heatmap (SVG or canvas, ~50 lines of JS) doesn't qualify. A scrubbing timeline with rich state, a graph of capture relationships, or a zoomable dashboard where layout is computed do. If the view layer grows to where Rust's type checker would catch real bugs, revisit.

- __Shared validation rules in a custom DSL__ — if Pour ever has a field validation language that must be enforced on both client and server identically, a shared Rust crate is the single-source-of-truth solution, and WASM is what gets it to the browser.

- __Team transition where most contributors are Rust-first__ — if Pour gains contributors and those contributors are Rust engineers who don't want to context-switch to JS for the browser layer, the ergonomic cost shifts. The "one language" argument gains weight proportionally to team size and JS-unfamiliarity.

## Honest Counterpoint

There's a real argument this note doesn't fully honor: the ergonomic and aesthetic value of one-language stacks for solo and small-team projects.

For a developer who thinks in Rust, writing vanilla JS means living in two mental models. The type checker disappears at the browser boundary. Null checks are manual. Refactors don't propagate. For someone who values those guarantees, the 200 KB WASM overhead might be a price worth paying — not because of bundle size math, but because writing JS is genuinely unpleasant and the Rust version would be more maintainable for them specifically.

Bundle size isn't always decisive. On a LAN tool running on the local machine, the "mobile network first paint" argument is weaker than it sounds — the phone is on the same wifi as the server, and round-trip latency is measured in milliseconds. The manifesto's "velocity is a feature" applies to the capture flow, not strictly to the developer experience of building the tool.

If Pour becomes a hobby playground where the joy of writing Rust end-to-end outweighs the practical constraints, this calculus shifts. The criteria above are grounded in Pour's current profile as a velocity tool. A project optimizing for developer delight over milliseconds-to-first-paint is a different project — and for that project, Leptos is a reasonable answer.

The checklist above is the honest prompt: revisit when the project's shape changes, not when the instinct to stay in Rust gets louder.
