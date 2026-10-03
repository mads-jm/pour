---
tags: [spec, review, priors, query, tui]
aliases:
  - priors spec
  - review priors
  - pour review
  - review panel spec
date created: Monday, July 13th 2026, 2:05:00 pm
status: shipped — L1 (coffee, TUI); revising — L1.5 reference columns
date modified: Friday, October 2nd 2026, 12:00:00 pm
---

# Pour Review / Priors Panel

> An inline, config-declared review surface: at capture time, Pour reads back the most-relevant prior captures for the module being logged — matched, ranked, and summarized entirely from `config.toml` — and renders them beside the form. The user never writes a query. Concrete first use case: dialing in a brew by seeing your most similar past brews side by side, each one a whole real recipe. See the story: [[priors_at_the_pour]].

## 1. Motivation

Pour writes rich structured frontmatter but offers **no read-back at the point of decision**. To answer "what did I do last time for this roaster?" the user must leave the terminal, open Obsidian, and build or find a Dataview/Bases view. That context switch is exactly the friction the [[the_pour_manifesto|manifesto]] exists to kill — and it happens at the worst moment, mid-capture, kettle on.

The manifesto's *Capture First, Synthesize Later* cedes **reflection** to Obsidian. This feature is not reflection; it is **decision support in service of the very next capture** — which is capture-first by definition. Bases owns the Sunday table; it cannot live in the terminal capture flow. That gap is the feature.

The value is **not** parsing and presenting frontmatter (Bases already does that — commodity). The value is **curation + placement + timing**: *your closest* priors, *beside the fields you are about to fill*, *without leaving the terminal*, *with no query to author*.

## 2. Concept

The panel is **reference, not prescription**. Every value it shows comes from one capture the user actually made. It never blends values across captures by default, because a median dose next to a median yield next to a median time reads like a recipe and is one nobody brewed.

A per-module `[modules.<key>.priors]` block declares:

1. **`match_on`** — the fields that define "similar", ordered most → least important. Priors are scored by how many of these agree with the form's current values (§4.1).
2. **`rank_by`** — a tie-breaker between equally similar priors (default `recent`). Finding the all-time *best* captures is Obsidian's job (Bases/Dataview), not the panel's.
3. **`show`** — which fields get a panel row. Defaults to every visible field, so the panel lines up with the form.
4. **`limit`** — how many priors to show, one column each (default 3).
5. **`summary`** — opt-in aggregate column (§6). Off by default.

At form-open and on every `match_on`-field change, the resolver:

1. Collects the module's prior captures (via `/search/` or FS scan — §7).
2. Applies hard filters: a field that gates other fields through `show_when` must match exactly, so espresso priors never appear on a pour-over form (§4.1).
3. Scores the rest by similarity and orders them, with `rank_by` then recency breaking ties (§5).
4. Renders the top `limit` priors as columns whose rows line up with the form's fields (§8.1).

The panel is **read-only**. It never writes, never blocks submit, and never fires at submit time.

*[Deviation: L1 shipped a row-per-capture panel. It filters with the strict widen-by-drop cascade (§4.1, old), ranks by `rank_by` first, shows a fixed `show` list (zero-config: the first four select/number fields, which are mostly fields the user has already filled), and always renders the `repeat:` median line. On a small corpus where nearly every combination is unique (17 coffee brews, 2026-10), the cascade almost always widens to `brew_method` alone and the panel reads as a recent-brews list. L1.5 (§10) replaces this.]*

## 3. Schema

```toml
[modules.coffee.priors]
match_on = ["bean", "brewer", "grinder", "intent"]  # similarity, most → least important
rank_by  = "rating desc"                             # tie-breaker only
limit    = 3                                         # one column per prior
# show defaults to every visible field; summary = true adds a median column (§6)

# richer match modes use object form (§4.2); bare string = equality
[modules.me.priors]
match_on = [
  { field = "tags", mode = "overlap" },      # #tag overlap in body
  { field = "date", mode = "window", days = 90 },
]
rank_by = "recent"
```

Every key is optional. **Zero-config default** (no `[priors]` block at all): `match_on` = every `wikilink`/select field in config order; `rank_by = "recent"`; `show` = every visible field; `limit = 3`; `summary = false`. The hard filter (§4.1) needs no config. This makes a useful panel appear for `me`/`note` with no setup, while `coffee` opts into richness.

