---
tags: [spec, review, priors, query, tui]
aliases:
  - priors spec
  - review priors
  - pour review
  - review panel spec
date created: Monday, July 13th 2026, 2:05:00 pm
status: partial — L1 and L1.5 shipped (TUI, unreleased); L2/L3 not started
date modified: Saturday, October 3rd 2026, 6:47:09 am
---

# Pour Review / Priors Panel

> An inline, config-declared review surface: at capture time, Pour reads back the most-relevant prior captures for the module being logged — matched, ranked, and summarized entirely from `config.toml` — and renders them beside the form. The user never writes a query. Concrete first use case: dialing in a brew by seeing your most similar past brews side by side, each one a whole real recipe. See the story: [[priors_at_the_pour]].

## 1. Motivation

Pour writes rich structured frontmatter but offers __no read-back at the point of decision__. To answer "what did I do last time for this roaster?" the user must leave the terminal, open Obsidian, and build or find a Dataview/Bases view. That context switch is exactly the friction the [[the_pour_manifesto|manifesto]] exists to kill — and it happens at the worst moment, mid-capture, kettle on.

The manifesto's *Capture First, Synthesize Later* cedes __reflection__ to Obsidian. This feature is not reflection; it is __decision support in service of the very next capture__ — which is capture-first by definition. Bases owns the Sunday table; it cannot live in the terminal capture flow. That gap is the feature.

The value is __not__ parsing and presenting frontmatter (Bases already does that — commodity). The value is __curation + placement + timing__: *your closest* priors, *beside the fields you are about to fill*, *without leaving the terminal*, *with no query to author*.

## 2. Concept

The panel is __reference, not prescription__. Every value it shows comes from one capture the user actually made. It never blends values across captures by default, because a median dose next to a median yield next to a median time reads like a recipe and is one nobody brewed.

A per-module `[modules.<key>.priors]` block declares:

1. __`match_on`__ — the fields that define "similar", ordered most → least important. Priors are scored by how many of these agree with the form's current values (§4.1).
2. __`rank_by`__ — a tie-breaker between equally similar priors (default `recent`). Finding the all-time *best* captures is Obsidian's job (Bases/Dataview), not the panel's.
3. __`show`__ — which fields get a panel row. Defaults to every visible field, so the panel lines up with the form.
4. __`limit`__ — how many priors to show, one column each (default 3).
5. __`summary`__ — opt-in aggregate column (§6). Off by default.

At form-open and whenever a `match_on` field or a hard-filter gate changes, the resolver:

1. Collects the module's prior captures (via `/search/` or FS scan — §7).
2. Applies hard filters: a field that gates other fields through `show_when` must match exactly, so espresso priors never appear on a pour-over form (§4.1).
3. Scores the rest by similarity and orders them, with `rank_by` then recency breaking ties (§5).
4. Renders the top `limit` priors as columns whose rows line up with the form's fields (§8.1).

The panel is __read-only__. It never writes, never blocks submit, and never fires at submit time.

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

Every key is optional. __Zero-config default__ (no `[priors]` block at all): `match_on` = every `wikilink`/select field in config order; `rank_by = "recent"`; `show` = every visible field; `limit = 3`; `summary = false`. The hard filter (§4.1) needs no config. This makes a useful panel appear for `me`/`note` with no setup, while `coffee` opts into richness.

### 3.1 `rank_by` Grammar

| Form | Meaning | Example |
|---|---|---|
| `"<field> desc"` / `"<field> asc"` | Sort by a field | `rating desc` — best shots |
| `"<field> max"` / `"<field> min"` | Single extreme (PR-style) | `weight_g max` — heaviest set |
| `"recent"` | Newest capture first | journals |
| `"none"` | Preserve scan order, unranked | — |

`"best" is domain-defined by rank_by` — the primitive has no built-in notion of quality.

