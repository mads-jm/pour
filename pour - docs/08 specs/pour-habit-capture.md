---
tags:
  - spec
  - modules
  - habits
  - frontmatter
date created: Wednesday, August 5th 2026
date modified: Saturday, October 3rd 2026, 6:47:11 am
status: partial — v1 shipped in v1.1.0; v1.1 date targeting and v2 limits not started
---

# Pour Habit Capture — Frontmatter Mutation Primitives — Spec

## Motivation

Daily notes already carry ambient state as frontmatter: `cannabis: false`, `water: null`. The properties exist, the template owns them, and Obsidian renders them — but *logging* one means opening the app, finding today's note, and hand-editing YAML. That is exactly the friction [[the_pour_manifesto|the manifesto]] exists to kill: if logging a glass of water takes longer than drinking it, the log dies.

`pour habit water 16` should be a reflex.

This spec deliberately __extracts the pour from the habit__. Pour ships no habit-specific code — the pattern decomposes into four general primitives, each useful beyond this module (the same discipline that kept [[pour-roots-and-hooks|the roots-and-hooks work]] free of module-specific code):

1. A third write mode — __`update`__ — that mutates frontmatter keys on an *existing* note (§2)
2. Two field types — __`toggle`__ and __`counter`__ — for ambient state (§3)
3. __Date-target resolution__ — a rollover boundary and an explicit date override (§4)
4. __One-shot argv capture__ — field + value from the command line, no TUI (§5)

The habit module itself (§6) is then a plain config block. See [[the_habit_story]] for the narrative.

---

## 1. The Creed: Frontmatter Habits vs. Event Notes

> [!quote] Ambient state gets a property. Novel experience gets a note. Never both for the same signal.

Pour now has two capture shapes, and the line between them is a __lasting rule__, inscribed here so no future module blurs it:

- __Frontmatter habit__ — transient, ambient, binary-or-cumulative state of a *day*: partaken or not, ounces so far. It has no story to tell beyond its value. It lives as a property on the periodic note. The __template owns the key and its default__ (`cannabis: false`, `water: null`); pour only ever *mutates*.
- __Event note__ — anything with novelty or nuance: a coffee has a bean, a ratio, a taste worth remembering. It gets its own note via `create` mode, as today.

Corollary: if a signal could be *derived* from event notes (coffee count from coffee notes), __derive it — never mirror it into frontmatter__. Two sources of truth drift by week one. A frontmatter habit is only for signals that have no event notes.

This rule also belongs in [[field-types]] when the field types land.

## 2. New Write Mode: `update`

```rust
// src/config.rs
pub enum WriteMode { Append, Create, Update }
```

`update` resolves `path` (strftime-templated, like `append`) to an __existing__ note and merges only the frontmatter keys named by the module's fields. Body untouched. Nothing else in the file is rewritten.

### 2.1 Transport: API Path

Obsidian Local REST API __v3.0__ supports patching a single frontmatter field by name — no YAML handling on our side at all; Obsidian's own metadata layer does the mutation:

```
PATCH /vault/{path}
Operation: replace
Target-Type: frontmatter
Target: water
Content-Type: application/json

64
```

