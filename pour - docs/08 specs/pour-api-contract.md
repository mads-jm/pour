---
tags:
  - spec
  - api
  - contract
  - mobile
aliases:
  - pour api contract
  - pour mobile api
  - api spec
date created: 2026-04-25
date modified: Saturday, October 3rd 2026, 6:47:14 am
status: ratified — shipped in v0.3.0, amended through v1.1.0; deviations annotated
---

# Pour HTTP API — Contract Specification

> __Status:__ RATIFIED on 2026-04-25. All sections are binding. Steps B–G implement against this contract; the inspector validates conformance; deviations require a contract amendment first.

## 1. Purpose & Scope

This document defines the HTTP interface exposed by `pour serve` for the [[pour - PWA companion]]. It is the single source of truth for:

- Request/response shapes
- Status codes
- Error envelope
- Auth
- Content negotiation
- Timing semantics (incl. offline-queue capture time)
- Idempotency

Both the Rust server (`src/server/`) and the PWA client (`web/`) MUST conform. New endpoints require a doc update first.

__This contract is *internal* to Pour.__ It is not a public API. There is no compatibility guarantee outside the bundled PWA. Versioning exists to coordinate the server and the bundled PWA across Pour releases.

## 2. Versioning — LOCKED

All endpoints are namespaced under `/api/v1/`.

- __Why v1 from day one__: cheap. Pour itself is pre-1.0; the API contract may change shape independently of the binary version. Renaming routes later costs more than carrying `/v1` now.
- The PWA reads `/api/v1/health` on load and refuses to render if the server returns a major version mismatch (`schema_version` field in health response).
- A future `/api/v2/` may coexist with `/api/v1/` during transitions; the server can mount both.
- No alias from `/api/health` to `/api/v1/health` — keep one true path. The PWA always calls the versioned path.

## 3. Authentication — LOCKED (Step A)

Every `/api/*` request MUST authenticate. Static assets at `/` and the manifest do NOT require auth (the PWA shell is fetched before login is possible; auth is enforced on the JSON API).

__Token storage:__ `~/.pour/secrets.toml` `mobile_token` field (UUIDv4 simple format, 122 bits hex). Override via `POUR_MOBILE_TOKEN` env var.

__Two acceptance modes, header is authoritative:__

1. `Authorization: Bearer <token>` — __authoritative__. If present and the suffix is non-empty, the server uses *only* this. A wrong header value returns `401` regardless of any query string.
2. `?token=<token>` query parameter — __bootstrap-only__. Consulted only when the `Authorization` header is absent or has an empty Bearer suffix. Used by the QR-code first-visit URL. After first contact, the PWA stores the token in `localStorage` and switches to the header.

__Comparison:__ constant-time (`subtle::ConstantTimeEq`). Plain `==` is forbidden.

__Failure response:__ `401 Unauthorized` with the standard error envelope and `code: "unauthorized"`. No `WWW-Authenticate` header (we are not implementing an auth challenge flow).

## 4. Transport, Content Negotiation, Encoding — LOCKED

- __Bind__: `0.0.0.0:<port>` (LAN-only by design). Off-LAN is the user's responsibility (Tailscale/ZeroTier).
- __Scheme__: HTTP. TLS deferred to Phase 3.
- __Request `Content-Type`__: `application/json; charset=utf-8` for bodies. Empty body for `GET`/`DELETE`. Unknown content types → `415 Unsupported Media Type`.
- __Response `Content-Type`__: `application/json; charset=utf-8` for all `/api/*` responses (including errors).
- __Encoding__: UTF-8 only. No `gzip` / `br` in Phase 1; payloads are small. Revisit if `/api/v1/history` exceeds ~50KB typical.
- __Trailing slashes__: rejected. `/api/v1/health/` returns 404. The PWA never appends them.
- __Methods__: only those listed per endpoint. Unsupported methods on a known path return `405 Method Not Allowed` with an `Allow` header listing accepted methods.

## 5. Standard Response Envelopes — LOCKED

### 5.1 Success

There is no wrapping envelope. Each endpoint returns its own typed body. Successful status codes:

| Method | Success status |
|--------|----------------|
| GET    | `200 OK` |
| POST (create) | `201 Created` (with `Location` header where applicable) |
| POST (action, no resource) | `200 OK` |
| PUT    | `200 OK` (upsert) |
| DELETE | `204 No Content` |

### 5.2 Error

Every non-2xx response uses this envelope:

```json
{
  "error": {
    "code": "string-machine-readable-slug",
    "message": "Human-readable single-line message.",
    "details": { /* optional, endpoint-specific structured data */ }
  }
}
```

Standard `code` values:

| Code | HTTP | Meaning |
|------|------|---------|
| `unauthorized` | 401 | Missing or wrong token |
| `not_found` | 404 | Resource does not exist (unknown module, unknown preset, etc.) |
| `method_not_allowed` | 405 | Wrong HTTP verb on a valid path |
| `validation_failed` | 400 | Request body or path parameters violate the schema |
| `unsupported_media_type` | 415 | Wrong `Content-Type` |
| `payload_too_large` | 413 | Body exceeds limit (default: 1 MiB; configurable later) |
| `transport_error` | 502 | API transport failed (Obsidian REST API unreachable AND filesystem fallback unconfigured) |
| `write_error` | 500 | Filesystem or REST API write failed |
| `internal_error` | 500 | Unhandled error |

__Validation errors__ include a `details.fields` array describing each rejected field:

```json
{
  "error": {
    "code": "validation_failed",
    "message": "Submit rejected because required fields are empty.",
    "details": {
      "fields": [
        { "field": "bean", "code": "required" },
        { "field": "ratio", "code": "invalid_number", "value": "1::15" }
      ]
    }
  }
}
```

Per-field `code` values in use: `required`, `invalid_number`, `invalid_toggle` (a `toggle` value that is not a boolean word), `invalid_counter` (a `counter` token that is neither `N` nor `=N`). A malformed value token is always a `400 validation_failed`, never a `500 write_error`.

## 6. Endpoint Reference

### 6.1 `GET /api/v1/health` — LOCKED

Liveness + capabilities probe. Already implemented in Step A; this section documents the response shape.

__Auth:__ required.

__Response 200:__

```json
{
  "ok": true,
  "version": "0.2.2",
  "schema_version": "1",
  "transport_mode": "API",
  "vault_base_path": "/path/to/vault",
  "capabilities": [
    "composite_array",
    "create_template",
    "post_create_command",
    "show_when",
    "presets",
    "history",
    "idempotency_key",
    "captured_at"
  ]
}
```

- `version` — Pour binary version (from `CARGO_PKG_VERSION`).
- `schema_version` — API schema major version (currently `"1"`). Mismatch = client should refuse to render.
- `transport_mode` — `"API"` or `"FileSystem"`. The PWA surfaces this in a status pill.
- `vault_base_path` — for display only. Phone never reads vault files directly.
- `capabilities` — feature flags so future PWA versions can detect older servers.

*[Deviation: the `capabilities` list has not changed since v0.3.0. It does not advertise `update` mode or the `toggle`/`counter` field types added in v1.1.0, so a client cannot detect them from `/health`.]*

### 6.2 `GET /api/v1/config` — LOCKED

Returns the full module/field/template schema. The PWA renders forms from this.

__Auth:__ required.

__Response 200:__

```json
{
  "modules": [
    {
      "key": "coffee",
      "display_name": "Coffee",
      "icon": "☕",
      "mode": "create",
      "fields": [ /* see §7 */ ],
      "callout_type": null,
      "append_under_header": null,
      "append_template": null,
      "append_shallow": false,
      "daily_link": false
    }
  ],
  "module_order": ["coffee", "me", "music"],
  "templates": {
    "bean": {
      "path": "Coffee/Beans/{{name}}.md",
      "fields": [ /* template field defs, see §7.4 */ ]
    }
  },
  "vault": {
    "date_format": "%Y%m%d",
    "transport_mode": "API"
  },
  "config_version": "0.3.0"
}
```

- `modules` is an __array, not a map__, ordered per `module_order` with unlisted modules sorted alphabetically. Order matters for dashboard tile rendering.
- `mobile_visible` defaults to `true`. A module with `mobile_visible = false` in `config.toml` is omitted from this response entirely (not included with the flag set false). The PWA cannot reveal hidden modules. __Note: `mobile_visible` is NOT echoed in the module object__ — it controls inclusion, not data. Clients must not depend on its presence.
- `module_order` echoes only the keys that survive the `mobile_visible` filter. A hidden module is removed from both `modules` AND `module_order`. This guarantees the rendering order has no phantom keys.
- `field_type` strings are the lowercase `snake_case` enum names: `"text"`, `"textarea"`, `"number"`, `"static_select"`, `"dynamic_select"`, `"composite_array"`.

*[Deviation: since v1.1.0, `mode` can also be `"update"` and `field_type` can also be `"toggle"` or `"counter"`. The field object does not carry the counter-only `unit` and `goal` keys. The bundled PWA has no widget for either type, which is why the `habit` preset sets `mobile_visible = false`.]*
- `target` is `"frontmatter"` or `"body"` or `null` (use field-type default).
- `show_when` rules ship verbatim — the client evaluates them (see §8).

__Caching__: response is `Cache-Control: no-store`. Config can change at runtime via the TUI configurator; the PWA refreshes on every navigation to the dashboard.

