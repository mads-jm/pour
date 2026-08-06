---
tags:
  - spec
  - modules
  - habits
  - frontmatter
date created: Wednesday, August 5th 2026
date modified: Wednesday, August 5th 2026
---

# Pour habit capture — frontmatter mutation primitives — Spec

## Motivation

Daily notes already carry ambient state as frontmatter: `cannabis: false`, `water: null`. The properties exist, the template owns them, and Obsidian renders them — but *logging* one means opening the app, finding today's note, and hand-editing YAML. That is exactly the friction [[the_pour_manifesto|the manifesto]] exists to kill: if logging a glass of water takes longer than drinking it, the log dies.

`pour habit water 16` should be a reflex.

This spec deliberately **extracts the pour from the habit**. Pour ships no habit-specific code — the pattern decomposes into four general primitives, each useful beyond this module (the same discipline that kept [[pour-roots-and-hooks|the roots-and-hooks work]] free of module-specific code):

1. A third write mode — **`update`** — that mutates frontmatter keys on an *existing* note (§2)
2. Two field types — **`toggle`** and **`counter`** — for ambient state (§3)
3. **Date-target resolution** — a rollover boundary and an explicit date override (§4)
4. **One-shot argv capture** — field + value from the command line, no TUI (§5)

The habit module itself (§6) is then a plain config block. See [[the_habit_story]] for the narrative.

---

## 1. The creed: frontmatter habits vs. event notes

> [!quote] Ambient state gets a property. Novel experience gets a note. Never both for the same signal.

Pour now has two capture shapes, and the line between them is a **lasting rule**, inscribed here so no future module blurs it:

- **Frontmatter habit** — transient, ambient, binary-or-cumulative state of a *day*: partaken or not, ounces so far. It has no story to tell beyond its value. It lives as a property on the periodic note. The **template owns the key and its default** (`cannabis: false`, `water: null`); pour only ever *mutates*.
- **Event note** — anything with novelty or nuance: a coffee has a bean, a ratio, a taste worth remembering. It gets its own note via `create` mode, as today.

Corollary: if a signal could be *derived* from event notes (coffee count from coffee notes), **derive it — never mirror it into frontmatter**. Two sources of truth drift by week one. A frontmatter habit is only for signals that have no event notes.

This rule also belongs in [[field-types]] when the field types land.

## 2. New write mode: `update`

```rust
// src/config.rs
pub enum WriteMode { Append, Create, Update }
```

`update` resolves `path` (strftime-templated, like `append`) to an **existing** note and merges only the frontmatter keys named by the module's fields. Body untouched. Nothing else in the file is rewritten.

### 2.1 Transport: API path

Obsidian Local REST API **v3.0** supports patching a single frontmatter field by name — no YAML handling on our side at all; Obsidian's own metadata layer does the mutation:

```
PATCH /vault/{path}
Operation: replace
Target-Type: frontmatter
Target: water
Content-Type: application/json

64
```

