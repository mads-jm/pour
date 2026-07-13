---
tags: [spec, review, priors, query, tui]
aliases:
  - priors spec
  - review priors
  - pour review
  - review panel spec
date created: Monday, July 13th 2026, 2:05:00 pm
status: shipped — L1 (coffee, TUI)
date modified: Monday, July 13th 2026, 2:05:00 pm
---

# Pour Review / Priors Panel

> An inline, config-declared review surface: at capture time, Pour reads back the most-relevant prior captures for the module being logged — matched, ranked, and summarized entirely from `config.toml` — and renders them beside the form. The user never writes a query. Concrete first use case: dialing in an espresso by seeing your best `⭐4+` shots for this roaster + brew method. See the story: [[priors_at_the_pour]].

## 1. Motivation

Pour writes rich structured frontmatter but offers **no read-back at the point of decision**. To answer "what did I do last time for this roaster?" the user must leave the terminal, open Obsidian, and build or find a Dataview/Bases view. That context switch is exactly the friction the [[the_pour_manifesto|manifesto]] exists to kill — and it happens at the worst moment, mid-capture, kettle on.

The manifesto's *Capture First, Synthesize Later* cedes **reflection** to Obsidian. This feature is not reflection; it is **decision support in service of the very next capture** — which is capture-first by definition. Bases owns the Sunday table; it cannot live in the terminal capture flow. That gap is the feature.

The value is **not** parsing and presenting frontmatter (Bases already does that — commodity). The value is **curation + placement + timing**: *your best* priors, *beside the field you just filled*, *without leaving the terminal*, *with no query to author*.

## 2. Concept

A per-module `[modules.<key>.priors]` block declares:

1. **`match_on`** — an ordered list of keys defining "similar". Widen by dropping the most-specific (tail) key when a tier yields no matches (the **new-bag cascade**).
2. **`rank_by`** — how to order matches; also defines what "best" means for this domain.
3. **`show`** — which frontmatter fields render as columns, and which get summarized.
4. **`limit`** — max rows (default 5).

At form-open and on every `match_on`-field change, the resolver:

1. Collects the module's prior captures (via `/search/` or FS scan — §7).
2. Filters by `match_on`, cascading down tiers until a tier matches (§4).
3. Ranks the surviving set per `rank_by` (§5).
4. Computes the per-field-type summary line (§6).
5. Renders the panel with a header naming the matched tier + rank.

The panel is **read-only**. It never writes, never blocks submit, and never fires at submit time.

## 3. Schema

```toml
[modules.coffee.priors]
match_on = ["bean", "roaster", "method"]   # ordered: most → least specific
rank_by  = "rating desc"                     # best shots first
show     = ["dose_g", "yield_g", "time_s"]   # columns + numeric summary
limit    = 5

# richer match modes use object form (§4.2); bare string = equality
[modules.me.priors]
match_on = [
  { field = "tags", mode = "overlap" },      # #tag overlap in body
  { field = "date", mode = "window", days = 90 },
]
rank_by = "recent"
```

Every key is optional. **Zero-config default** (no `[priors]` block at all): match on the module's first `wikilink`/`select` field if one exists, else no match (recent-N of the same module); `rank_by = "recent"`; `show` = the module's numeric + select fields capped at 4 columns; `limit = 5`. This makes a useful panel appear for `me`/`note` with no setup, while `coffee` opts into richness.

### 3.1 `rank_by` grammar

| Form | Meaning | Example |
|---|---|---|
| `"<field> desc"` / `"<field> asc"` | Sort by a field | `rating desc` — best shots |
| `"<field> max"` / `"<field> min"` | Single extreme (PR-style) | `weight_g max` — heaviest set |
| `"recent"` | Newest capture first | journals |
| `"none"` | Preserve scan order, unranked | — |

`"best" is domain-defined by rank_by` — the primitive has no built-in notion of quality.

## 4. Match Semantics

### 4.1 The cascade (widen-by-drop-tail)

`match_on` is ordered most-specific → least. Resolve the full conjunction; if it yields zero rows, **drop the last key** and retry, until either a tier matches or the list is empty (→ empty-state message). The panel header always names the tier that matched (`Onyx · V60` vs `any roaster · V60`).

Rationale: the canonical trigger is a **new bag** — the most-specific key (`bean`) has no history precisely when the user most wants the panel. Widening is not a fallback bolt-on; it is the core behavior.

### 4.2 Match modes

| Mode | Config | Semantics | Phase |
|---|---|---|---|
| `equality` | bare string `"roaster"` | exact frontmatter value match | **L1** |
| `wikilink` | bare string on a `wikilink` field | strip `[[ ]]`/alias/fragment, compare targets | **L1** (special case of equality) |
| `overlap` | `{field, mode="overlap"}` | non-empty intersection of list/tag values | L2 |
| `window` | `{field, mode="window", days=N}` | capture within N days of today | L2 |