Since L1.5, `rank_by` only breaks ties between equally similar priors (§5). Similarity decides the order. A user who wants their all-time best captures builds that view in Obsidian. `max`/`min` matter less under this model; whether they ship at all is open (GitHub issue #11).

## 4. Match Semantics

### 4.1 Similarity Scoring

Two steps, in order:

1. __Hard filter.__ A field that gates other fields through `show_when` (coffee: `brew_method`) and has a value on the form must match exactly. Priors with a different value are dropped, because their fields describe a different form. This comes from the module's `show_when` rules, so it needs no config and no coffee-specific code.
2. __Score.__ For each remaining prior, count the `match_on` fields where the prior agrees with the form's current value. Fields the user hasn't filled are ignored. A higher count ranks first. Equal counts are broken by *which* fields agree, in `match_on` order, so agreeing on `bean` beats agreeing on `intent`.

Nothing is dropped for scoring low. If no prior agrees on anything past the hard filter, the panel still shows the closest priors and its header says so (`no close match`).

The new-bag story still holds. A new bean agrees on nothing in `bean`, so its brews with the same brewer and grinder rise to the top on their own, without a separate widening step. The strict cascade (L1) is the special case where only full agreement counts.

__Weighting (L1.5):__ a plain count. Order matters only between equal counts, compared field by field in `match_on` order, so a prior agreeing on `bean` alone outranks one agreeing on `intent` alone, but a prior agreeing on `brewer` and `grinder` outranks one agreeing on `bean` alone. An order-weighted score would let the first key outvote all the others, which is the strict cascade again by another name.

__No minimum score (L1.5).__ The panel is never hidden for scoring low. `no close match` appears only when the form has at least one `match_on` field filled. At form-open with nothing filled there is nothing to miss, so the header stays `similar` and the order falls to `rank_by` and recency.

__Missing gate key (L1.5).__ A prior with no value at all for a filled gate is dropped along with the mismatches. Nothing shows it describes the current form's shape.

__Hidden fields don't count (L1.5).__ The form keeps a field's value after `show_when` hides it, the same way it does for submit. The resolver sees only the values of fields the form currently shows, so a `brewer` picked under Pour Over and left behind by a switch to Espresso neither gates, scores, nor turns the header into `no close match`. Submit drops hidden fields by the same rule.

### 4.2 Match Modes

| Mode | Config | Semantics | Phase |
|---|---|---|---|
| `equality` | bare string `"roaster"` | exact frontmatter value match | __L1__ |
| `wikilink` | bare string on a `wikilink` field | strip `[[ ]]`/alias/fragment, compare targets | __L1__ (special case of equality) |
| `overlap` | `{field, mode="overlap"}` | non-empty intersection of list/tag values counts as agreement | L2 |
| `window` | `{field, mode="window", days=N}` | capture within N days of today; a filter, not a score | L2 |

Bare string ⇒ `equality` (or `wikilink` if the field is declared `wikilink = true`). Object form ⇒ explicit mode. Simple stays simple; complexity is pay-as-you-go.

### 4.3 Text-only Modules (`me`/`note`)

No wikilink/select/number to match on. Zero-config behavior: __recent-N of the same module, unfiltered__, PLUS `#tag`-in-body overlap when the body contains inline tags (parse `#\w+`, match on intersection). This gives journals a "recent context" rail without demanding schema they don't have.

## 5. Ranking

Columns are ordered left to right by:

1. __Similarity score__ (§4.1), highest first.
2. __`rank_by`__, for priors with equal scores. With `rank_by = "<field> desc|asc"`, a prior missing that field sorts after the ones that have it.
3. __Recency__, newest first.

A prior missing the `rank_by` field (an unrated brew) is still shown, with its column __dimmed__. The header reads `similar`, plus the `rank_by` label when one is set (`similar · rating desc`). It never claims "best": the panel shows what is close, not what is good.

## 6. Summary Column (Opt-in, Per-field-type)

__Off by default.__ `summary = true` adds one extra column on the right, labeled with its aggregation, that summarizes each row across the displayed priors. It is off by default because it mixes values from different captures, which is exactly what §2 warns against. It earns its place where an average is the point: a stable process variable (water temp), or a `lift` module's working weight.

When enabled, each shown field summarizes per its type, with an optional per-field override:

| Field type | Default agg | Override options |
|---|---|---|
| `number` | `median` (outlier-robust) → `repeat: 15g · 1:16 · 2:55` | `mean` · `max` · `min` · `latest` |
| `static_select` / `dynamic_select` / tags | `mode` (most common) → `most-seen venue: The Fillmore` | `latest` |
| `text` / `textarea` | omitted | — |

__Median is the default__ — a single 40s pull won't drag the target on a small, noisy set. `mean` is available where central tendency over *every* value is what you want (stable process vars — water temp; a `lift` module's working-set average). Configured with the __same bare-string-vs-object pattern as `match_on`__ (§4.2):