One PATCH per mutated key. Reference: [PATCH v2→v3 changes](https://github.com/coddingtonbear/obsidian-local-rest-api/wiki/Changes-to-PATCH-requests-between-versions-2.0-and-3.0). Verify the installed plugin version at connect time; a v2 plugin should degrade to the fs path (§2.2), not fail.

### 2.2 Transport: fs fallback — the invariant holds

The scary scenario — pour rewriting a file Obsidian holds open in an editor — and the fs-fallback scenario are **nearly mutually exclusive**: the fs path fires when the API is unreachable, and the API being unreachable usually means Obsidian isn't running. The dangerous quadrant (Obsidian open, plugin disabled/broken) is narrow, and a guarded write shrinks it to a sliver:

1. **Read** the file; note its mtime.
2. **Surgical single-key edit** — locate the key's line inside the frontmatter block and replace *only that line* (insert into the block if absent, §2.4). No YAML re-emit: key order, quoting style, and every untouched line survive byte-for-byte. This is a line-level edit, not a parse–serialize round-trip.
3. **Verify** mtime is unchanged since the read; abort loudly on mismatch.
4. **`atomic_replace`** — the temp-file-and-rename path that already exists in `src/transport/fs.rs`.

So the invariant — *the fs fallback always works* — is **kept**, not broken. [[the_pour_manifesto|"Plaintext is Forever"]] and [[pour_without_obsidian]] both demand it: a module class that only functions with a plugin alive would betray the portability story.

**Reading**, by contrast, needs a real (read-only) frontmatter parser: `counter` increments and progress display must interpret existing values. Parsing is safe — it's *re-emitting* that destroys formatting, and we never re-emit.

*(Investigated and rejected as a third transport: the official Obsidian CLI, GA since v1.12.4. It is a remote control for the running desktop app — the same availability constraint as the REST API — and `obsidian-headless` is sync-only. Nothing there changes this design; revisit if the CLI grows a daemon mode.)*

### 2.3 Missing note

`pour habit water 16` at 9am, before today's note exists. Pour must **never fabricate a daily note** — the template (Daily Notes/Templater) owns that shape, and a bare file with three keys poisons the note the template would have generated.

- **API path:** fire the daily-notes create command via the existing `execute_command` transport method, then retry the read once. The template applies; the capture lands.
- **fs path:** fail loudly with a clear message (`today's note doesn't exist yet — open Obsidian or create it first`). Never silently drop, never silently create.

### 2.4 Missing key (note exists)

Template drift: the note exists but the key isn't in it. **Write the key into the frontmatter block anyway and surface a one-line notice.** Capture-first: refusing to log because the template was stale is administrative friction of exactly the kind the manifesto kills. The notice tells the user their template needs the key added.

## 3. New field types: `toggle` and `counter`

```rust
// src/config.rs
pub enum FieldType { Text, Textarea, Number, StaticSelect, DynamicSelect, CompositeArray, Toggle, Counter }
```

Both default `target = "frontmatter"`; both are valid in any write mode (a `create` module may declare a `toggle` too — general primitives, not habit-only).

### 3.1 `toggle`

A bool. TUI: space flips it. One-shot (§5): bare field name sets `true`; explicit `false`/`off` sets false (the correction path).

### 3.2 `counter`

A number that **accumulates**. New config keys:

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

**Semantics — increment by default:**

- `pour habit water 16` → read current, **add** 16, write. Missing/`null` reads as 0 — so the template keeps `water: null` ("untouched today"), usefully distinct from an explicit `0`.
- `pour habit water =160` → **set**, the fat-finger correction.
- Values parse as numbers; integral results emit without a decimal point (matching `number` field emission).

**Every write echoes the resulting state** — `water: 64/96 oz` — which is simultaneously the confirmation, the correction prompt, and the in-the-loop progress display. Goals live in **config only**; pour never writes a goal into the vault. Obsidian-side progress rendering (Bases/Dataview) mirrors the goal in its own query and is out of scope.

## 4. Date-target resolution

Two separate mechanisms, deliberately not merged:

### 4.1 `rollover` — the 4am boundary

```toml
[modules.habit]
rollover = "04:00"   # until 4am, "today" is yesterday
```

Module-level key shifting the date used for strftime `path` resolution (and the `{{date}}` token). A capture at 12:40am belongs to the evening it ends — for this module's founding property, the *most common* logging time. General by construction: `[modules.me]` wants this exact key for 1am journal entries. Per-module for v1; promote to `[vault]` with per-module override if it repeats (the `base_path` precedent — promote at the third use, not the first).

### 4.2 Explicit date override

`pour habit cannabis --date yesterday` / `--date 2026-08-04` — deliberate backfill, one invocation. Distinct from rollover (a standing policy) and never persisted. v1 scope: one-shot flag only; a TUI affordance can follow if reached for.

## 5. One-shot argv capture

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
- Generalizes beyond `update` modules in principle, but **v1 wires it for `toggle`/`counter` fields only** — text-bearing one-shots (`pour me "thought"`) raise quoting/required-field questions that deserve their own spec pass.
- The `serve`/PWA surface flows through the normal submit handler; an `update`-mode module is a module like any other to the server. One-shot is a CLI affordance, not a new API.

## 6. The `habit` module (mads preset)

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

The daily-note template owns the keys and defaults (`cannabis: false`, `water: null`). Same seed-not-mirror caveat as every personal preset ([[pour-roots-and-hooks|roots-and-hooks follow-ups]]): lands in `resources/mads_config.toml`, hand-added to the live stowed config.

## 7. v2 — periodic limits

The founding story's real requirement: *"you've already partaken enough this week (3) — come back Fri, Aug 8."*

A `limit` + `limit_period = "week"` on a `counter`/`toggle` field aggregates the current period before writing. This stays tractable — **not** a query engine — because periodic-note paths are *computable from the date format*: "this week" is ≤7 deterministic paths. Read them (the frontmatter parser from §2.2 already exists by then), sum, compare, message. No glob, no index, no Dataview.

Display is the inverse of `goal`: reach vs. stay-under, with the come-back date derived from the period boundary. Deferred, but the counter schema above already reserves the keys so v2 slots in without a schema break.

## 8. Data-model sketch

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

## Scope & phasing

- [x] **v1 — the capture loop.** `update` mode (both transports, §2), `toggle` + `counter` with `goal` (§3), one-shot argv (§5), the mads `habit` preset (§6). Docs: [[field-types]] (new types + keys + the creed), System-Architecture-Overview (transport method + write path), README.
- [ ] **v1.1 — date targeting.** `rollover` (§4.1) + `--date` (§4.2).
- [ ] **v2 — periodic limits** (§7): `limit` + `limit_period`, cross-note aggregation, come-back messaging.

## Open questions

- **Plugin version detection.** How does pour learn the Local REST API is v3-capable — probe response header, or attempt PATCH and fall back on 4xx? *Lean: attempt-and-fall-back; one less handshake.*
- **TUI shape for `update` modules.** Does `pour habit` (no args) show current values fetched at form-open (read-before-render), and is stale-cache display acceptable on slow reads? *Lean: read at open over transport; this module's read is one file.*
- **`toggle` false in one-shot.** `pour habit cannabis false` vs `--off` vs both? *Lean: accept `false`/`off` as the value token; no flag.*
- **Counter floats.** `water 12.5` — allowed? *Lean: yes; parse f64, emit integers bare (matches `number`).*
