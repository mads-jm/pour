---
date created: Monday, April 27th 2026, 5:43:51 pm
date modified: Wednesday, April 29th 2026, 5:31:46 pm
---
# Pour — PWA Contract-typing via JSDoc + OpenAPI Codegen

__Status:__ Drafted 2026-04-27. Not yet scheduled. Intended to branch off `main` when picked up.

## Context

The PWA in `web/` is hand-written JS — 3 files (`app.js` ~3500 lines, `queue.js` ~212 lines, `sw.js` ~541 lines), classic `<script>` tags, no bundler, ~2% JSDoc coverage. The Rust server's HTTP contract is ratified in `pour - docs/02 references/pour-openapi.yaml`. Right now the contract is enforced one-sidedly: Rust honors it, the PWA hopes it does. A field rename or status-code shift in `src/server/handlers/` ships green and breaks silently in the user's pocket — exactly the bug class offline-queue replay is most exposed to.

This plan extends the project's existing contract-first principle to the client. We stay on `.js` (no bundler, no transpile, zero shipped runtime change) but turn on TypeScript's `checkJs` against types generated from the OpenAPI. The bugs caught are envelope-level (field-name typos, wrong DTO shape, unhandled status code, status-code drift after a server change) — exactly the universal ones. Per-config bugs ("module coffee gained a `dose_g` field") remain runtime-validated, same as in the Rust server.

## Constraints Discovered

- `src/server/mod.rs:26-28` uses `rust-embed` with `#[folder = "web/"]` — every file under `web/` ships in the binary. Toolchain artifacts (`package.json`, `node_modules/`, `tsconfig.json`) __must live outside `web/`__. Generated `.d.ts` is small (~50 KB) and harmless if embedded — kept at `web/types/` for natural JSDoc relative imports.
- Repo currently has zero Node.js footprint (no `package.json` anywhere, CI is `cargo`-only at `.github/workflows/ci.yml`). This is a net-new toolchain.
- OpenAPI uses 3.1 features (`type: ["string", "null"]`, `oneOf` with `type: "null"`, `additionalProperties`) — `openapi-typescript@7` handles all three.
- `index.html` loads `queue.js` and `app.js` as classic scripts (no ESM). `// @ts-check` works fine without ESM; JSDoc `@import` is sufficient for type references.

## Decisions

- __Toolchain at repo root__ — `/package.json`, `/tsconfig.json`, `/node_modules/`. `tsconfig.json` `include` points at `web/**/*.js` and `web/types/**/*.d.ts`.
- __CI-gated__ — new `web-typecheck` job in `.github/workflows/ci.yml`. The gate also re-runs codegen and `git diff --exit-code` on `web/types/` to catch stale committed types.
- __All-at-once rollout__ — single PR adds toolchain, generated types, `// @ts-check` + JSDoc to all three JS files, and the CI job. The `app.js` type-error backlog is cleared in this same PR before merge.

## File-by-file Changes

### New Files

- `/package.json` — `private: true`, devDeps: `typescript@^5`, `openapi-typescript@^7`. Scripts:
  - `gen:types`: `openapi-typescript "pour - docs/02 references/pour-openapi.yaml" -o web/types/api.d.ts`
  - `typecheck`: `tsc --noEmit`
  - `verify:types`: `npm run gen:types && git diff --exit-code web/types/`
- `/tsconfig.json` — `allowJs: true`, `checkJs: true`, `noEmit: true`, `target: "ES2022"`, `lib: ["ES2022", "DOM", "DOM.Iterable", "WebWorker"]`, `module: "ESNext"`, `moduleResolution: "Bundler"`, `strict: true`, `noUncheckedIndexedAccess: true`, `include: ["web/**/*.js", "web/types/**/*.d.ts"]`. Note `WebWorker` lib is required for `sw.js`.
- `/web/types/api.d.ts` — generated, committed. Re-export named aliases for the most-used DTOs (`SubmitRequest`, `SubmitResponse`, `HealthResponse`, `ConfigResponse`, `OptionsResponse`, `HistoryResponse`, `Preset`, `ErrorResponse`) on top of the raw `paths`/`components` shapes for ergonomic JSDoc.
- `/web/types/queue.d.ts` — hand-written types for non-API client shapes (IDB queue record, SW message envelope). Not generated — these are PWA-internal contracts.
- `/.github/workflows/ci.yml` — extended (not replaced) with a new `web-typecheck` job parallel to the Rust jobs.

### Modified Files