```toml
show = [
  "dose_g",                              # median (default)
  "time_s",                              # median
  { field = "water_temp_c", agg = "mean" },
]
```

Rows with no summarizable type stay blank in the summary column. Ratios (`1:16`) are a coffee-specific *render* of two numeric fields; the primitive summarizes each number independently — ratio formatting is a display concern (§8.3), not a summary type.

## 7. Data Path (Hybrid, Mirrors [[ADR-001-Hybrid-Transport-Layer]])

Two-tier, matching the existing transport fallback:

1. __API up__ — `POST /search/` with `Content-Type: application/vnd.olrapi.jsonlogic+json`, building a __JsonLogic__ predicate tree from `match_on`. JsonLogic is chosen over Dataview DQL because it is __injection-safe__ — `match_on` values (user-controlled roaster/bean names) ride as JSON *data*, never interpolated into a query string — while still covering equality (`==`), wikilink equality, list-overlap (`in`), and date windows (`>=`). Obsidian returns matching files with frontmatter. Fast; filtering pushed server-side. See [[obsidian-local-rest-api]] §search.

   ```json
   {"and":[
     {"==":[{"var":"frontmatter.roaster"},"Onyx"]},
     {"==":[{"var":"frontmatter.brew_method"},"V60"]}
   ]}
   ```

2. __API down__ — filesystem scan of the module's `source`/output directory: `list_directory_entries` → `read_file` with `Accept: application/vnd.olrapi.note+json` semantics (frontmatter parse) → filter/rank in-process. Bounded: stop once `limit` matches are found per tier.