*[Deviation: L1 zero-config matches on the first `wikilink`/select field only, shows the first four numeric + select fields, and uses `limit = 5`.]*

### 3.1 `rank_by` grammar

| Form | Meaning | Example |
|---|---|---|
| `"<field> desc"` / `"<field> asc"` | Sort by a field | `rating desc` — best shots |
| `"<field> max"` / `"<field> min"` | Single extreme (PR-style) | `weight_g max` — heaviest set |
| `"recent"` | Newest capture first | journals |
| `"none"` | Preserve scan order, unranked | — |

`"best" is domain-defined by rank_by` — the primitive has no built-in notion of quality.

Since L1.5, `rank_by` only breaks ties between equally similar priors (§5). Similarity decides the order. A user who wants their all-time best captures builds that view in Obsidian. `max`/`min` matter less under this model; whether they ship at all is open (GitHub issue #11).

## 4. Match Semantics

### 4.1 Similarity scoring

Two steps, in order:

1. **Hard filter.** A field that gates other fields through `show_when` (coffee: `brew_method`) and has a value on the form must match exactly. Priors with a different value are dropped, because their fields describe a different form. This comes from the module's `show_when` rules, so it needs no config and no coffee-specific code.
2. **Score.** For each remaining prior, count the `match_on` fields where the prior agrees with the form's current value. Fields the user hasn't filled are ignored. A higher count ranks first. Equal counts are broken by *which* fields agree, in `match_on` order, so agreeing on `bean` beats agreeing on `intent`.

Nothing is dropped for scoring low. If no prior agrees on anything past the hard filter, the panel still shows the closest priors and its header says so (`no close match`).

The new-bag story still holds. A new bean agrees on nothing in `bean`, so its brews with the same brewer and grinder rise to the top on their own, without a separate widening step. The strict cascade (L1) is the special case where only full agreement counts.

**Architect's call:** the exact weighting (plain count vs. order-weighted), and whether a minimum score should hide the panel. Record the choice in impl-notes.

*[Deviation: L1 runs the strict widen-by-drop cascade. It resolves the full conjunction of `match_on`, drops the most-specific (front) key on zero rows, and repeats until a tier matches. The header names the matched tier (`Onyx · V60`).]*

### 4.2 Match modes

| Mode | Config | Semantics | Phase |
|---|---|---|---|
| `equality` | bare string `"roaster"` | exact frontmatter value match | **L1** |
| `wikilink` | bare string on a `wikilink` field | strip `[[ ]]`/alias/fragment, compare targets | **L1** (special case of equality) |
| `overlap` | `{field, mode="overlap"}` | non-empty intersection of list/tag values counts as agreement | L2 |
| `window` | `{field, mode="window", days=N}` | capture within N days of today; a filter, not a score | L2 |

Bare string ⇒ `equality` (or `wikilink` if the field is declared `wikilink = true`). Object form ⇒ explicit mode. Simple stays simple; complexity is pay-as-you-go.

### 4.3 Text-only modules (`me`/`note`)

No wikilink/select/number to match on. Zero-config behavior: **recent-N of the same module, unfiltered**, PLUS `#tag`-in-body overlap when the body contains inline tags (parse `#\w+`, match on intersection). This gives journals a "recent context" rail without demanding schema they don't have.

## 5. Ranking

Columns are ordered left to right by:

1. **Similarity score** (§4.1), highest first.
2. **`rank_by`**, for priors with equal scores. With `rank_by = "<field> desc|asc"`, a prior missing that field sorts after the ones that have it.
3. **Recency**, newest first.

A prior missing the `rank_by` field (an unrated brew) is still shown, with its column **dimmed**. The header reads `similar`, plus the `rank_by` label when one is set (`similar · rating desc`). It never claims "best": the panel shows what is close, not what is good.

*[Deviation: L1 ranks by `rank_by` first. Rows that have the field come first ("qualifying"), and dimmed "texture" rows fill up to `limit`. The header shows the rank qualifier only when every row qualifies, and the summary line is computed from qualifying rows.]*

## 6. Summary Column (opt-in, per-field-type)

**Off by default.** `summary = true` adds one extra column on the right, labeled with its aggregation, that summarizes each row across the displayed priors. It is off by default because it mixes values from different captures, which is exactly what §2 warns against. It earns its place where an average is the point: a stable process variable (water temp), or a `lift` module's working weight.

When enabled, each shown field summarizes per its type, with an optional per-field override:

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

Rows with no summarizable type stay blank in the summary column. Ratios (`1:16`) are a coffee-specific *render* of two numeric fields; the primitive summarizes each number independently — ratio formatting is a display concern (§8.3), not a summary type.

*[Deviation: L1 always renders a `repeat:` line of medians under the rows, computed from qualifying rows, with a match-count fallback. There is no `summary` key.]*

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

### 8.1 TUI (L1.5 target)

One column per prior, and each panel row sits on the same screen line as the form field it describes. The form's labels label the panel, so the panel carries no field-name column and three priors fit in roughly the L1 panel width.

```
 ▽ pour coffee — Coffee                ┌ similar · rating desc ──────┐
                                       │  ★4.5     ★4       ★3.5     │
   Brew method*    Pour Over           │  ·        ·        ·        │
   Intent          Bright              │  Balanced ·        Balanced │
   Brewer          Switch              │  ·        ·        V60      │
   Bean            Cyesha Honey        │  ·        Benj Paz ·        │
   Grinder         K-Ultra             │  ·        ·        ·        │
 ▸ Grind setting   <empty>             │▸ 6.5      7        6        │
   Dose (g)        18                  │  18       16       18       │
   Yield (g)       <empty>             │  270      250      280      │
   Total time (s)  <empty>             │  165      150      185      │
   Water temp      <empty>             │  94       96       93       │
   Filter          <empty>             │  Sibarist Sibarist Cafec    │
   Recipe          add rows            │  5 stg    4 stg    5 stg    │
   Rating          <empty>             │  4.5      4        3.5      │
                                       │  3d       1w       2w       │
                                       └─────────────────────────────┘
```
*(illustrative values)*

- **Column = one real prior.** Read top to bottom, it is a whole capture. Nothing is blended (summary column aside, §6).
- **Row = the field beside it.** Read across, it is the range for that field. The active field's row is highlighted across the panel.
- **`·` means "same as the form's current value".** A value that differs from the current selection renders highlighted, so the user sees at a glance how close each prior is.
- **Column header:** the `rank_by` value when set (`★4.5`). **Column footer:** age (`3d`, `1w`).
- **Unrated priors** (missing `rank_by`) render dimmed (§5).
- **Row alignment is the hard part.** Some form items take two lines (callout fields), and the preset row sits above the fields. Panel row heights must come from the same visible-field item list `tui/form/render/fields.rs` builds, not from a parallel count.
- **Composite fields** render a short digest (`5 stg`). Recipe stages are better reused through presets; the panel only shows that a prior had them. **Textarea fields** stay blank unless listed in `show`.
- **Narrow terminals drop columns** (3 → 2 → 1) instead of moving the panel below the form, so rows stay aligned. Below one column's width the panel collapses to a one-line hint.
- Appears automatically when any prior survives the hard filter. With none, a one-line empty state.
- `Ctrl+R` toggles the panel (collapse/expand).
- Trigger timing: resolve at form-open and on any `match_on`-field change. **Never at submit** (read-only). Reuse the [[pour-lookup-fields]] trigger model.

*[Deviation: L1 renders a 34-column panel to the right (≥ 100 cols) or stacked below, one prior per row, cells joined without alignment, header `<matched-tier> · <rank-label>`, and a `repeat:` line + `N of M captures` footer.]*

### 8.2 PWA

Out of scope for L1 (like lookup-fields). L2: needs the contract amendment (§9) landed first per `feedback_contract_first`. Side-by-side columns do not fit a phone. The PWA needs its own presentation of the same data, for example one card per prior in form-field order, swiped. Design it in L2.

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

### L1.5 — Reference columns (TUI)

Replaces L1's row panel with the column layout. Driven by first real use (2026-10): the L1 panel showed back fields the user had already filled and rarely found similar brews.

- Resolver: hard filter from `show_when` gates + similarity scoring (§4.1) in place of the cascade; ordering per §5.
- Output shape: an ordered list of priors (columns) with per-field cell values, a same-as-current flag per cell, and the header/footer values. Drop the `repeat:` line.
- Config: `limit` default 3, `show` default every visible field, new `summary` key (default `false`), zero-config `match_on` = every `wikilink`/select field. No `config_version` bump (additive / defaults only).
- TUI: column panel aligned to form rows (§8.1), `·` for same-as-current, active-row highlight, column dropping on narrow terminals.
- Tests: scoring and hard filter in `tests/priors_resolver.rs` (new-bag case, no-close-match case, tie-breaks), plus a render test pinning row alignment with a two-line form item.
- Docs: `field-types.md` `[priors]` block, this spec's deviation notes removed as they close, architecture overview.
- Carries over unchanged: corpus fetch (`search.rs`, API note+json / FS scan), the frontmatter reader, the wikilink stripper, the JsonLogic builder, config deserialization.

### L2 — Generalization + PWA

- Match modes `overlap`, `window`.
- Per-field-type summary (mode for selects/tags), for the opt-in summary column.
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

### Resolved (L1 review, 2026-10-02)

- ~~**Panel shape**~~ → **one column per prior, rows aligned to form fields** (§8.1). Considered and rejected: inline ghost hints in the form, sourced per field. They read as prescription, and per-field aggregation invents a recipe.
- ~~**Same-as-current cells**~~ → render as **`·`**, so differences stand out.
- ~~**Ordering**~~ → **similarity first**; `rank_by` is a tie-breaker (§5). Best-of views belong in Obsidian. Weighting details are the Architect's call.
- ~~**Summary line**~~ → **opt-in summary column**, off by default (§6).

### Still open

3. **Window timezone** — `mode="window"` uses `today_local` (server-side, per [[pour-lookup-fields]] §11.2). Document the same caveat. *(L2 — only relevant once `window` mode lands.)*
4. **Minimum similarity** — show the closest priors even when nothing agrees past the hard filter (current spec), or hide the panel below some score? Architect's call in L1.5, revisit with use.

## 13. Cross-references

- [[priors_at_the_pour]] — the story / vision framing.
- [[pour-lookup-fields]] — shares frontmatter-read + wikilink-resolution + trigger-model plumbing; slot this behind it.
- [[history_heatmap_dashboard]] — sibling review surface (cadence vs. content).
- [[ADR-001-Hybrid-Transport-Layer]] — the API→FS fallback this data path mirrors.
- [[obsidian-local-rest-api]] — `POST /search/` DQL/JsonLogic contract.
- [[pour_without_obsidian]] — `pour query` lineage; this is its first concrete increment.
- [[pour-api-contract]] — L2 amendment target (§9).

## 14. Change Log

- **2026-10-02 (L1.5 specced)** — First real use showed the L1 panel was not useful: zero-config showed fields the user had already filled, and the strict cascade rarely found similar brews on a 17-brew corpus. Revised to reference columns: one column per real prior, rows aligned to the form, `·` for same-as-current, similarity scoring with a `show_when` hard filter in place of the cascade, `rank_by` demoted to tie-breaker, `repeat:` line replaced by an opt-in summary column. L1 behavior annotated as deviations. New phase L1.5 (§10).

- **2026-07-13 (L1 shipped)** — Coffee's exact path landed (TUI). Shared foundation `src/data/frontmatter_read.rs` (hand-rolled reader — see [[ADR-007-Frontmatter-Reader]]) + `src/data/wikilink.rs` stripper. `src/priors/` resolver (cascade widens by dropping the *most-specific/front* key — the spec's "tail" wording; the story's `bean → roaster+method` ordering is authoritative), injection-safe JsonLogic builder, transport corpus-fetch (API note+json / FS scan). Config `[priors]` block deserializes the full L1+L2 vocabulary; L1 validation rejects `overlap`/`window` modes and `max`/`min` rank forms. Open questions #1 (accessor shape) and #2 (stack-below min height) resolved above. No `config_version` bump (additive, all-optional block). Deferred to L2/L3 unchanged.
- **2026-07-13** — Initial draft. Scoped via design interview: inline TUI panel, config-declared query (never user-authored), pluggable `rank_by`, ordered new-bag match cascade, per-field-type summaries, hybrid `/search`→FS data path, zero-config defaults. L1 = coffee's exact path; full vocabulary designed but deferred to L2/L3. Status: not yet scheduled — requires roadmap allocation behind [[pour-lookup-fields]].