One PATCH per mutated key. Reference: [PATCH v2→v3 changes](https://github.com/coddingtonbear/obsidian-local-rest-api/wiki/Changes-to-PATCH-requests-between-versions-2.0-and-3.0). Verify the installed plugin version at connect time; a v2 plugin should degrade to the fs path (§2.2), not fail.

### 2.2 Transport: Fs Fallback — the Invariant Holds

The scary scenario — pour rewriting a file Obsidian holds open in an editor — and the fs-fallback scenario are __nearly mutually exclusive__: the fs path fires when the API is unreachable, and the API being unreachable usually means Obsidian isn't running. The dangerous quadrant (Obsidian open, plugin disabled/broken) is narrow, and a guarded write shrinks it to a sliver:

1. __Read__ the file; note its mtime.
2. __Surgical single-key edit__ — locate the key's line inside the frontmatter block and replace *only that line* (insert into the block if absent, §2.4). No YAML re-emit: key order, quoting style, and every untouched line survive byte-for-byte. This is a line-level edit, not a parse–serialize round-trip.
3. __Verify__ mtime is unchanged since the read; abort loudly on mismatch.
4. __`atomic_replace`__ — the temp-file-and-rename path that already exists in `src/transport/fs.rs`.

*[Deviation: __the stat comes before the read, not after.__ As drafted, step 1 reads and then notes the mtime, which leaves a window where an edit landing between the read and the stat goes undetected by the very check meant to catch it. The Inspector blocked on this in round 1. Shipped order: stat, read, edit, re-stat, replace, in `FsWriter::patch_frontmatter_since`, with a deterministic race test that pushes the mtime forward between the baseline and the read. The guard is a mitigation, not a lock; a sub-millisecond window survives by design.]*

*[Deviation: __a multi-key update is not atomic across keys.__ The patch operation is single-key on both transports, so a two-field submit is two independent writes. Every value-level error is raised while planning, before the first write; a transport failure mid-sequence returns an error naming the keys that already landed. Batching the fs path was declined for v1 because it would diverge the two transports. Candidate for v1.1.]*

So the invariant — *the fs fallback always works* — is __kept__, not broken. [[the_pour_manifesto|"Plaintext is Forever"]] and [[pour_without_obsidian]] both demand it: a module class that only functions with a plugin alive would betray the portability story.

__Reading__, by contrast, needs a real (read-only) frontmatter parser: `counter` increments and progress display must interpret existing values. Parsing is safe — it's *re-emitting* that destroys formatting, and we never re-emit.

*(Investigated and rejected as a third transport: the official Obsidian CLI, GA since v1.12.4. It is a remote control for the running desktop app — the same availability constraint as the REST API — and `obsidian-headless` is sync-only. Nothing there changes this design; revisit if the CLI grows a daemon mode.)*

### 2.3 Missing Note

`pour habit water 16` at 9am, before today's note exists. Pour must __never fabricate a daily note__ — the template (Daily Notes/Templater) owns that shape, and a bare file with three keys poisons the note the template would have generated.

- __API path:__ fire the daily-notes create command via the existing `execute_command` transport method, then retry the read once. The template applies; the capture lands.
- __fs path:__ fail loudly with a clear message (`today's note doesn't exist yet — open Obsidian or create it first`). Never silently drop, never silently create.

### 2.4 Missing Key (Note Exists)

Template drift: the note exists but the key isn't in it. __Write the key into the frontmatter block anyway and surface a one-line notice.__ Capture-first: refusing to log because the template was stale is administrative friction of exactly the kind the manifesto kills. The notice tells the user their template needs the key added.

*[Deviation: __a note with no frontmatter block at all fails loudly on both transports.__ Neither this section nor §2.3 covers it. §2.4 argues capture-first, §2.3 argues pour never restructures a note; the Architect took §2.3's side so that behavior is uniform across transports. Five-line change if the other reading wins.]*

## 3. New Field Types: `toggle` and `counter`

```rust
// src/config.rs
pub enum FieldType { Text, Textarea, Number, StaticSelect, DynamicSelect, CompositeArray, Toggle, Counter }
```

Both default `target = "frontmatter"`; both are valid in any write mode (a `create` module may declare a `toggle` too — general primitives, not habit-only).

### 3.1 `toggle`

A bool. TUI: space flips it. One-shot (§5): bare field name sets `true`; explicit `false`/`off` sets false (the correction path).

*[Deviation: __a value pour cannot read is left alone, and the form shows `[?]`.__ Round 1 coerced anything unparseable (`cannabis: maybe`) to `false` and re-persisted it on submit, which silently rewrote a byte the user owned. Shipped: `normalise_toggle` returns `None` for such values, the planner skips the key, and the renderer shows a third state rather than an empty box that asserts a `false` pour does not know.]*

### 3.2 `counter`

A number that __accumulates__. New config keys:

```toml
[[modules.habit.fields]]
name = "water"
field_type = "counter"
prompt = "Water"
unit = "oz"        # display only, never written to YAML
goal = 96          # reach-target; progress renders as 64/96 oz
# limit = 3        # stay-under target — RESERVED, v2 (§7). Distinct key from
# limit_period = "week"  # goal by design: they are inverses and must not conflate.
```

__Semantics — increment by default:__

- `pour habit water 16` → read current, __add__ 16, write. Missing/`null` reads as 0 — so the template keeps `water: null` ("untouched today"), usefully distinct from an explicit `0`.
- `pour habit water =160` → __set__, the fat-finger correction.
- Values parse as numbers; integral results emit without a decimal point (matching `number` field emission).

__Every write echoes the resulting state__ — `water: 64/96 oz` — which is simultaneously the confirmation, the correction prompt, and the in-the-loop progress display. Goals live in __config only__; pour never writes a goal into the vault. Obsidian-side progress rendering (Bases/Dataview) mirrors the goal in its own query and is out of scope.

## 4. Date-target Resolution

Two separate mechanisms, deliberately not merged:

*[Not built as of 2026-10-02: neither `rollover` nor `--date` exists in the code. A `rollover` key in config is not rejected, it is silently ignored, because config has no `deny_unknown_fields`. Tracked as v1.1 below.]*

### 4.1 `rollover` — The 4am Boundary

```toml
[modules.habit]
rollover = "04:00"   # until 4am, "today" is yesterday
```

Module-level key shifting the date used for strftime `path` resolution (and the `{{date}}` token). A capture at 12:40am belongs to the evening it ends — for this module's founding property, the *most common* logging time. General by construction: `[modules.me]` wants this exact key for 1am journal entries. Per-module for v1; promote to `[vault]` with per-module override if it repeats (the `base_path` precedent — promote at the third use, not the first).

### 4.2 Explicit Date Override

`pour habit cannabis --date yesterday` / `--date 2026-08-04` — deliberate backfill, one invocation. Distinct from rollover (a standing policy) and never persisted. v1 scope: one-shot flag only; a TUI affordance can follow if reached for.

## 5. One-shot Argv Capture

```
pour <module> <field> [value]      # no TUI, write, echo result, exit
```

The manifesto's velocity ethos made literal — the whole interaction is one shell line:

```
$ pour habit water 16
water: 64/96 oz · ✓ 20260805.md

$ pour habit cannabis
cannabis: true · ✓ 20260805.md
```

- Resolution: `args[2]` matches a field `name` in the module; remaining arg parses per that field's type (`toggle`: absent→`true`, `false`/`off`→false; `counter`: `N` increments, `=N` sets).
- `pour habit` with no field args → TUI form, exactly as `pour <module>` behaves today. No behavior change for existing modules.
- Generalizes beyond `update` modules in principle, but __v1 wires it for `toggle`/`counter` fields only__ — text-bearing one-shots (`pour me "thought"`) raise quoting/required-field questions that deserve their own spec pass.
- The `serve`/PWA surface flows through the normal submit handler; an `update`-mode module is a module like any other to the server. One-shot is a CLI affordance, not a new API.

*[Deviation: the grammar lives in `src/oneshot.rs`, not `main.rs`, so the parse layer is testable from the integration suite. `pour <module>` with no field argument parses to `None` and falls through to the TUI unchanged. The server side gained `invalid_toggle` / `invalid_counter` wire codes in `validate.rs`, reusing the same parsers the writer uses so the two cannot drift; see [[pour-api-contract]].]*

*[Deviation: an `update` module __rejects at config load__ any field that would be silently inert on it: `composite_array`, `list = true`, body-targeted fields, and `daily_link`. Better a validation error than a key that never writes.]*

## 6. The `habit` Module (Mads Preset)

A plain config block — no new concepts beyond the primitives above:

```toml
[modules.habit]
mode = "update"
path = "06 - Periodic/00 - Daily/%Y%m%d.md"
display_name = "Habits"
icon = "🌱"
rollover = "04:00"

[[modules.habit.fields]]
name = "cannabis"
field_type = "toggle"
prompt = "Partaken?"

[[modules.habit.fields]]
name = "water"
field_type = "counter"
prompt = "Water"
unit = "oz"
goal = 96
```

*[Deviation: the shipped preset has no `rollover` line, because §4.1 is not built.]*

The daily-note template owns the keys and defaults (`cannabis: false`, `water: null`). Same seed-not-mirror caveat as every personal preset ([[pour-roots-and-hooks|roots-and-hooks follow-ups]]): lands in `resources/mads_config.toml`, hand-added to the live stowed config.

*[Deviation: __the field name is the frontmatter key__; pour does not map one to the other. The daily template tracked `water_oz` at the time, and the first live run wrote a stray `water: 15` beside it. Rule now recorded in [[Pour-Types]]: match the template, or change the template. Settled by changing the template — it tracks `water:` as of 2026-08-06, and the module's field is `water` to match. The preset also sets `mobile_visible = false`; the PWA has no toggle/counter widgets yet.]*

## 7. V2 — Periodic Limits

The founding story's real requirement: *"you've already partaken enough this week (3) — come back Fri, Aug 8."*

A `limit` + `limit_period = "week"` on a `counter`/`toggle` field aggregates the current period before writing. This stays tractable — __not__ a query engine — because periodic-note paths are *computable from the date format*: "this week" is ≤7 deterministic paths. Read them (the frontmatter parser from §2.2 already exists by then), sum, compare, message. No glob, no index, no Dataview.

Display is the inverse of `goal`: reach vs. stay-under, with the come-back date derived from the period boundary. Deferred, but the counter schema above already reserves the keys so v2 slots in without a schema break.

## 8. Data-model Sketch

```rust
// src/transport/mod.rs — one new method, dispatching per transport
pub async fn patch_frontmatter(&self, vault_path: &str, key: &str, value: &FrontmatterValue) -> Result<()>;
//   Api → PATCH Target-Type: frontmatter (§2.1); v2 plugin → fall through to fs
//   Fs  → guarded surgical edit (§2.2): read → mtime → line-edit → atomic_replace

// src/output/frontmatter.rs (or sibling) — read-only parse + line-level edit
pub fn read_frontmatter(content: &str) -> Option<BTreeMap<String, String>>;
pub fn patch_frontmatter_line(content: &str, key: &str, value: &str) -> PatchOutcome; // Replaced | Inserted

// src/server/handlers/submit/ — write_update alongside write_create / write_append

// src/main.rs — one-shot dispatch: `pour <module> <field> [value]` before the TUI branch
```

*[Deviation: `write_update` lives in `src/output/update.rs` beside `write_create` and `write_append`, not under the server handlers. The TUI, the one-shot path and `/api/v1/submit` all call it. `patch_frontmatter_line` returns `Result<(String, PatchOutcome), PatchLineError>`, so it can refuse a key whose value spans several lines.]*

## Scope & Phasing

- [x] __v1 — the capture loop.__ `update` mode (both transports, §2), `toggle` + `counter` with `goal` (§3), one-shot argv (§5), the mads `habit` preset (§6). Docs: [[field-types]] (new types + keys + the creed), System-Architecture-Overview (transport method + write path), README. *[Shipped 2026-08-06, cycle `habit-capture-v1`: one Inspector blocker and three majors in round 1, APPROVE in round 2, 1051 tests. Smoke-tested live the same day; the API-path §2.3 create-and-retry is still unexercised.]*
- [ ] __v1.1 — date targeting.__ `rollover` (§4.1) + `--date` (§4.2).
- [ ] __v2 — periodic limits__ (§7): `limit` + `limit_period`, cross-note aggregation, come-back messaging.

## Open Questions

- __Plugin version detection.__ How does pour learn the Local REST API is v3-capable — probe response header, or attempt PATCH and fall back on 4xx? *Lean: attempt-and-fall-back; one less handshake.* *[Shipped as leaned: a PATCH answered with 400, 405, 415 or 501 degrades to the filesystem path (`classify_patch_status`). A 404 means the note is missing.]*
- __TUI shape for `update` modules.__ Does `pour habit` (no args) show current values fetched at form-open (read-before-render), and is stale-cache display acceptable on slow reads? *Lean: read at open over transport; this module's read is one file.* *[Shipped as leaned: one read at form open. A failed or slow read shows a placeholder and does not block the form.]*
- __`toggle` false in one-shot.__ `pour habit cannabis false` vs `--off` vs both? *Lean: accept `false`/`off` as the value token; no flag.* *[Shipped as leaned, and `no`/`0` also clear it.]*
- __Counter floats.__ `water 12.5` — allowed? *Lean: yes; parse f64, emit integers bare (matches `number`).* *[Shipped as leaned.]*
