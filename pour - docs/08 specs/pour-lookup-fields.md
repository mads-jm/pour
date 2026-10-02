---
tags: [spec, fields, lookup, computed]
aliases:
  - lookup fields
  - computed fields
  - field lookup
date created: Wednesday, April 29th 2026, 5:03:15 pm
status: draft — not yet scheduled
date modified: Wednesday, April 29th 2026, 5:31:45 pm
---

# Pour Lookup / Computed Fields

> A new `field_type = "lookup"` that resolves its value by reading frontmatter from a sibling field's linked vault note, optionally applying a transform. The resolved value is written into the capturing note's frontmatter at submit time — statically, not at view time. Concrete first use case: `days_off_roast` derived from the bean note's `roast_date`.

## 1. Motivation

Today every Pour field is either user-typed or has a static `default`. There is no way to derive a field's value from another note in the vault. This forces duplication: a coffee bean's `roast_date` is captured once on the bean note when it's first poured into the vault, but `days_off_roast` is then either (a) re-entered manually on every brew (data the user could compute), (b) skipped, or (c) deferred to Dataview at view time — which leaves the brew note's frontmatter empty and breaks cross-vault frontmatter queries.

Lookup fields close this gap:
- __Single source of truth__: the bean note's `roast_date` is the canonical value.
- __DRY at capture time__: the user picks the bean; `days_off_roast` resolves automatically.
- __Statically queryable__: the resolved value lives in the brew note's frontmatter, not as a Dataview computation.
- __Composable__: the same mechanism works for any "this field's value depends on the linked note" relationship — origin from bean, brewer category from brewer, etc.

## 2. Concept

A `lookup` field declares:
1. __`lookup_via`__ — the name of a sibling field on the same form that holds a wikilink (typically a `dynamic_select` with `wikilink = true`).
2. __`lookup_key`__ — the frontmatter key on the linked note to read.
3. __`compute`__ (optional) — a named transform applied to the looked-up value before it's written. Default: `passthrough`.
4. __`fallback`__ (optional) — what to do if resolution fails (linked note missing, key missing, transform fails). Default: `manual`.

At submit time, the resolver:
1. Reads `lookup_via`'s value (a wikilink target like `Onyx Monarch`).
2. Resolves it to a vault path under the relevant `source` directory (or the absolute wikilink target).
3. Parses that note's YAML frontmatter.
4. Reads `lookup_key` from the parsed frontmatter.
5. Applies the named `compute` transform.
6. Writes the result into THIS field's value in the capturing note's frontmatter.

## 3. Schema

```toml
[[modules.coffee.fields]]
name = "days_off_roast"
field_type = "lookup"
prompt = "Days off roast"
target = "frontmatter"
lookup_via = "bean"
lookup_key = "roast_date"
compute = "days_since"
fallback = "manual"          # manual | skip | default | error
default = ""                  # used only when fallback = "default"
```

`prompt` still applies — see UX (§5) for how it's surfaced.

`target` MUST be `frontmatter`. Body targeting for lookup fields is out of scope (the use case is structured derived data).

## 4. Resolution Semantics

### 4.1 Trigger Points

- __At form-open__ (TUI / PWA): if `lookup_via` already has a value (from a preset apply, an edit-resubmit prefill, or a default), resolve eagerly and surface the result to the user.
- __At `lookup_via` change__: re-resolve when the user picks a different value for the source field. The user sees the resolved value update.
- __At submit__: a final resolution pass runs server-side as the authoritative result; whatever the client showed is replaced with the server-computed value to defeat client-side staleness.

### 4.2 Resolution Path