Bare string ⇒ `equality` (or `wikilink` if the field is declared `wikilink = true`). Object form ⇒ explicit mode. Simple stays simple; complexity is pay-as-you-go.

### 4.3 Text-only modules (`me`/`note`)

No wikilink/select/number to match on. Zero-config behavior: **recent-N of the same module, unfiltered**, PLUS `#tag`-in-body overlap when the body contains inline tags (parse `#\w+`, match on intersection). This gives journals a "recent context" rail without demanding schema they don't have.

## 5. Ranking

Rows split into **qualifying** (have the `rank_by` field) and **texture** (missing it):

1. **Qualifying rows first**, ordered by `rank_by` (`"<field> desc/asc/max/min"`). These are the "your best" rows.
2. **Texture rows fill to `limit`** below the qualifying rows, ordered by recency, rendered **dimmed**. A recent-but-unrated brew stays visible as context rather than vanishing.
3. `rank_by = "recent"` / `"none"` — every row qualifies; no texture split.
4. **Header qualifier** — the rank label (`your ⭐4+`) is shown **only when every displayed row qualifies**. If texture rows are mixed in, the header drops the qualifier and shows the tier alone (`Onyx · V60`), so the panel never claims "best" over rows that weren't ranked.
5. **All-texture tier** (nothing qualifies, e.g. no brew in this tier is rated) — degenerates to a pure recency list; header shows the tier with no rank qualifier.

The **summary line (§6) is computed from the qualifying rows only** — the `repeat:` target reflects what *worked*, not the unrated texture. If there are no qualifying rows, the summary is computed over the displayed (recency) set, or omitted per §6.

## 6. Summary Line (per-field-type)

One line under the rows, computed over the **qualifying rows** (§5). Each shown field summarizes per its type, with an optional per-field override:

| Field type | Default agg | Override options |
|---|---|---|
| `number` | `median` (outlier-robust) → `repeat: 15g · 1:16 · 2:55` | `mean` · `max` · `min` · `latest` |
| `static_select` / `dynamic_select` / tags | `mode` (most common) → `most-seen venue: The Fillmore` | `latest` |
| `text` / `textarea` | omitted | — |

**Median is the default** — a single 40s pull won't drag the target on a small, noisy set. `mean` is available where central tendency over *every* value is what you want (stable process vars — water temp; a `lift` module's working-set average). Configured with the **same bare-string-vs-object pattern as `match_on`** (§4.2):

```toml
show = [
  "dose_g",                              # median (default)
  "time_s",                              # median
  { field = "water_temp_c", agg = "mean" },
]
```

If no shown field is summarizable → replace the line with a match-count (`3 of 12 brews · matched roaster+method`). Ratios (`1:16`) are a coffee-specific *render* of two numeric fields; the primitive summarizes each number independently — ratio formatting is a display concern (§8.3), not a summary type.

## 7. Data Path (hybrid, mirrors [[ADR-001-Hybrid-Transport-Layer]])

Two-tier, matching the existing transport fallback:

1. **API up** — `POST /search/` with `Content-Type: application/vnd.olrapi.jsonlogic+json`, building a **JsonLogic** predicate tree from `match_on`. JsonLogic is chosen over Dataview DQL because it is **injection-safe** — `match_on` values (user-controlled roaster/bean names) ride as JSON *data*, never interpolated into a query string — while still covering equality (`==`), wikilink equality, list-overlap (`in`), and date windows (`>=`). Obsidian returns matching files with frontmatter. Fast; filtering pushed server-side. See [[obsidian-local-rest-api]] §search.

   ```json
   {"and":[
     {"==":[{"var":"frontmatter.roaster"},"Onyx"]},
     {"==":[{"var":"frontmatter.brew_method"},"V60"]}
   ]}
   ```
2. **API down** — filesystem scan of the module's `source`/output directory: `list_directory_entries` → `read_file` with `Accept: application/vnd.olrapi.note+json` semantics (frontmatter parse) → filter/rank in-process. Bounded: stop once `limit` matches are found per tier.

**Why not the in-memory history log?** `HistoryEntry` (`src/data/history.rs`) stores only `id`, `module_key`, `timestamp`, `vault_path`, `first_field` — **not** field values. The heatmap's "pure in-memory view" trick does not apply; the corpus must be read from the notes. (Enriching `history.jsonl` with field values is a *possible* future fast-path — see §11 — but deliberately out of L1 to avoid a log-schema change.)