### 6.3 `GET /api/v1/options/{module}/{field}` — LOCKED

Resolves the 3-tier fallback for a `dynamic_select` field. Used by the PWA when opening a dropdown.

__Auth:__ required.

__Path params:__
- `module` — module key, e.g. `coffee`
- `field` — field name, e.g. `bean`

__Query params:__ none.

__Response 200:__

```json
{
  "options": ["Ethiopia Guji", "Yirgacheffe", "Sumatra Mandheling"],
  "source_path": "Coffee/Beans",
  "tier": "transport"
}
```

- `tier` — which of the 3 tiers served the result: `"transport"`, `"cache"`, or `"empty"`. Lets the PWA show "live" vs "cached" badge if desired.
- `source_path` — echoed back so the PWA can label "Beans / Coffee" in the dropdown header.

__404__ if the module or field does not exist. __400__ with `code: validation_failed` if the field is not a `dynamic_select`.

### 6.4 `POST /api/v1/submit/{module}` — LOCKED (The Keystone)

Executes a form submit. Reuses the engine's `autocreate::run` + `output::write_create` / `output::write_append` + `History::record` pipeline.

*[Deviation: since v1.1.0 the pipeline also calls `output::write_update` for `update` modules, then runs the module's `post_write_shell` hook, but only when `post_write_shell_on_serve = true`. A module with a `base_path` or `platform` root override always writes over the filesystem, and the 201 reports `transport_mode: "FileSystem"` for it.]*

__Auth:__ required.

__Headers:__
- `Content-Type: application/json; charset=utf-8` (required)
- `Idempotency-Key: <opaque-string>` (optional) — see §9

__Request body:__

```json
{
  "field_values": {
    "bean": "Ethiopia Guji",
    "dose_g": "18",
    "yield_g": "36",
    "ratio": "1:2",
    "method": "Espresso",
    "notes": "Bright, blueberry, mid-body."
  },
  "composite_data": {
    "ingredients": [
      ["flour", "200", "g"],
      ["water", "150", "ml"]
    ]
  },
  "callout_overrides": {
    "notes": "tip"
  },
  "callout_titles": {
    "notes": "First taste"
  },
  "auto_create_inputs": {
    "bean": {
      "roaster": "Onyx",
      "origin": "Ethiopia",
      "process": "Washed",
      "roast_level": "Light",
      "bag_weight_g": "250"
    }
  },
  "captured_at": "2026-04-25T14:33:02Z",
  "client_id": "phone-pwa"
}
```

__Field semantics:__

| Key | Type | Required | Meaning |
|-----|------|----------|---------|
| `field_values` | `map<string, string>` | yes | All values are strings. Numbers cross the wire as strings (`"18"`, not `18`) — this matches the engine's internal model and avoids JSON number/string coercion. Empty strings = field not filled. |
| `composite_data` | `map<string, string[][]>` | when present | Outer key = composite_array field name; inner = rows of cell values, all strings. Empty rows are stripped server-side. |
| `callout_overrides` | `map<string, string>` | optional | field_name → callout_type for textarea fields with runtime callout cycling. |
| `callout_titles` | `map<string, string>` | optional | field_name → callout title. |
| `auto_create_inputs` | `map<string, map<string, string>>` | optional | When a `dynamic_select` field has `create_template` and the user typed a novel value, this carries the sub-form values. Outer key = parent field name; inner = template field values. __Single round-trip: the autocreate fires before the parent submit, atomically from the client's perspective.__ |
| `captured_at` | ISO 8601 string \| null | optional | The instant the user tapped Submit on the phone. Used for offline-queue replays — the History entry, the file's `date` frontmatter, and any strftime tokens in `path`/`append_template` resolve against this time, __not__ the server's `now`. If null/absent, server uses `Local::now()`. |
| `client_id` | string | optional | Opaque label to disambiguate clients in logs / future telemetry. Free-form. |

__Response 201 Created:__

```json
{
  "vault_path": "Coffee/2026/20260425 18-30-42.md",
  "transport_mode": "API",
  "auto_created": [
    {
      "field": "bean",
      "value": "Ethiopia Guji",
      "vault_path": "Coffee/Beans/Ethiopia Guji.md",
      "templated": true
    }
  ],
  "post_create_commands": [
    { "field": "bean", "command": "templater:run", "fired": true }
  ],
  "history_id": "20260425T183042-coffee",
  "captured_at": "2026-04-25T14:33:02Z"
}
```

- `Location: /api/v1/history/<history_id>` header on 201.
- `auto_created[].templated` — whether `create_template` was used (sub-form path) vs bare-stub.
- `post_create_commands[].fired` — `true` only on API transport. Filesystem transport returns `fired: false` with no error (engine treats this as best-effort, mirroring TUI behavior).

__Error cases:__

| Status | Code | Trigger |
|--------|------|---------|
| 400 | `validation_failed` | Required field empty (and visible per `show_when`); invalid number format if engine rejects; unknown field name. |
| 404 | `not_found` | Unknown module key, OR a known module with `mobile_visible = false` (the server enforces visibility on submit too — clients cannot bypass the `/api/v1/config` filter by guessing module keys). |
| 409 | `idempotency_replay_in_flight` | Same `Idempotency-Key` is currently being processed (rare; race). Client should retry after a short delay. |
| 500 | `write_error` | Engine returned an error from `write_create`/`write_append`. `details.engine_error` carries the underlying message. |
| 502 | `transport_error` | API + filesystem both unreachable. |

*[Deviation: the submit handler never returns 502. Any engine failure, transport failures included, comes back as `500 write_error` with `details.engine_error`. Unknown field names are not rejected either: values for fields the module does not declare, or that `show_when` hides, are dropped before validation. A novel value on a `create_template` field with no matching `auto_create_inputs` entry returns `400 validation_failed` with `details.code = "auto_create_input_required"`.]*

__Synthetic 202 (PWA service worker only — round 6 amendment):__

The HTTP status `202 Accepted` is NOT produced by this server endpoint. It is a __client-side construct__ emitted exclusively by the PWA service worker to its own page when a submit is queued for offline replay. No server-side handler ever returns 202 from `/api/v1/submit/*`.

Clients written directly against this contract — including a future MCP companion (Phase 4) that bypasses the service worker entirely — will __never__ see 202. They receive 201 on success or 4xx/5xx on failure as documented above.

The synthetic 202 body from the service worker is: `{ "queued": true, "queue_id": <idb-id>, "captured_at": "<from-body>" }`. The page treats 202 as a "Queued" summary state (not "Saved"). This status code is documented here for completeness; it is not part of the server contract.

__Engine semantics preserved:__
- Hidden fields (`show_when` false) are NOT validated for `required` and are NOT written. Server runs `visible_field_indices` server-side as belt-and-suspenders even though the client should already have done so.
- Hidden modules (`mobile_visible = false`) reject submits with 404 `not_found`. The server is the source of truth on module visibility; the PWA-side filter is not authoritative. This mirrors the `/api/v1/config` filter and prevents clients from bypassing visibility by guessing module keys.
- Auto-create is best-effort. If it fails, the parent submit still proceeds; `auto_created[]` carries success records only, and a failure is reflected in `details.autocreate_warnings` of a separate `warnings` array (200/201 with warnings is preferred over 207 Multi-Status).

```json
{
  "vault_path": "...",
  "transport_mode": "FileSystem",
  "auto_created": [],
  "warnings": [
    { "code": "autocreate_failed", "field": "bean", "message": "..." }
  ],
  ...
}
```

*[Deviation: warning codes in use on a 201 are `autocreate_failed`, `history_record_failed`, `frontmatter_update_notice` (an `update` capture added a key the note was missing) and `post_write_shell_failed`.]*

### 6.5 `GET /api/v1/history` — AMENDED 2026-04-26

Recent capture log for the mobile dashboard (heatmap, last-pour, streak).

__Auth:__ required.

__Query params:__
- `since` — ISO 8601, inclusive lower bound. Optional.
- `until` — ISO 8601, exclusive upper bound. Optional. Direct timestamp filter ("older than X date"). Do NOT use `until` as a pagination cursor — use `cursor` instead.
- `cursor` — opaque string (the `next_cursor` from the previous response). When present, returns entries whose id is lexicographically less than the cursor. This is the correct way to paginate; `until` alone is incorrect for pagination because same-millisecond entries share a timestamp and would be silently dropped at the boundary.
- `limit` — integer, 1–1000. Default 100.
- `module` — module key. Optional. Filters to one module.

__Response 200:__

```json
{
  "entries": [
    {
      "id": "20260425T183042123-0-coffee",
      "module_key": "coffee",
      "timestamp": "2026-04-25T18:30:42Z",
      "vault_path": "Coffee/2026/20260425 18-30-42.md",
      "first_field": "Ethiopia Guji"
    }
  ],
  "summary": {
    "version": 1,
    "last_pour": { /* HistoryEntry */ },
    "today_count": 3,
    "week_count": 14,
    "streak_days": 7,
    "per_module_today": { "coffee": 2, "me": 1 }
  },
  "has_more": false,
  "next_cursor": null
}
```

- `summary` is included only when neither `since` nor `until` filters are present (i.e. the dashboard call). For windowed queries, it's omitted to keep the response small.
- `has_more` + `next_cursor` provide cursor pagination. `next_cursor` is the opaque `id` of the last returned entry. Pass `?cursor=<next_cursor>` to retrieve the next page.
- __Why opaque cursor instead of timestamp__: entry ids have the format `YYYYMMDDTHHmmSSsss-<counter>-<module>` and are lexicographically sortable = chronologically + counter sortable. The PWA's offline-queue replay can submit multiple entries at the exact same millisecond; a timestamp-only cursor would silently drop entries that share the cursor timestamp. The id-based cursor is exact.
- __Backwards compatibility__: the old `next_until` field is removed. Clients that relied on `next_until` must switch to `next_cursor` + `?cursor=`.

__Note:__ the trim feature (`pour trim`) does NOT have a mobile equivalent in Phase 1. Destructive operations stay on the desktop where the confirmation flow lives.

### 6.6 `GET /api/v1/captures/{history_id}` — LOCKED

Read back the rendered file content of a prior capture. Enables agent and PWA tasks like "summarize my last 5 coffee logs" without giving the client direct vault access.

__Auth:__ required.

__Path params:__
- `history_id` — the opaque `id` returned by `POST /api/v1/submit/{module}` and listed in `/api/v1/history`.

__Response 200:__

```json
{
  "id": "20260425T183042-coffee",
  "module_key": "coffee",
  "timestamp": "2026-04-25T18:30:42Z",
  "vault_path": "Coffee/2026/20260425 18-30-42.md",
  "content": "---\nicon: ☕\nbean: \"[[Ethiopia Guji]]\"\ndose_g: 18\n...\n---\n\nBright, blueberry, mid-body.\n",
  "transport_mode": "API"
}
```

- `content` is the full UTF-8 file content (frontmatter + body) as written.
- `vault_path` is echoed for context.
- `transport_mode` reflects which backend served the read at this moment (may differ from the write-time mode).

__Error cases:__

| Status | Code | Trigger |
|--------|------|---------|
| 404 | `not_found` | Unknown `history_id`, or the history entry exists but the underlying vault file has been deleted |
| 502 | `transport_error` | Both API and filesystem unreachable for the read |
| 500 | `read_error` | Underlying read failed for another reason; `details.engine_error` carries the message |

__Engine surface used:__ the existing `Transport::list_directory` does not cover single-file reads. Step C adds `Transport::read_file(vault_path) -> Result<String>` to `src/transport/mod.rs` (one-line wrapper over the filesystem read; API path uses `GET /vault/{path}`). This is the only engine addition Phase 1 introduces beyond what was already in `lib.rs`.

__Note:__ body size is bounded by whatever Pour just wrote. Vault content is trusted user data; no sanitization is performed before returning.

### 6.7 `GET /api/v1/presets/{module}` — LOCKED

__Response 200:__

```json
{
  "presets": [
    {
      "name": "Morning Onyx",
      "description": "default for weekday espresso",
      "values": { "bean": "Ethiopia Guji", "dose_g": "18", "method": "Espresso" }
    }
  ]
}
```

Order matches the on-disk order (which the TUI lets users reorder via Ctrl+Left/Right).

### 6.8 `PUT /api/v1/presets/{module}/{name}` — LOCKED

Upsert a single preset. Body is the preset entry (without `name`).

__Request body:__

```json
{
  "description": "default for weekday espresso",
  "values": { "bean": "Ethiopia Guji", "dose_g": "18", "method": "Espresso" }
}
```

__Response 200__ (upsert) or __201 Created__ (new):

```json
{ "preset": { "name": "Morning Onyx", "description": "...", "values": {...} } }
```

`name` URL-encoded in the path. Names containing `/` are rejected (404 not_found-style mismatch from axum's matcher).

__Reserved names:__ the literal name `"order"` (case-insensitive) is reserved and MUST NOT be used as a preset name. The route `/presets/{module}/order` (§6.10) is a fixed path segment registered before `/{name}` in the router; a preset named "order" would be permanently unreachable via single-preset endpoints. Attempting `PUT /presets/{module}/order` with a preset body (wrong DTO for §6.10) returns `400 validation_failed`. Attempting the same via percent-encoding (e.g. `ord%65r`) reaches `put_handler` and is rejected with `400 validation_failed { code: "reserved_name" }`. Clients MUST reject the name "order" (case-insensitive) in name-input validation before sending the request. *(Amendment round 8 — 2026-04-27)*

### 6.9 `DELETE /api/v1/presets/{module}/{name}` — LOCKED

__Response 204__ on success, __404__ if not found.

### 6.10 `PUT /api/v1/presets/{module}/order` — LOCKED

Reorder presets within a module.

__Request body:__

```json
{ "names": ["Morning Onyx", "Aeropress quick", "Decaf evening"] }
```

The full list of preset names must be supplied. Missing or extra names → 400 `validation_failed` with `details.missing` / `details.extra`. Duplicate names in the request → 400 `validation_failed` with `details.duplicates`. The TUI's reorder semantics are preserved.

__Response 200__ with the resulting `{ presets: […] }`.

## 7. Field Schema Marshalling — LOCKED

### 7.1 Field Types

The `field_type` enum on the wire matches the Rust `FieldType` enum (snake_case):

```
text | textarea | number | static_select | dynamic_select | composite_array
```

*[Deviation: v1.1.0 added `toggle` and `counter`. See §6.2.]*

### 7.2 Field Config Object (In `/api/v1/config`)

```json
{
  "name": "bean",
  "field_type": "dynamic_select",
  "prompt": "Bean",
  "required": true,
  "default": null,
  "options": null,
  "source": "Coffee/Beans",
  "target": null,
  "callout": null,
  "callout_title": null,
  "allow_create": true,
  "wikilink": true,
  "create_template": "bean",
  "post_create_command": "templater:run",
  "show_when": null,
  "icon": "🫘",
  "preset_exclude": false,
  "list": false,
  "sub_fields": null
}
```

All optional fields are present and explicitly `null` (not omitted). This keeps client decoders simple.

### 7.3 `show_when` Shape

```json
{ "field": "method", "equals": "Espresso" }
```

Or:

```json
{ "field": "method", "one_of": ["Espresso", "Moka"] }
```

Client-side evaluation (see §8). Server still validates `show_when` rules at config load (existing validator).

### 7.4 Template Field Config

```json
{
  "name": "roaster",
  "field_type": "text",
  "prompt": "Roaster",
  "options": null,
  "default": null,
  "allow_create": false
}
```

`field_type` restricted to `text | number | static_select`.

## 8. `show_when` Evaluation — LOCKED (Per Plan)

__Client-side evaluation.__ The server ships rules verbatim in `/api/v1/config`; the PWA evaluates them on every field-value change. Rationale:

- Avoids a round-trip per keystroke that touches a controlling field
- Logic is trivial (~20 lines of JS) — `equals` and `one_of` only, case-sensitive
- Server still re-evaluates server-side at submit as a safety net (engine's `partition_fields` already calls `visible_field_indices`)

The PWA's `visibility.js` mirrors `src/visibility.rs` semantics:
- Hidden fields: skipped in render, navigation, and validation
- A field whose controlling field is empty/absent is hidden
- Self-reference / circular chains: handled at config-load time on the server; the client trusts the rules it receives

## 9. Idempotency — LOCKED

`POST /api/v1/submit/{module}` accepts an optional `Idempotency-Key` header.

- __Format__: opaque string, 1–256 ASCII printable characters. Recommended: a UUIDv4 generated client-side.
- __Server behavior__: on receipt, the server records `(key, response)` in an in-memory LRU (capacity ~1024, TTL 5 minutes). If the same key is replayed within the window:
  - If the prior request returned a __cacheable response__: replay the same status + body byte-for-byte. The replayed response carries an `Idempotency-Replay: true` header.
  - If the prior request is still in flight: return `409 idempotency_replay_in_flight`.
- __Cacheable responses__: only __2xx terminal successes__ (typically 201) are stored in the cache. __4xx and 5xx responses are NOT cached__ — these represent recoverable conditions (validation_failed: fix the field and retry; transport_error / write_error: retry when the underlying issue clears). Caching errors would block the user from recovering within the form session. The client SHOULD reuse the same `Idempotency-Key` for retries after a recoverable error so a duplicate-write race never opens.
- __Different body, same key (cached 2xx)__: returns the original cached response (NOT a fresh execution). The client is responsible for rotating keys per submit attempt after success; reusing a key for a different payload after a 2xx is a client bug.
- __Different body, same key (no cached response)__: each retry executes fresh. The server cannot detect "different body, same key" before execution; that's a client-discipline issue.
- __Storage__: in-memory only. Restarting `pour serve` clears the cache. Acceptable: the offline queue's retry windows are seconds-to-minutes, not hours.

The PWA persists `Idempotency-Key` across retries within a form session, rotates it after a successful 2xx, and rotates again on form reset. Service-worker retries reuse the same key.

## 10. `captured_at` Timing Semantics — LOCKED (Critical for Offline)

When `captured_at` is present in a submit body:

1. The History entry's `timestamp` field is set to `captured_at`, NOT the server's receipt time.
2. The output file's `date` frontmatter (and any `{{date}}` / strftime tokens in `path` / `append_template`) is rendered using `captured_at` in the server's local timezone.
3. The autocreated note's `date` frontmatter (when `auto_create_inputs` is present) is also rendered against `captured_at`.

When absent or `null`: server uses `chrono::Local::now()` exactly as the TUI does today.

__Validation__: `captured_at` must be within the past 30 days and not more than 5 minutes in the future. Outside this range → 400 `validation_failed` with `code: captured_at_out_of_range`. The 5-minute future allowance accommodates clock skew.

__Why this matters__: a festival capture queued at 11pm on Friday and synced at 9am Monday would otherwise be dated Monday. That breaks the entire premise of offline capture.

## 11. CORS — LOCKED

Same-origin only.

- The PWA is served from the same `pour serve` instance. Origin matches.
- The server does NOT emit `Access-Control-Allow-Origin` headers.
- Pre-flight `OPTIONS` requests on `/api/*` return `405 Method Not Allowed` (we do not support cross-origin).
- This intentionally blocks browser-based exfiltration from other origins even if a token leaks into a malicious page's reach.

## 12. Cache Headers — LOCKED

| Resource | `Cache-Control` |
|----------|-----------------|
| `/api/*` (all JSON) | `no-store` |
| `/` (PWA shell HTML) | `no-cache, max-age=0, must-revalidate` (always revalidate; service worker handles offline) |
| `/static/*` (JS, CSS, icons) | `public, max-age=300, must-revalidate` (5 min) — content-hashed filenames could allow `immutable` in a future iteration |
| `/manifest.json`, `/sw.js` | `no-cache, max-age=0, must-revalidate` |

`ETag` headers MAY be set by axum's static-file layer; the contract does not require them.

## 13. Body Size Limits — LOCKED

- `POST /api/v1/submit/*` — 1 MiB max. `composite_array` data is the primary contributor; 1 MiB allows several thousand rows of typical fields.
- `PUT /api/v1/presets/*` — 256 KiB max.
- All other endpoints: 16 KiB max.

Exceeding the limit → `413 payload_too_large`.

## 14. Logging & Telemetry — LOCKED

- Tokens MUST NOT appear in logs. The auth middleware logs only the auth outcome (`accepted_via_header`, `accepted_via_query`, `rejected`), never the token itself.
- Request bodies for `POST /api/v1/submit/*` MUST NOT be logged at info level — they contain user content. Debug level is acceptable for local dev only.
- Errors include the `code` slug in logs but not user input that triggered them (to avoid PII spillage in case of crash dumps).
- No external telemetry. Pour is local-first; no metrics leave the machine.

## 15. Future / Out of Scope

The contract leaves room for, but does not yet define:

- __`/api/v1/events` (Server-Sent Events)__ — live updates so an open browser session sees the TUI's writes in real time. Phase 3.
- __`/api/v1/trim`__ — destructive history trim. Stays desktop-only for now (the TUI's "type 'trim' to confirm" gate is manifesto-aligned).
- __Multi-tenant__: not supported. One config, one vault, one token per server.
- __TLS__: deferred to Phase 3 with a setup guide.
- __Service-side render of `show_when`__ — if client-side proves problematic in practice, an opt-in `?resolve_visibility=true` query on `/api/v1/config` could ship a pre-filtered field list. Not built unless needed.
- __`pour mcp` companion__ (Phase 4) — a Model Context Protocol server that wraps this API and exposes `pour_submit_<module>` tools dynamically derived from `/api/v1/config`. Same binary or sibling crate; reuses the engine. Documented intent only — no implementation slot in Phases 1–3. The HTTP API contract is designed to be MCP-friendly: schema discovery, valid-choices endpoints, idempotent submits, time-preserving captures, read-back via `/api/v1/captures/{id}`.

## 15.1 OpenAPI 3.1 Spec — LOCKED

A machine-readable OpenAPI 3.1 document accompanies this human-readable contract:

- __Phase 1__: hand-written at `pour - docs/02 references/pour-openapi.yaml`. The inspector audits every Step B–G endpoint against both this contract (markdown) and the OpenAPI spec (YAML); any divergence is a fix-before-merge. Optionally served at `/api/v1/openapi.json` so the PWA, agents, and `curl`+`jq` users can pull it at runtime.
- __Phase 2__: replaced by `utoipa`-generated output. Handlers gain `#[utoipa::path(…)]` annotations; request/response types gain `#[derive(ToSchema)]`. The hand-written YAML is deleted in the same PR; OpenAPI is now derived from the Rust types deterministically. The markdown contract (this file) stays as the human narrative — synced via inspector audit, not via codegen.

The "single source of truth" hierarchy: __Rust types__ (Phase 2 onward) → __OpenAPI__ (machine spec) → __this markdown__ (human narrative). All three must agree; the inspector is the watchdog.

## 16. Ratification Log

All open questions raised during contract drafting were ratified on 2026-04-25:

| # | Decision | Locked answer |
|---|---|---|
| 1 | API namespace | `/api/v1/…` from day one |
| 2 | Wire types for field values | All strings, including numbers (matches engine's internal `HashMap<String, String>` — no JSON number ambiguity) |
| 3 | `Idempotency-Key` support | Phase 1, in-memory LRU, 5-min TTL |
| 4 | `captured_at` window | 30 days past, 5 minutes future |
| 5 | `mobile_visible` per-module key | Lands in Step B alongside `/api/v1/config`. Bumps `config_version` from `0.2.0` → `0.3.0`. Default `true`; `false` omits the module from `/api/v1/config` entirely |
| 6 | Presets HTTP shape | Separated CRUD: `GET /presets/{module}`, `PUT /presets/{module}/{name}` upsert, `DELETE /presets/{module}/{name}`, `PUT /presets/{module}/order` reorder |
| 7 | History `summary` payload | Included on the dashboard call (no `since`/`until`); omitted on windowed queries |
| 8 | `capabilities` array in `/health` | Carried from day one |
| 9 | Read-back via `GET /api/v1/captures/{history_id}` | Lands in Phase 1 (Step C alongside `/api/v1/submit`). Adds `Transport::read_file` as the only new engine method. Enables agent and PWA "summarize my last N" workflows |
| 10 | OpenAPI 3.1 spec | Hand-written in Phase 1 at `pour - docs/02 references/pour-openapi.yaml`; replaced in Phase 2 by `utoipa`-derived output. Markdown contract (this file) remains the human narrative |
| 11 | MCP companion in roadmap | Documented as Phase 4 intent in §15. No build commitment now. The HTTP API contract is intentionally MCP-friendly so the wrapper is thin when built |

All future deviations require an amendment to this contract committed before the implementation lands.

## 17. Change Log

- __2026-04-25__ — initial draft.
- __2026-04-25__ — ratified (round 1). Eight open questions resolved per §16 rows 1–8. Status: locked.
- __2026-04-25__ — agent-surface amendment (round 2). Added `GET /api/v1/captures/{history_id}` as §6.6, OpenAPI 3.1 Phase 1 hand-written + Phase 2 `utoipa` (§15.1), MCP companion roadmap (§15). Three new ratified decisions per §16 rows 9–11.
- __2026-04-26__ — pagination cursor amendment (round 3). §6.5: `next_cursor: string|null` replaces `next_until: timestamp|null` for pagination; `cursor` query param added; `until` remains as a direct timestamp filter but must not be used as a pagination cursor. Rationale: same-millisecond entries (possible during PWA offline-queue replay) share a timestamp and would be silently dropped by a timestamp-only cursor; id-based cursor is exact. §6.10: `details.duplicates` added to reorder 400 response when the request list contains duplicate names.
- __2026-04-26__ — submit-side mobile_visible amendment (round 4). §6.4: `POST /api/v1/submit/{module}` now explicitly returns 404 `not_found` for modules with `mobile_visible = false`, mirroring the `/api/v1/config` filter. Previously the contract only documented config-side filtering, leaving submit behavior implicit. Ratifies what the implementation already does: server is the source of truth on module visibility; clients cannot bypass the filter by guessing module keys.
- __2026-04-26__ — idempotency cacheability amendment (round 5). §9: only __2xx terminal successes__ are stored in the idempotency cache. __4xx and 5xx responses are NOT cached.__ Earlier wording said "final response" without disambiguating, which led to an implementation that cached every status — meaning a `400 validation_failed` would replay for 5 minutes, blocking users from fixing a field and retrying within the form session. The amendment makes recoverable errors retryable while preserving the duplicate-write protection that idempotency exists to provide.
- __2026-04-27__ — synthetic-202 clarification (round 6). §6.4: added a note that `202 Accepted` is a PWA service-worker–only construct emitted to the page when a submit is queued offline. The server __never__ returns 202 from `/api/v1/submit/*`. Clients bypassing the service worker (future MCP companion, direct `curl`) see 201 on success or 4xx/5xx on failure only. No server-side change; documentation only.
- __2026-04-27__ — reserved name "order" (round 8). §6.8: the name `"order"` (case-insensitive) is reserved and must not be used as a preset name. The `/presets/{module}/order` fixed segment (§6.10) is registered before `/{name}` in the router, making a preset literally named "order" permanently unreachable via single-preset endpoints. Server rejects the name in `put_handler` with `400 validation_failed { code: "reserved_name" }` as a belt-and-suspenders guard. Clients must also reject "order" (case-insensitive) in name-input validation before sending any request. Three regression tests added to `tests/server_presets.rs`.
- __2026-10-02__ — docs sync, no amendment. Deviations from the shipped server are annotated inline in §6.1, §6.2, §6.4 and §7.1. Most come from v1.1.0 (`update` mode, `toggle`/`counter`, `post_write_shell`, root overrides), which changed the server without a round entry here. The per-field codes in §5.2 were already updated at that time.