1. `lookup_via_value = field_values[lookup_via]`. If empty → handle per `fallback`.
2. Strip wikilink syntax: `[[Onyx Monarch]]` → `Onyx Monarch`; `[[Onyx Monarch|My Onyx]]` → `Onyx Monarch`.
3. Resolve to a vault path: prefer the source folder declared on the `lookup_via` field's config (e.g. `02 - Areas/204 - Cooking/Coffee/Beans`). Append `.md`. If that file doesn't exist, fall through to a vault-wide search (the same resolution Obsidian does). If still not found → `fallback`.
4. Parse the note's frontmatter. If frontmatter is malformed or missing → `fallback`.
5. Read `frontmatter[lookup_key]`. If absent or empty → `fallback`.
6. Apply `compute` (§5). If the transform fails (e.g., unparseable date) → `fallback`.
7. Write the result.

### 4.3 Fallback Modes

- __`manual`__ (default): the field becomes a regular text/number input with the prompt; user types the value. Submit succeeds.
- __`skip`__: the field is omitted from the capturing note's frontmatter entirely. Submit succeeds.
- __`default`__: write the field's `default` value. Submit succeeds.
- __`error`__: submit fails with a `validation_failed` error. Use for required derivations.

### 4.4 Cycle Detection

Lookup fields can reference other lookup fields (`A` looks up `bean.X`, then `B` looks up `A`'s value as a key). Limit the resolution depth to __3 hops__. Beyond that, fall back. Cycle (A → B → A) detected by tracking visited field names; aborts with `fallback`.

### 4.5 Determinism

Every field is resolved AT MOST ONCE per submit. The resolver builds a topologically-ordered list of lookup fields based on `lookup_via` references and resolves them in order. If there's a cycle, all involved fields fall back.

## 5. Compute Transforms

Initial set (v1):

| Name | Input | Output | Notes |
|---|---|---|---|
| `passthrough` | any | same value as string | Default. The looked-up value is written as-is. |
| `days_since` | date string (`YYYY-MM-DD`) | integer | `(today_local - parsed_date).days`. Negative if date is in the future. |
| `parse_int` | string | integer | Parses the value as a base-10 int; fails the resolution if non-integer. |
| `to_lower` | string | string | Lowercases. |

Future v2: `multiply`, `concat`, `template`, `format_date`, etc. Add them as concrete needs surface — don't pre-design.

The compute namespace is closed: the user cannot define their own transforms in config. (Alternative: a Lua/Rhai sandbox. Out of scope — punt to a future "scriptable transform" feature if demand exists.)

## 6. UX

### 6.1 TUI

- Lookup field renders as a regular field row with a __🔗__ icon next to the prompt and the resolved value shown in the value column.
- If `lookup_via` is empty, the value column shows `<awaiting <lookup_via>>` in dim italic.
- If resolution fails and `fallback = "manual"`, the row becomes editable text/number; the prompt stays. The 🔗 icon dims to indicate the lookup didn't resolve.
- The user can override the resolved value by pressing Enter on the row and typing — same flow as overriding any auto-suggested value. The override persists for that capture only; doesn't write back to the linked note.

### 6.2 PWA

- Same icon treatment.
- Resolved value renders in a disabled-looking input with a small "auto" pill.
- Tap the input to override (enables editing). Override persists for the submit only.
- Empty `lookup_via` → input is disabled with placeholder `awaiting <lookup_via>`.

### 6.3 Override Semantics

- An overridden lookup value is sent in the submit body like a regular field.
- Server resolver detects override (the submitted value differs from what server-side resolution would produce) and respects the override — writes the user's value, not the resolved one.
- Logged at debug level (no user content per §14): `lookup field "days_off_roast" overridden`.

## 7. API Contract Impact

Round-N amendment to `pour-api-contract.md`:

### 7.1 New field_type

-7 (field types) gains `lookup` with the new keys (`lookup_via`, `lookup_key`, `compute`, `fallback`).

### 7.2 Submit Body

No body shape change. The lookup field's resolved value rides in `field_values[<lookup_field_name>]` like any other frontmatter field. Whether the value is server-resolved or user-overridden is transparent on the wire.

### 7.3 Server Resolver Endpoint (optional, for Live previews)

For PWA live preview without round-trip-on-every-keystroke, expose:

```
GET /api/v1/lookup/{module}/{field}?lookup_via_value=<urlencoded>
→ 200 { resolved_value: string, fallback_applied: bool }
→ 404 { error: { code: "lookup_target_missing" } }
→ 422 { error: { code: "lookup_compute_failed" } }
```

Optional — the PWA can also resolve client-side by fetching the linked note's frontmatter via existing `/api/v1/captures` paths, but that conflates two endpoints. Add the dedicated endpoint when PWA support lands.

### 7.4 OpenAPI

Add the new field-type discriminator and the optional endpoint.

## 8. Edge Cases

1. __Wikilink with display alias__ (`[[Onyx|My Bean]]`) — strip alias, resolve target.
2. __Wikilink to a heading or block__ (`[[Onyx#Roast]]`) — strip the fragment, resolve to file. The frontmatter key is at the file level.
3. __Bean note exists but has no `roast_date`__ — `fallback`.
4. __Bean note has `roast_date: 2025-13-99`__ (malformed) — `compute = "days_since"` fails parse → `fallback`.
5. __`lookup_via` field is `dynamic_select` with `allow_create = true` and the user types a novel value__ — the bean note doesn't exist yet (will be created via `auto_create_inputs`). The lookup MUST run AFTER autocreate so the new bean note's frontmatter is readable. Submit-time resolution order: autocreate first, then lookups.
6. __`lookup_via` field is empty/required-but-missing__ — fail with the existing `validation_failed` for the required field; don't separately fail the lookup. The lookup field falls back to `manual`/`skip`/etc.
7. __Note path resolution ambiguity__ — if two notes share the same name in different folders, prefer the one under the source folder declared on `lookup_via`'s field config. If still ambiguous, take the first match and log a warning.
8. __Bean note frontmatter encoded with non-UTF-8__ — fail per §15 (server doesn't accept non-UTF-8); falls back.
9. __Lookup of a lookup__ (depth ≥ 1) — supported up to 3 hops. Cycle detection drops to fallback.
10. __Caching__: bean note frontmatter is read on every submit. Cheap (<1KB note, single fs read). No cache in v1; revisit if a single submit triggers >5 lookups.

## 9. Phasing

### L1 — Foundation (target: 1–2 weeks)

- Schema: `field_type = "lookup"` with `lookup_via`, `lookup_key`, `compute`, `fallback`, `default`.
- Resolver in `src/data/lookup.rs` (new module): `resolve(field, form_state, vault) -> Result<String, FallbackReason>`.
- Compute transforms: `passthrough`, `days_since`. (Skip `parse_int`/`to_lower` for L1; add when needed.)
- Server-side resolution at submit: `submit.rs` runs the resolver after `auto_create_inputs` and before the final write.
- TUI render: 🔗 icon, resolved value display, override flow.
- Tests: `tests/lookup_resolver.rs` (resolver unit tests, fallback modes, cycle detection), extend `tests/server_submit.rs` for the submit-time pipeline.
- Docs: `field-types.md` adds lookup; this spec moves from `draft` to `shipped`.
- __PWA__: NOT in L1. PWA continues to render the field as text/number per the fallback type until L2.

### L2 — PWA Support

- PWA renders the 🔗 icon and the resolved-value pill.
- New endpoint `GET /api/v1/lookup/{module}/{field}` for client-side live preview.
- Override UX in PWA.
- Contract amendment landed FIRST per `feedback_contract_first.md`.

### L3 — More Transforms + Caching

- Add `parse_int`, `to_lower`, `format_date`, `concat`, `template` as concrete needs surface.
- Lookup result cache: per-submit memoization (already implicit in the resolver) plus per-form-session cache so re-resolves on `lookup_via` change don't re-read the same note.

## 10. Out of Scope (forever)

- __Editing the linked note from Pour__ — Pour writes, doesn't edit. Bean note's `roast_date` is set when the user creates the bean; updating requires opening the bean note in Obsidian.
- __Cross-vault lookups__ — single-vault model per design spec §1.
- __Live reactive queries__ — Dataview is the right tool. Pour's lookups are a one-time resolution at submit, not a live binding.
- __Scriptable transforms__ — closed compute namespace. If this becomes painful, scope a separate "scripted transform" feature with a sandbox.
- __Lookup target caching across submits__ — bean notes can be edited externally; assuming staleness defeats the point of single-source-of-truth. Read fresh every submit.

## 11. Open Questions

1. __Override-detection semantics__: server-side, how does the resolver know the submitted value is an override vs the resolved value? Option A: resolve fresh; if submitted ≠ resolved, treat as override. Option B: client signals override via a `_lookup_override` map in the body (similar to `auto_create_inputs`). Option B is more explicit; Option A is simpler. Pick A for L1; revisit if it causes confusion.
2. __Date timezone__: `days_since` uses `today_local`. Whose local? The server's, where the submit handler runs. Document this — it could surprise a user submitting from a phone in a different timezone via `pour serve`.
3. __`lookup_via` referencing a `composite_array` field__: composite values are arrays of maps, not single wikilinks. Reject in `validate_axes`-style validation? Or take the first element's first wikilink? Reject for L1; revisit if needed.
4. __Empty looked-up value vs missing key__: TOML/YAML conflate `key: ""` and key-absent in some serializers. Decide: empty string → `fallback`, OR empty string is a valid "looked up" value. For `days_since` empty is unparseable → fallback regardless. For `passthrough` empty might be intended. Default: treat empty as fallback; document.
5. __Multiple wikilinks in one field value__ — `lookup_via` field value contains `[[A]] [[B]]` (rare but possible in `text` field). Resolve A only? Concatenate? Reject in L1.
6. __Logging__: §14 forbids logging user content. Lookup resolution involves reading frontmatter values that could be user-supplied. Resolver MUST NOT log resolved values. Log only field name + outcome (resolved/fallback/cycle/missing).
7. __Field ordering__: lookup fields should render AFTER their `lookup_via` field in the form so the user sees the source first. Validate at config-load that all `lookup_via` references point to fields earlier in the field list. (Otherwise the resolver still works server-side but the TUI/PWA UX is awkward.)

## 12. Concrete First Use case (the Trigger for This spec)

```toml
# In templates.bean — already added 2026-04-29
[[templates.bean.fields]]
name = "roast_date"
field_type = "text"
prompt = "Roast date (YYYY-MM-DD)"

# In modules.coffee — replaces the current manual `days_off_roast` field once L1 ships
[[modules.coffee.fields]]
name = "days_off_roast"
field_type = "lookup"
prompt = "Days off roast"
target = "frontmatter"
lookup_via = "bean"
lookup_key = "roast_date"
compute = "days_since"
fallback = "manual"
```

When L1 ships, the manual `days_off_roast` field in `resources/mads_config.toml` becomes the lookup version above. Until then, the manual field stays as-is — users still benefit from the bean's `roast_date` for retrospective Dataview queries.

## 13. Cross-references

- `pour - docs/08 specs/pour-design-spec.md` §7 — architecture intent.
- `pour - docs/02 references/field-types.md` — field type reference (will gain `lookup` when L1 ships).
- `pour - docs/08 specs/pour-api-contract.md` §7 (field types), §15 (intentional omissions — this leaves §15).
- `pour - docs/08 specs/pour-pwa-roadmap.md` — Phase 4+ candidate for PWA support (L2 above).
- [[pour-review-priors]] — shares this spec's frontmatter-read, wikilink-resolution, and trigger-model plumbing; the Priors panel should slot *behind* lookup-fields on the roadmap.

## 14. Change Log

- __2026-04-29__ — Initial draft. Concrete trigger: `days_off_roast` derived from bean note's `roast_date`. Spec covers schema, resolution, transforms, UX, API impact, phasing, and edge cases. Status: not yet scheduled — requires roadmap allocation.