New transport surface: a `search(module, predicate) -> Vec<CaptureFrontmatter>` method wrapping `/search/` with the FS-scan fallback. `read_file` and `list_directory_entries` already exist; **a YAML frontmatter *reader* and a wikilink *stripper* do NOT** — the codebase writes frontmatter (`src/output/frontmatter.rs`) and *wraps* wikilinks but has no reader/stripper and no YAML-parser crate (ADR-002 hand-rolls YAML *writing*). Both are built as shared `src/data/`-style foundation in L1 (see §10) and reused by [[pour-lookup-fields]]. On the API path, Obsidian returns pre-parsed frontmatter via `application/vnd.olrapi.note+json`, so the reader is exercised only on the FS-fallback path.

## 8. UX

### 8.1 TUI (L1 target)

- Panel renders to the **right** of the form when terminal width ≥ ~100 cols; **stacks below** the form when narrower. The "always-on when data exists" promise degrades gracefully, never breaks the form.
- Appears automatically when the matched tier is non-empty. Hidden with no matches (shows a one-line empty state instead of an empty box).
- `Ctrl+R` toggles the panel (collapse/expand) for users who want a minimal form.
- Header line: `<matched-tier> · <rank-label>` (e.g. `Onyx · V60 · your ⭐4+`, or `any roaster · V60 · recent`).
- Trigger timing: resolve at form-open and on any `match_on`-field change. **Never at submit** (read-only). Reuse the [[pour-lookup-fields]] trigger model.

### 8.2 PWA

Out of scope for L1 (like lookup-fields). L2: render the same panel below the form on mobile; needs the contract amendment (§9) landed first per `feedback_contract_first`.

### 8.3 Rendering notes

- Column widths derived from `show` field types (numbers right-aligned, capped precision).
- Ratio display (`1:16`) is opt-in per module via a display hint, not a summary type — deferred to L2. L1 shows raw `dose_g`/`yield_g`.

## 9. API Contract Impact

Deferred to L2 (PWA). When it lands:

```
POST /api/v1/priors/{module}
  body: { match_values: { <field>: <value>, … } }
  → 200 { tier: "roaster+method", rank: "rating desc",
          rows: [ { … frontmatter subset … } ],
          summary: { dose_g: 15, … } | null }
  → 200 { tier: null, rows: [], summary: null }   # empty state
```

No submit-path change — this is a pure read endpoint. Contract amendment lands before PWA UI per contract-first discipline.

## 10. Phasing

### L1 — Coffee's exact path (TUI)