*[Deviation: neither L1 nor L1.5 calls `POST /search/`. Both paths list the module folder (`list_directory_entries`), then read every note in it: the API path as `note+json`, the FS path from disk. The resolver filters in-process (see Resolved #1). The fetch stops at 500 notes, not at `limit` matches per tier. The folder is the module `path` up to its last `/`, taken literally, so a strftime or `{{field}}` token in the folder part, like the default config's `Coffee/%Y/`, points at a folder that doesn't exist and the panel shows its empty state.]*

__Why not the in-memory history log?__ `HistoryEntry` (`src/data/history.rs`) stores only `id`, `module_key`, `timestamp`, `vault_path`, `first_field` — __not__ field values. The heatmap's "pure in-memory view" trick does not apply; the corpus must be read from the notes. (Enriching `history.jsonl` with field values is a *possible* future fast-path — see §11 — but deliberately out of L1 to avoid a log-schema change.)

New transport surface: a `search(module, predicate) -> Vec<CaptureFrontmatter>` method wrapping `/search/` with the FS-scan fallback. `read_file` and `list_directory_entries` already exist; __a YAML frontmatter *reader* and a wikilink *stripper* do NOT__ — the codebase writes frontmatter (`src/output/frontmatter.rs`) and *wraps* wikilinks but has no reader/stripper and no YAML-parser crate (ADR-002 hand-rolls YAML *writing*). Both are built as shared `src/data/`-style foundation in L1 (see §10) and reused by [[pour-lookup-fields]]. On the API path, Obsidian returns pre-parsed frontmatter via `application/vnd.olrapi.note+json`, so the reader is exercised only on the FS-fallback path.

## 8. UX

### 8.1 TUI (L1.5 Target)

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
   Dose (g)        18                  │  ·        16       ·        │
   Yield (g)       <empty>             │  270      250      280      │
   Total time (s)  <empty>             │  165      150      185      │
   Water temp      <empty>             │  94       96       93       │
   Filter          <empty>             │  Sibarist Sibarist Cafec    │
   Recipe          add rows            │                             │
   Rating          <empty>             │  4.5      4        3.5      │
                                       │  3d       1w       2w       │
                                       └─────────────────────────────┘
```

*(illustrative values)*

- __Column = one real prior.__ Read top to bottom, it is a whole capture. Nothing is blended (summary column aside, §6).
- __Row = the field beside it.__ Read across, it is the range for that field. The active field's row is highlighted across the panel.
- __`·` means "same as the form's current value".__ A value that differs from the current selection renders highlighted, so the user sees at a glance how close each prior is. A value filled from a field's config `default` counts as current like any other, so at form-open the Dose row above shows `·` for every prior brewed at 18 g. The marks are computed each frame from the live form, so typing a dose moves them without a new fetch.
- __Column header:__ the `rank_by` value when set (`★4.5`). __Column footer:__ age (`3d`, `1w`). *[Deviation: L1.5 prints the stored value without the `★`. The glyph would assume every `rank_by` field is a rating.]*
- __Unrated priors__ (missing `rank_by`) render dimmed (§5).
- __Row alignment is the hard part.__ Some form items take two lines (callout fields), and the preset row sits above the fields. Panel row heights must come from the same visible-field item list `tui/form/render/fields.rs` builds, not from a parallel count.
- __Composite fields__ render blank. Presets are how you reuse recipe stages, so the panel doesn't summarize them. __Textarea fields__ stay blank unless listed in `show`.
- __Narrow terminals drop columns__ (3 → 2 → 1) instead of moving the panel below the form, so rows stay aligned. Below one column's width the panel collapses to a one-line hint. L1.5 thresholds: the form keeps 60 columns and each panel column is 10 wide, so 3 columns need 94, 2 need 84, 1 needs 74. The summary column (§6) is dropped before any prior.
- Appears automatically when any prior survives the hard filter. With none, a one-line empty state.
- `Ctrl+R` toggles the panel (collapse/expand).
- Trigger timing: resolve at form-open and on any `match_on` or gate field change. A gate re-resolves even when it isn't in `match_on`, because it changes which priors survive. __Never at submit__ (read-only). Reuse the [[pour-lookup-fields]] trigger model.

### 8.2 PWA

Out of scope for L1 (like lookup-fields). L2: needs the contract amendment (§9) landed first per `feedback_contract_first`. Side-by-side columns do not fit a phone. The PWA needs its own presentation of the same data, for example one card per prior in form-field order, swiped. Design it in L2.

### 8.3 Rendering Notes

- Column widths derived from `show` field types (numbers right-aligned, capped precision). *[Deviation: L1.5 uses one fixed 9-cell width for every column, left-aligned. Longer values truncate with `…`.]*
- __Shared prefixes are elided.__ When a value is too long for its cell and shares a leading run with a different value on the same screen line, either another column or the form's own value, the panel replaces the shared run with `…`, cut back to a word boundary. Beans named `YesPlz - Homestar` and `YesPlz - SOE Ethiopia` read `…Homestar` and `…SOE Ethi…` rather than `YesPlz -…` twice. Values that fit are shown whole. `·` is still decided on the full value.
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

### L1 — Coffee's Exact Path (TUI)

- __Foundation (new, folded into L1):__ a YAML frontmatter *reader* (parses the `---` block of Pour-written notes into key/values; must not crash on richer externally-edited YAML but need not fully model it) and a wikilink *stripper* (`[[Target|Alias]]`/`[[Target#Frag]]` → `Target`), built as shared `src/data/`-style utilities with their own tests. Reused by [[pour-lookup-fields]]. __Design note (Architect's call, record in impl-notes):__ crate (e.g. `serde_yaml`) vs. hand-rolled, weighed against round-trip fidelity with the ADR-002 hand-rolled *writer* — if a crate is chosen, assess whether ADR-002 needs an amendment.
- Schema: `[modules.<key>.priors]` with `match_on` (equality + wikilink), `rank_by` (`<field> desc/asc`), `show`, `limit`; zero-config default.
- Resolver in `src/priors/` (new module): cascade match, qualifying-first + dim-texture rank, numeric-median summary.
- Transport: `search()` wrapper — `/search/` __JsonLogic__ fast path + FS-scan fallback (both in L1, per [[ADR-001-Hybrid-Transport-Layer]]).
- TUI: right/below panel, header (with qualifier logic §5.4), `Ctrl+R` toggle, form-open + field-change triggers.
- Tests: `tests/priors_resolver.rs` (cascade, qualifying/texture split, all-texture degeneration, summary source), `tests/priors_transport.rs` (JsonLogic search + FS fallback parity), plus a JsonLogic-builder unit test asserting no value reaches the query as a string literal.
- Docs: `field-types.md` gains the `[priors]` block; this spec → `shipped`.
- __Deferred, specced:__ tag-overlap, recency-window, non-numeric summaries, `me`/`note` panels, PWA. Vocabulary designed up front so these are pure additions — no re-architecture.

### L1.5 — Reference Columns (TUI)

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

### L3 — Optional Fast-path + Standalone Surface

- `history.jsonl` field-value enrichment for pure-in-memory review (if FS scan feels slow at scale).
- `pour review <module>` standalone read surface on the same engine (only if a real need surfaces — Bases covers exploration).

## 11. Out of Scope (V1)

- __A query language / filter UI at capture time__ — forever. The config is the query; capture stays low-load. This is the load-bearing boundary.
- __Standalone browse/explore screen__ — Bases/Dataview own exploration. `pour review <module>` is an L3 *maybe*, not a goal.
- __Writing back to notes__ — read-only, like all review surfaces.
- __Cross-module priors__ — single-module scope per capture.
- __`history.jsonl` enrichment__ — L3 only; avoid a log-schema change in L1.

## 12. Open Questions

### Resolved (Design Interview, 2026-07-13)

- ~~__Naming__~~ → __Priors__; config key is `[modules.<x>.priors]`.
- ~~__Missing-field rows in ranking__~~ → __qualifying-first, then dim texture__ (§5), not exclusion. Header drops the rank qualifier when texture is mixed in.
- ~~__DQL vs JsonLogic__~~ → __JsonLogic__ (injection-safe), and the `/search/` fast path ships __in L1__ alongside the FS fallback.
- ~~__Summary aggregation__~~ → __configurable per shown field, default `median`__ (§6). `mean`/`max`/`min`/`latest` via object form.
- ~~__Zero-config `show`__~~ → __numeric + select fields, first 4 by config order__ (§3), user-overridable.
- ~~__Narrow terminal__~~ → __stack below the form__, full width (§8.1). Carries a min-terminal-height caveat (open #2 below). *Superseded by L1.5 (2026-10-02): narrow terminals drop columns and the panel stays beside the form.*

### Resolved (L1 Build, 2026-07-13)

1. ~~__JsonLogic frontmatter accessor shape__~~ → __`{"var": "frontmatter.<key>"}`__, confirmed from the Obsidian Local REST API OpenAPI spec (`obsidian-local-rest-api-openapi.yaml`, `/search/` examples: `find_by_frontmatter_value`). Equality uses `{"==": […]}`. *[Deviation: the spec claimed `/search/` returns "matching files with frontmatter"; the API actually returns `[{filename, result}]`, so the L1 build fetches each note's frontmatter via `Accept: application/vnd.olrapi.note+json`. And, to keep the two transport paths provably equivalent under the new-bag cascade, L1 fetches the module corpus once and runs the (pure, tested) resolver in-process rather than pushing each cascade tier server-side; the injection-safe JsonLogic builder ships and is tested as the documented query surface, so server-side filtering remains a localized future change.]*
2. ~~__Stack-below min height__~~ → __9-row form minimum__: the stacked panel is only rendered as a box when ≥ 6 rows remain after reserving 9 rows for the form; otherwise it collapses to the one-line summary hint (`▸ repeat: … — ^R for rows`). The `Ctrl+R` collapse reserves a single hint row. *Superseded by L1.5 (2026-10-02): there is no stacked layout, so there is no height budget. The one-line hint takes the bottom row.*

### Resolved (L1 Review, 2026-10-02)

- ~~__Panel shape__~~ → __one column per prior, rows aligned to form fields__ (§8.1). Considered and rejected: inline ghost hints in the form, sourced per field. They read as prescription, and per-field aggregation invents a recipe.
- ~~__Same-as-current cells__~~ → render as __`·`__, so differences stand out.
- ~~__Ordering__~~ → __similarity first__; `rank_by` is a tie-breaker (§5). Best-of views belong in Obsidian. Weighting details are the Architect's call.
- ~~__Summary line__~~ → __opt-in summary column__, off by default (§6).

### Still Open

3. __Window timezone__ — `mode="window"` uses `today_local` (server-side, per [[pour-lookup-fields]] §11.2). Document the same caveat. *(L2 — only relevant once `window` mode lands.)*
4. __Minimum similarity__ — show the closest priors even when nothing agrees past the hard filter (current spec), or hide the panel below some score? Architect's call in L1.5, revisit with use. *L1.5 chose: no minimum, and a plain-count score with earlier-field tie-breaks (§4.1). Revisit once the corpus is larger.*

## 13. Cross-references

- [[priors_at_the_pour]] — the story / vision framing.
- [[pour-lookup-fields]] — shares frontmatter-read + wikilink-resolution + trigger-model plumbing; slot this behind it. *[Deviation: priors shipped first. Lookup-fields has not started, so L1 built the shared reader and stripper itself.]*
- [[history_heatmap_dashboard]] — sibling review surface (cadence vs. content).
- [[ADR-001-Hybrid-Transport-Layer]] — the API→FS fallback this data path mirrors.
- [[obsidian-local-rest-api]] — `POST /search/` DQL/JsonLogic contract.
- [[pour_without_obsidian]] — `pour query` lineage; this is its first concrete increment.
- [[pour-api-contract]] — L2 amendment target (§9).

## 14. Change Log

- __2026-10-02 (L1.5 built)__ — Reference columns landed in the TUI. The resolver hard-filters on `show_when` gates, scores by plain `match_on` agreement count with earlier-field tie-breaks, and orders by score, `rank_by`, then recency. No minimum score; `no close match` needs at least one filled `match_on` field. Config: `limit` defaults to 3, `show` to every non-textarea, non-composite field, zero-config `match_on` to every wikilink/select field in config order, and a new opt-in `summary` key. No `config_version` bump. The panel draws one column per prior with rows placed from the form's own item heights, `·` for values equal to the live form (defaults included), dimmed columns for priors missing `rank_by`, `rank_by` values as column headers and ages as footers. Narrow terminals drop columns, then fall back to a one-line hint; the stacked layout and `repeat:` line are gone. Composite rows render blank. The resolver ignores values of fields `show_when` hides, and cells elide a prefix shared with another value on the same row. Deviation notes closed by this work were removed.

- __2026-10-02 (L1.5 specced)__ — First real use showed the L1 panel was not useful: zero-config showed fields the user had already filled, and the strict cascade rarely found similar brews on a 17-brew corpus. Revised to reference columns: one column per real prior, rows aligned to the form, `·` for same-as-current, similarity scoring with a `show_when` hard filter in place of the cascade, `rank_by` demoted to tie-breaker, `repeat:` line replaced by an opt-in summary column. L1 behavior annotated as deviations. New phase L1.5 (§10).

- __2026-07-13 (L1 shipped)__ — Coffee's exact path landed (TUI). Shared foundation `src/data/frontmatter_read.rs` (hand-rolled reader — see [[ADR-007-Frontmatter-Reader]]) + `src/data/wikilink.rs` stripper. `src/priors/` resolver (cascade widens by dropping the *most-specific/front* key — the spec's "tail" wording; the story's `bean → roaster+method` ordering is authoritative), injection-safe JsonLogic builder, transport corpus-fetch (API note+json / FS scan). Config `[priors]` block deserializes the full L1+L2 vocabulary; L1 validation rejects `overlap`/`window` modes and `max`/`min` rank forms. Open questions #1 (accessor shape) and #2 (stack-below min height) resolved above. No `config_version` bump (additive, all-optional block). Deferred to L2/L3 unchanged.
- __2026-07-13__ — Initial draft. Scoped via design interview: inline TUI panel, config-declared query (never user-authored), pluggable `rank_by`, ordered new-bag match cascade, per-field-type summaries, hybrid `/search`→FS data path, zero-config defaults. L1 = coffee's exact path; full vocabulary designed but deferred to L2/L3. Status: not yet scheduled — requires roadmap allocation behind [[pour-lookup-fields]].