- `/.gitignore` — add `node_modules/`, `*.tsbuildinfo`, `.eslintcache`.
- `/Justfile` — add `types` (regenerate `.d.ts`) and `typecheck` (run tsc) recipes for local ergonomics.
- `/web/queue.js` — prepend `// @ts-check`. Annotate the existing public functions (`enqueue`, `listQueue`, `dequeue`, etc.) with JSDoc referencing `QueueRecord` from `./types/queue.d.ts`. Existing partial JSDoc gets upgraded to typed.
- `/web/sw.js` — prepend `// @ts-check`. Add `/// <reference lib="webworker" />` and `/** @typedef {ServiceWorkerGlobalScope & typeof globalThis} SwGlobal */` so `self` types correctly. Annotate IDB helpers (mirrors of `queue.js`), the message handler dispatch (`QUEUED`/`DRAINED`/`DRAIN_ERROR`/`DRAIN_FINISHED`/`SW_UPDATED`/`GET_TOKEN`), and the synthetic 202 fetch handler.
- `/web/app.js` — prepend `// @ts-check`. JSDoc the 9 fetch call-sites against generated types. The known type-error backlog will be cleared in this same PR (zero JSDoc today; expect 50–150 errors initially, mostly nullable-narrowing and global state typing).

### Files NOT to touch

- `src/server/mod.rs:26-28` — the `rust-embed` folder macro stays as-is. Embedding `web/types/api.d.ts` is harmless (~50 KB, never served because no client requests it).

## JSDoc Style Anchor

Pattern for fetch wrappers, applied uniformly:

```js
/** @import { paths, components } from "./types/api" */
/** @typedef {components["schemas"]["SubmitRequest"]}  SubmitRequest */
/** @typedef {components["schemas"]["SubmitResponse"]} SubmitResponse */
/** @typedef {components["schemas"]["ErrorResponse"]}  ErrorResponse */

/**
 * @param {string} moduleKey
 * @param {SubmitRequest} body
 * @param {string} idempotencyKey
 * @returns {Promise<SubmitResponse>}
 */
async function submitCapture(moduleKey, body, idempotencyKey) { /* ... */ }
```

The `@import` form (TS 5.5+) keeps the runtime-side import-free. Type-only references; nothing reaches the browser.

## CI Job Shape

New `web-typecheck` job in `.github/workflows/ci.yml`:

```yaml
web-typecheck:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with: { node-version: "20", cache: "npm" }
    - run: npm ci
    - run: npm run verify:types   # regen + git diff --exit-code web/types/
    - run: npm run typecheck      # tsc --noEmit
```

The `verify:types` step is the load-bearing gate: it catches the case where someone edits `pour-openapi.yaml` but forgets to commit the regenerated `.d.ts`, which would otherwise mask drift.

## Verification

End-to-end checks that must pass before this PR is mergeable:

1. `npm install` succeeds on a clean clone.
2. `npm run gen:types` produces `web/types/api.d.ts` with no diff against the committed file.
3. `npm run typecheck` exits 0.
4. __Drift detection sanity check (manual, before merge)__: temporarily rename a field in `pour-openapi.yaml` (e.g. `field_values` → `field_value`), run `npm run gen:types && npm run typecheck`, confirm `tsc` flags every `app.js` site that reads it. Revert.
5. __Stale-types sanity check (manual)__: edit `pour-openapi.yaml` without regenerating, confirm `npm run verify:types` fails with a clear diff. Revert.
6. `cargo build --release` succeeds. Binary size delta should be < 100 KB (the embedded `.d.ts`).
7. `cargo run -- serve` boots; PWA loads in browser; one capture round-trips successfully (golden path: open PWA → fill coffee form → submit → verify file written to vault and visible in `/history`).
8. Service worker offline path: airplane-mode the device, queue a capture, restore network, confirm drain succeeds. Type-checking changed no runtime behavior here.
9. CI passes on the PR (Rust jobs + new `web-typecheck` job).

## Out of Scope

- Migrating `.js` → `.ts` (graduation path; revisit when PWA grows a framework or non-trivial client state).
- ESLint/Prettier (separate concern; not contract-related).
- Generating types from `config.toml` for per-user `field_values` typing (would require a second codegen pipeline; even Rust doesn't do this internally).
- Replacing the hand-written `pour-openapi.yaml` with `utoipa`-generated output (Phase 2 contract item; unrelated to client-side typing).

## Follow-ups when Picked up

- Branch off `main`, e.g. `web/contract-typing`.
- Link this spec from `pour - docs/00 index/` (specs index) and from `pour - docs/08 specs/pour-pwa-roadmap.md`.
- Consider whether to also re-export typed fetch wrappers (a thin `web/api.js` module) so call-sites pick up types automatically without per-site JSDoc — a refinement, not a blocker.