- **Foundation (new, folded into L1):** a YAML frontmatter *reader* (parses the `---` block of Pour-written notes into key/values; must not crash on richer externally-edited YAML but need not fully model it) and a wikilink *stripper* (`[[Target|Alias]]`/`[[Target#Frag]]` → `Target`), built as shared `src/data/`-style utilities with their own tests. Reused by [[pour-lookup-fields]]. **Design note (Architect's call, record in impl-notes):** crate (e.g. `serde_yaml`) vs. hand-rolled, weighed against round-trip fidelity with the ADR-002 hand-rolled *writer* — if a crate is chosen, assess whether ADR-002 needs an amendment.
- Schema: `[modules.<key>.priors]` with `match_on` (equality + wikilink), `rank_by` (`<field> desc/asc`), `show`, `limit`; zero-config default.
- Resolver in `src/priors/` (new module): cascade match, qualifying-first + dim-texture rank, numeric-median summary.
- Transport: `search()` wrapper — `/search/` **JsonLogic** fast path + FS-scan fallback (both in L1, per [[ADR-001-Hybrid-Transport-Layer]]).
- TUI: right/below panel, header (with qualifier logic §5.4), `Ctrl+R` toggle, form-open + field-change triggers.
- Tests: `tests/priors_resolver.rs` (cascade, qualifying/texture split, all-texture degeneration, summary source), `tests/priors_transport.rs` (JsonLogic search + FS fallback parity), plus a JsonLogic-builder unit test asserting no value reaches the query as a string literal.
- Docs: `field-types.md` gains the `[priors]` block; this spec → `shipped`.
- **Deferred, specced:** tag-overlap, recency-window, non-numeric summaries, `me`/`note` panels, PWA. Vocabulary designed up front so these are pure additions — no re-architecture.

### L2 — Generalization + PWA

- Match modes `overlap`, `window`.
- Per-field-type summary (mode for selects/tags).
- `me`/`note` recent-N + `#tag` overlap.
- `POST /api/v1/priors/{module}` + PWA panel.

### L3 — Optional fast-path + standalone surface

- `history.jsonl` field-value enrichment for pure-in-memory review (if FS scan feels slow at scale).
- `pour review <module>` standalone read surface on the same engine (only if a real need surfaces — Bases covers exploration).

## 11. Out of Scope (v1)

- **A query language / filter UI at capture time** — forever. The config is the query; capture stays low-load. This is the load-bearing boundary.
- **Standalone browse/explore screen** — Bases/Dataview own exploration. `pour review <module>` is an L3 *maybe*, not a goal.
- **Writing back to notes** — read-only, like all review surfaces.
- **Cross-module priors** — single-module scope per capture.
- **`history.jsonl` enrichment** — L3 only; avoid a log-schema change in L1.

## 12. Open Questions

### Resolved (design interview, 2026-07-13)

- ~~**Naming**~~ → **Priors**; config key is `[modules.<x>.priors]`.
- ~~**Missing-field rows in ranking**~~ → **qualifying-first, then dim texture** (§5), not exclusion. Header drops the rank qualifier when texture is mixed in.
- ~~**DQL vs JsonLogic**~~ → **JsonLogic** (injection-safe), and the `/search/` fast path ships **in L1** alongside the FS fallback.
- ~~**Summary aggregation**~~ → **configurable per shown field, default `median`** (§6). `mean`/`max`/`min`/`latest` via object form.
- ~~**Zero-config `show`**~~ → **numeric + select fields, first 4 by config order** (§3), user-overridable.
- ~~**Narrow terminal**~~ → **stack below the form**, full width (§8.1). Carries a min-terminal-height caveat (open #2 below).

### Resolved (L1 build, 2026-07-13)

1. ~~**JsonLogic frontmatter accessor shape**~~ → **`{"var": "frontmatter.<key>"}`**, confirmed from the Obsidian Local REST API OpenAPI spec (`obsidian-local-rest-api-openapi.yaml`, `/search/` examples: `find_by_frontmatter_value`). Equality uses `{"==": [...]}`. *[Deviation: the spec claimed `/search/` returns "matching files with frontmatter"; the API actually returns `[{filename, result}]`, so the L1 build fetches each note's frontmatter via `Accept: application/vnd.olrapi.note+json`. And, to keep the two transport paths provably equivalent under the new-bag cascade, L1 fetches the module corpus once and runs the (pure, tested) resolver in-process rather than pushing each cascade tier server-side; the injection-safe JsonLogic builder ships and is tested as the documented query surface, so server-side filtering remains a localized future change.]*
2. ~~**Stack-below min height**~~ → **9-row form minimum**: the stacked panel is only rendered as a box when ≥ 6 rows remain after reserving 9 rows for the form; otherwise it collapses to the one-line summary hint (`▸ repeat: … — ^R for rows`). The `Ctrl+R` collapse reserves a single hint row.

### Still open

3. **Window timezone** — `mode="window"` uses `today_local` (server-side, per [[pour-lookup-fields]] §11.2). Document the same caveat. *(L2 — only relevant once `window` mode lands.)*

## 13. Cross-references

- [[priors_at_the_pour]] — the story / vision framing.
- [[pour-lookup-fields]] — shares frontmatter-read + wikilink-resolution + trigger-model plumbing; slot this behind it.
- [[history_heatmap_dashboard]] — sibling review surface (cadence vs. content).
- [[ADR-001-Hybrid-Transport-Layer]] — the API→FS fallback this data path mirrors.
- [[obsidian-local-rest-api]] — `POST /search/` DQL/JsonLogic contract.
- [[pour_without_obsidian]] — `pour query` lineage; this is its first concrete increment.
- [[pour-api-contract]] — L2 amendment target (§9).

## 14. Change Log

- **2026-07-13 (L1 shipped)** — Coffee's exact path landed (TUI). Shared foundation `src/data/frontmatter_read.rs` (hand-rolled reader — see [[ADR-007-Frontmatter-Reader]]) + `src/data/wikilink.rs` stripper. `src/priors/` resolver (cascade widens by dropping the *most-specific/front* key — the spec's "tail" wording; the story's `bean → roaster+method` ordering is authoritative), injection-safe JsonLogic builder, transport corpus-fetch (API note+json / FS scan). Config `[priors]` block deserializes the full L1+L2 vocabulary; L1 validation rejects `overlap`/`window` modes and `max`/`min` rank forms. Open questions #1 (accessor shape) and #2 (stack-below min height) resolved above. No `config_version` bump (additive, all-optional block). Deferred to L2/L3 unchanged.
- **2026-07-13** — Initial draft. Scoped via design interview: inline TUI panel, config-declared query (never user-authored), pluggable `rank_by`, ordered new-bag match cascade, per-field-type summaries, hybrid `/search`→FS data path, zero-config defaults. L1 = coffee's exact path; full vocabulary designed but deferred to L2/L3. Status: not yet scheduled — requires roadmap allocation behind [[pour-lookup-fields]].
