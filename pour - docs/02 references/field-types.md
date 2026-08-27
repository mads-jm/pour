---
tags:
  - reference
  - config
  - fields
date created: Wednesday, April 1st 2026, 10:49:25 pm
date modified: Monday, May 4th 2026, 11:17:48 pm
---

# Field Types Reference

This document covers all field types available in Pour's `config.toml` schema, their config keys, validation rules, default output targets, and TUI rendering behavior.

## The creed: frontmatter habits vs. event notes

> [!quote] Ambient state gets a property. Novel experience gets a note. Never both for the same signal.

Pour has two capture shapes, and the line between them is a lasting rule:

- **Frontmatter habit** — transient, ambient, binary-or-cumulative state of a *day*: partaken or not, ounces so far. It has no story to tell beyond its value, so it lives as a property on a periodic note. The **template owns the key and its default** (`cannabis: false`, `water: null`); pour only ever *mutates* it, via `mode = "update"` with `toggle`/`counter` fields.
- **Event note** — anything with novelty or nuance: a coffee has a bean, a ratio, a taste worth remembering. It gets its own note via `create` mode.

Corollary: if a signal could be *derived* from event notes (coffee count from coffee notes), **derive it — never mirror it into frontmatter.** Two sources of truth drift by week one. A frontmatter habit is only for signals that have no event notes.

See [[pour-habit-capture]] for the full spec.

## Module Config Keys

Every module defined in `[modules.<name>]` supports these keys:

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `mode` | string | yes | `"create"` (new file), `"append"` (add to existing file), or `"update"` (mutate frontmatter on an existing file) |
| `path` | string | yes | Vault-relative path template. Supports strftime tokens (`%Y`, `%m`, `%d`) and field placeholders (`{{field_name}}`). |
| `display_name` | string | no | Human-readable label shown on the dashboard. Defaults to the module key. |
| `append_under_header` | string | conditional | Required for `append` mode. Markdown heading to insert content under. |
| `append_template` | string | no | Template string for append-mode output. Supports `{{field}}`, `{{date}}`, `{{time}}`, `{{callout}}` placeholders. |
| `callout_type` | string | no | Default Obsidian callout type for `{{callout}}` in templates. |
| `icon` | string | no | Optional icon displayed on the TUI dashboard next to the module name (e.g. `"☕"`). For create-mode modules, also written to output frontmatter as `icon: <value>`, making it queryable by Dataview and compatible with Iconize/Supercharged Links. |
| `preset_axes` | string[] | no | Ordered list of field names used as drilldown axes in the preset picker. Empty/absent → no picker; the legacy `←→` cycler stays active. See [[pour-preset-hierarchy]]. |

## Field Config Keys

Every field in a module's `[[modules.<name>.fields]]` array supports these keys:

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | yes | Field identifier, used as the YAML frontmatter key |
| `field_type` | string | yes | One of the eight types below |
| `prompt` | string | yes | Label shown in the TUI form |
| `required` | bool | no | If `true`, submit is blocked when the field is empty |
| `default` | string | no | Pre-filled value on form init |
| `options` | string[] | conditional | Required for `static_select`; ignored otherwise |
| `source` | string | conditional | Required for `dynamic_select`; vault-relative directory path |
| `target` | string | no | `"frontmatter"` or `"body"` — overrides the default routing |
| `sub_fields` | array | conditional | Required for `composite_array`; column definitions |
| `callout` | string | no | Obsidian callout type (e.g. `"note"`, `"tip"`). When set on a `textarea` field targeting body, the output is wrapped in `> [!type]` blockquote syntax. |
| `callout_title` | string | no | Default title rendered on the callout line: `> [!type] <callout_title>`. Only used when `callout` is set. In the TUI, press `t` while focused on the textarea row (editor closed) to edit the title for the current entry — an empty title clears it. A `t title` hint appears in the footer bar when the hotkey is available. Bare `t` is used rather than `Ctrl+T` because some IDEs/terminals intercept Ctrl-letter chords before they reach the TUI. |
| `allow_create` | bool | no | Only valid on `dynamic_select`. When `true`, the user can type characters to filter options and enter a completely novel value if nothing matches. Defaults to `false` (closed list). |
| `wikilink` | bool | no | If `true`, wraps the output value in Obsidian wikilink syntax: `[[value]]`. Applies to `text`, `static_select`, and `dynamic_select` fields. No-ops if the value is already wrapped. Defaults to `false`. |
| `create_template` | string | no | Only valid on `dynamic_select` fields with `allow_create = true`. References a template name from `[templates.<name>]`. When set, typing a novel value opens a sub-form overlay to fill in the template's fields before creating the note. Without this key, novel values create a bare stub note. |
| `post_create_command` | string | no | Obsidian command ID to execute after template-driven note creation (e.g. `"templater:run"`). Only valid when `create_template` is set. Fires via the REST API `/commands/` endpoint; silently skipped on filesystem transport. |
| `show_when` | object | no | Conditional visibility rule. When present, the field is only rendered and navigable if the condition is satisfied. If the condition becomes false while the field is focused, focus moves to the nearest visible field. See __Conditional Visibility__ below. |
| `icon` | string | no | Optional icon displayed next to the field prompt in the TUI form (e.g. `"🫘"`). Purely cosmetic — not written to output. |
| `preset_exclude` | bool | no | When `true`, this field is excluded from preset capture and application. Useful for notes/textarea fields whose values change every entry and shouldn't be part of a saved preset. Defaults to `false`. |
| `unit` | string | no | Only valid on `counter`. Display-only suffix (e.g. `"oz"`) shown in the TUI and the one-shot echo. **Never written to YAML.** |
| `goal` | number | no | Only valid on `counter`. Reach-target rendered as `64/96`. **Config-only — pour never writes a goal into the vault.** Obsidian-side progress rendering (Bases/Dataview) mirrors it in its own query. |
| `list` | bool | no | When `true`, values containing `", "` are split on that delimiter and emitted as a YAML sequence in frontmatter (e.g. `"a, b"` → `- a\n- b`). When `wikilink = true` is also set, each split item is individually wrapped in `[[...]]`. Defaults to `false` — value is treated as a literal string and properly escaped. Valid on `text`, `static_select`, and `dynamic_select`. |

## Output Target Defaults

| Field Type | Default Target |
|------------|---------------|
| `text` | frontmatter |
| `number` | frontmatter |
| `static_select` | frontmatter |
| `dynamic_select` | frontmatter |
| `textarea` | body |
| `composite_array` | frontmatter |
| `toggle` | frontmatter |
| `counter` | frontmatter |

Any field can override its default via `target = "frontmatter"` or `target = "body"`.
`update`-mode modules are the exception: they write frontmatter and nothing else, so a body-targeted field there is a config error rather than a silently dropped value.

---

## [[Conditional-Visibility|Conditional Visibility]]

Any field can be conditionally shown using a `show_when` block. Hidden fields are skipped during rendering and navigation.

```toml
[[modules.brew.fields]]
name = "pressure"
field_type = "number"
prompt = "Pressure"
[modules.brew.fields.show_when]
field = "method"        # name of the controlling field
equals = "Espresso"     # show only when method == "Espresso"
```

Or using `one_of` to match multiple values:

```toml
[modules.brew.fields.show_when]
field = "method"
one_of = ["Espresso", "Moka"]
```

__Visibility rules:__
- `equals`: visible if `field_values[field] == equals` (case-sensitive).
- `one_of`: visible if `field_values[field]` matches any listed value (case-sensitive).
- If the controlling field is absent or empty, the conditional field is hidden.

__Submit behavior:__
- Hidden fields are skipped during validation — a hidden `required` field does not block submit.
- Hidden field values are cleared on submit, so no stale data appears in output.
- Hidden fields are excluded from frontmatter, body, and template placeholder resolution. Template placeholders for hidden fields resolve to empty string.

__Navigation behavior:__
- Tab/Shift-Tab/Up/Down bounds are computed from the *visible* field set, not total field count.
- If a field becomes hidden while focused (e.g. the user changes a controlling field), focus moves to the next visible field, then previous, then the submit button.
- New fields becoming visible do __not__ steal focus.

__Config validation rules:__
- Exactly one of `equals` or `one_of` must be specified — not both, not neither.
- `equals` must not be an empty string.
- `one_of` must not be an empty array.
- `show_when.field` must reference an existing field in the same module.
- A field cannot reference itself.
- A field cannot reference a `composite_array` field as the controller.
- Circular dependencies are rejected (A→B→A, or longer chains).
- Forward references (referencing a field defined later in the array) are allowed.

__Limitations (v1):__
- `show_when` is not supported on `composite_array` sub-fields.
- Only a single condition per field — no AND/OR combinators.
- No negation operators (`not_equals`, `none_of`).
- Matching is case-sensitive only.

---

## `text`

Single-line free text input.

```toml
[[modules.coffee.fields]]
name = "origin"
field_type = "text"
prompt = "Bean origin"
```

__TUI__: Inline text input with cursor. Accepts any characters.
__Output__: Value written as-is to frontmatter (or body if overridden). If `wikilink = true`, the value is wrapped in `[[...]]` before output.

## `textarea`

Multi-line text input with an editor overlay.

```toml
[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "What's on your mind?"
target = "body"
```

__TUI__: Opens a bordered overlay editor on Enter. Supports multi-line editing. Escape closes the overlay.
__Output__: Defaults to Markdown body. Can be overridden to frontmatter.
__Callout wrapping__: When `callout = "note"` (or any Obsidian callout type) is set, the body output is automatically wrapped in blockquote callout syntax. This applies in both create mode (`partition_fields`) and append mode (template `{{field}}` substitution).

```toml
[[modules.me.fields]]
name = "notes"
field_type = "textarea"
prompt = "Notes"
callout = "tip"
```

Produces:

```markdown
> [!tip]
> First line of content
> Second line
```

__Runtime cycling__: When a textarea field has `callout` configured, Left/Right arrow keys cycle through callout types while the editor overlay is closed. The `[!type]` label is shown on the field row. The selected type overrides the config default for that entry only.

Available callout types: `note`, `info`, `todo`, `tip`, `success`, `question`, `warning`, `failure`, `danger`, `bug`, `example`, `quote`.

## `number`

Numeric input. Restricts keyboard input to digits, `.`, and `-`.

```toml
[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating (1-5)"
default = "3"
```

__TUI__: Inline text input, filtered to numeric characters only.
__Output__: Written to frontmatter as an unquoted YAML number (if parseable as integer or float). Falls back to quoted string if the value contains non-numeric content.
__Validation__: Non-numeric characters are rejected at input time, not at submit time.

## `static_select`

Dropdown with hardcoded options defined in config.

```toml
[[modules.coffee.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
options = ["V60", "AeroPress", "Espresso", "French Press"]
```

__TUI__: Enter toggles a dropdown overlay. Up/Down cycles options while open. Enter again confirms selection. The selected value is shown inline when the dropdown is closed.
__Output__: Selected string written to frontmatter. If `wikilink = true`, the value is wrapped in `[[...]]` before output (e.g. `roaster: "[[Onyx]]"`), creating an Obsidian backlink to the named note.
__Extensible options__: With `allow_create = true`, a novel value typed into the open dropdown is accepted on Enter, appended to the in-memory options list, and persisted back to the field's `options` array in `config.toml`. Fields without `allow_create` remain locked to the configured options.
__Validation__: `options` must be present and non-empty. Config load fails otherwise. `allow_create` is only valid on `static_select` and `dynamic_select` fields.

## `dynamic_select`

Dropdown populated from vault directory contents at runtime via [[The-3-Tier-Data-Fallback|the 3-tier fallback pipeline]]. With `allow_create = true`, novel values trigger [[Inline-Note-Creation|inline note creation]] back into the vault.

```toml
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"
```

__TUI__: Same dropdown interaction as `static_select`. Options are populated via the 3-tier fallback: API directory listing, filesystem scan, JSON cache (`~/.pour/cache/state.json`), then freetext input if all fail.

When `allow_create = true`, the user can type characters directly into the field to filter the dropdown options (case-insensitive substring match). If typing produces no matching options, `Enter` accepts the typed text as a novel value. `Backspace` trims the typed text. `Esc` clears the search buffer before closing the dropdown. Navigating away (Tab/Shift-Tab) discards any unsaved search text.

__Output__: Selected (or typed) string written to frontmatter. If `wikilink = true`, the value is wrapped in `[[...]]` before output.
__Validation__: `source` must be present and must be a vault-relative path (no absolute, drive-qualified, UNC, or `..` traversal paths). Config load fails otherwise. `allow_create` is only valid on `dynamic_select`; using it on any other field type fails config validation.
__Source path__: Relative to the vault root. Example: `"Coffee/Beans"` resolves to `<vault_base_path>/Coffee/Beans/`.

### Auto-create Behavior

When `allow_create = true` and the submitted value is not in the existing options list (case-insensitive), Pour automatically creates a note at `{source}/{sanitized_value}.md` before writing the module output.

__Without `create_template`__ — a bare stub note is created:

```markdown
---
date: YYYY-MM-DD
---
```

__With `create_template`__ — a sub-form overlay appears in the TUI, prompting the user to fill in the template's fields. The created note gets full frontmatter from the template. See [[#Template-Driven Creation]] below.

The filename is sanitized: characters invalid on any platform (`:`  `?`  `*`  `<`  `>`  `|`  `"`  `\`  `/`) are replaced with `-`, consecutive dashes are collapsed, and Windows reserved device names (`CON`, `NUL`, `COM1`–`COM9`, etc.) are rejected. If the value sanitizes to an empty or reserved string, auto-creation is skipped silently.

The new entry is appended to the in-memory cache so the next dropdown opens with the value available immediately. Creation is best-effort — a transport failure is logged to stderr but does not block form submission.

### Combined Example (bare stub)

```toml
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"
allow_create = true
wikilink = true
```

With this config, selecting or typing `"Ethiopia Guji"` writes `bean: "[[Ethiopia Guji]]"` to frontmatter and, if the value is novel, creates `Coffee/Beans/Ethiopia Guji.md` with a `date` frontmatter entry.

### Template-Driven Creation

When `create_template` references a `[templates.<name>]` section, novel values trigger a __sub-form overlay__ instead of creating a bare stub. This lets you capture structured metadata for the new note without leaving the TUI.

#### Flow

1. User types a value that doesn't match any existing option.
2. The sub-form overlay appears with the template's fields (text, number, static_select).
3. User fills in the fields. `Tab`/`Shift+Tab` navigates, `Enter` on the submit button creates the note.
4. Pour writes the note with full YAML frontmatter: `date`, `name` (the typed value), and all template fields.
5. If `post_create_command` is set and the API is connected, the Obsidian command fires (e.g. Templater processes the new file to add body content).
6. The parent form field is populated with the new value.

`Esc` cancels the sub-form without creating anything. If the terminal is too small for the overlay (< 10 rows or < 30 cols), Pour falls back to bare stub creation.

#### Combined Example (template + Command hook)

```toml
# Field references the template
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "02 - Areas/204 - Cooking/Coffee/Beans"
allow_create = true
wikilink = true
create_template = "bean"
post_create_command = "templater:run"

# Template defines the sub-form fields and output path
[templates.bean]
path = "02 - Areas/204 - Cooking/Coffee/Beans/{{name}}.md"

[[templates.bean.fields]]
name = "roaster"
field_type = "text"
prompt = "Roaster"

[[templates.bean.fields]]
name = "origin"
field_type = "static_select"
prompt = "Origin"
options = ["Ethiopia", "Colombia", "Guatemala", "Kenya", "Brazil", "Yemen", "Blend"]

[[templates.bean.fields]]
name = "process"
field_type = "static_select"
prompt = "Process"
options = ["Washed", "Natural", "Honey", "Anaerobic", "Wet Hulled"]
default = "Washed"

[[templates.bean.fields]]
name = "roast_level"
field_type = "static_select"
prompt = "Roast level"
options = ["Light", "Light-Medium", "Medium", "Medium-Dark", "Dark"]
default = "Light"

[[templates.bean.fields]]
name = "bag_weight_g"
field_type = "number"
prompt = "Bag weight (g)"
default = "250"
```

Typing `"Ethiopia Guji"` opens the sub-form. After filling in roaster, origin, etc., Pour creates `Beans/Ethiopia Guji.md`:

```markdown
---
date: 2026-04-02
name: Ethiopia Guji
roaster: Onyx
origin: Ethiopia
process: Washed
roast_level: Light
bag_weight_g: 250
---
```

Then `post_create_command` fires `templater:run`, which can add body content (brew log table, tasting notes section, metadata) via an Obsidian Templater template.

## `toggle`

A boolean property. Valid in **any** write mode — a general primitive, not a habit-only type.

```toml
[[modules.habit.fields]]
name = "cannabis"
field_type = "toggle"
prompt = "Partaken?"
```

__TUI__: Renders as `[x]` / `[ ]`; **space** flips it, and the footer says so (`space flip`) whenever the row is focused. Typing does nothing — there is no text buffer. On an `update` module the checkbox is seeded from the note's current value when the form opens. If the note holds something that is *not* a boolean word (a hand-edit, a value from before the field existed), the field is left unseeded and renders `[?]`: pour will not guess `false` on your behalf, and an unflipped `[?]` is skipped on submit so the note's value survives.
__One-shot__: `pour habit cannabis` sets `true`; `pour habit cannabis false` (or `off`) clears it. Also accepts `on`/`yes`/`1` and `no`/`0`.
__Output__: A bare YAML boolean — `cannabis: true`, never `"true"`.

## `counter`

A number that **accumulates**. Valid in any write mode, but only meaningful over time on an `update` module.

```toml
[[modules.habit.fields]]
name = "water"
field_type = "counter"
prompt = "Water"
unit = "oz"        # display only, never written to YAML
goal = 96          # reach-target; progress renders as 64/96 oz
```

__Semantics__ (`update` mode):

- `pour habit water 16` → read the current value, **add** 16, write.
- `pour habit water =160` → **set** it. The fat-finger correction.
- A missing key, an empty value, and an explicit `null`/`~` all read as `0`, so a template's `water: null` ("untouched today") stays meaningfully distinct from an explicit `0` in the note.
- A non-numeric current value (`water: lots`) is an **error**, not a silent reset — incrementing it would destroy what is there.
- Values parse as `f64`; integral results emit bare (`64`, not `64.0`).
- A **blank** counter on a submitted form means "no change", not "set to zero".

__TUI__: Inline input filtered to digits, `.`, `-`, and `=`. The footer spells out which is which (`0-9 add`, `= set`) whenever the row is focused, since a bare number and an `=`-prefixed one do opposite things. On an `update` module the row also shows the note's current state — `now 64/96 oz` — read once when the form opens. A failed or slow read renders `now —/96 oz` rather than blocking the form.
__Echo__: every write echoes the resulting state, `water: 64/96 oz`, which is simultaneously the confirmation, the correction prompt, and the progress display.
__Output__: A bare YAML number.

> [!note] Reserved for v2
> `limit` and `limit_period` are reserved on `counter` for periodic stay-under targets (the inverse of `goal`). They are **not implemented** — nothing reads them today.

## `composite_array`

Tabular data entry with multiple columns (sub-fields). Renders as a YAML array of objects in frontmatter or a Markdown table in body.

```toml
[[modules.recipe.fields]]
name = "ingredients"
field_type = "composite_array"
prompt = "Ingredients"

[[modules.recipe.fields.sub_fields]]
name = "item"
field_type = "text"
prompt = "Item"

[[modules.recipe.fields.sub_fields]]
name = "amount"
field_type = "number"
prompt = "Amount"

[[modules.recipe.fields.sub_fields]]
name = "unit"
field_type = "static_select"
prompt = "Unit"
options = ["g", "ml", "oz", "cups", "tbsp", "tsp"]
```

__TUI__: Enter opens a bordered table editor overlay. Navigate cells with arrow keys. Tab advances to next cell. Enter adds a new row. Escape closes the overlay. Empty rows are stripped on output. Inside the overlay, `s` saves the current rows as a per-field preset, `l` opens a picker over saved presets, and `p` cycles through them in place — see "Per-field presets" below.
__Output (frontmatter)__: Serialized as a YAML array of objects. Number sub-fields are written as unquoted YAML numbers.

```yaml
ingredients:
  - item: "flour"
    amount: 200
    unit: "g"
  - item: "milk"
    amount: 250
    unit: "ml"
```

__Output (body)__: Rendered as a Markdown table.
__Validation__: `sub_fields` must be present and non-empty. Sub-field names must be unique. `static_select` sub-fields must have non-empty `options`.

### Per-field Presets

Composite-array fields support saved row-set presets so high-friction inputs (recipes, pressure profiles, ingredient lists) can be replayed with a single keypress. Presets are scoped per `module.field` — `coffee.recipe` and `coffee.pressure_profile` have independent lists.

__Storage__: `~/.pour/field_presets.json` (or `$POUR_HOME/field_presets.json`). User-curated, atomic-replaced on save, separate from module-level `presets.json`.

__Keys (inside the composite overlay)__:

| Key | Action |
|---|---|
| `s` | Save current rows as a named preset. Opens a name + description prompt. Empty editors are rejected with a status message. |
| `l` | Open the load picker. Up/Down navigate, Enter applies, `Ctrl+D` deletes, Esc cancels. |
| `p` | Quick-cycle to the next saved preset. Wraps; no modal. |

__Apply behaviour__: Always replaces all existing rows silently — no confirmation prompt. The composite overlay shows `preset: <name>` as a subtitle once a preset has been applied so the active selection is visible.

__Schema drift__: If `sub_fields` changed since the preset was saved (column added or removed), each saved row is right-padded with empty strings or truncated to match the current sub-field count on apply. The status line reads "preset shape adjusted to current schema" so the change is visible; the on-disk preset is not rewritten — re-save with `s` to clean it up.

### PWA Overlay Rendering

The PWA sub-form overlay (shipped in Phase 2, closed 2026-04-27) renders template fields using the same field-type contract defined in this document. __Overlay rendering does not change any field-type semantics__: `text` and `number` fields render as standard inputs; `static_select` fields render as inline-cycling controls (◂ ▸ chevrons) instead of a `<select>` element (PWA-only, no `<select>` in overlays per UX convention). The `dynamic_select` and `composite_array` types are not permitted as template fields (§7.4 already restricts templates to `text | number | static_select`). Output targets, required semantics, and default-value behavior are identical to the TUI sub-form overlay.

### Template Fields and `allow_create`

Template fields (`[[templates.<name>.fields]]`) support `allow_create = true` on `static_select` fields only. When set, typing a novel value in the sub-form is accepted; after the templated note is written successfully, the new value is appended to the template field's `options` array in `config.toml` so it appears next session. Without `allow_create`, the sub-form static_select remains locked to the configured cycle of options.

### Sub-field Types

Sub-fields support a restricted set of types — no nesting or dynamic data:

| Sub-field type | Description |
|---------------|-------------|
| `text` | Free text cell |
| `number` | Numeric cell (digits, `.`, `-` only) |
| `static_select` | Dropdown cell with `options` |

---

## Module-Level Config Keys

These keys are set on the module itself, not on individual fields:

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `mode` | string | yes | `"create"` (new file per entry), `"append"` (add to existing note), or `"update"` (mutate frontmatter keys on an existing note — see [[#`update` mode]]) |
| `path` | string | yes | Vault-relative output path. Supports strftime tokens: `%Y`, `%m`, `%d`, `%H`, `%M`, `%S` |
| `fields` | array | yes | At least one field definition |
| `display_name` | string | no | Human-readable name shown in the dashboard (defaults to module key) |
| `append_under_header` | string | conditional | Required when `mode = "append"`. The Markdown heading to append under |
| `append_template` | string | no | Template for append-mode content. Supports `{{time}}`, `{{date}}`, `{{callout}}`, and field name placeholders |
| `callout_type` | string | no | Obsidian callout type (e.g. `"note"`, `"tip"`). Resolved as `{{callout}}` in `append_template` |
| `icon` | string | no | Unicode emoji shown in the TUI dashboard and written to frontmatter in create-mode output |
| `daily_link` | boolean | no | When `true`, create-mode output includes a `daily` frontmatter key linking to today's daily note |
| `append_shallow` | boolean | no | When `true` (append mode only), treats any subsequent heading as a section boundary — prevents sub-headings from being absorbed into the append target |
| `mobile_visible` | boolean | no | When `false`, this module is hidden from the mobile PWA (`/api/v1/config` omits it entirely). Defaults to `true`. Togglable from the module configure screen. |
| `base_path` | string | no | Per-module root override. When set, this module's `path` resolves against it instead of `[vault].base_path`. **Absolute only — `~` is not expanded** (Pour expands it nowhere). Absent → the vault, exactly as before. See [[#Per-module root (`base_path`)]]. |
| `[modules.<name>.platform]` | table | no | Per-OS overrides for this module's `base_path`, keyed by `std::env::consts::OS` (`linux`, `macos`, `windows`). Mirrors `[vault.platform]`. |
| `[modules.<name>.frontmatter]` | table | no | Static key→value frontmatter merged into **create**-mode output. Arrays render as YAML block sequences; scalars as scalars. **No token interpolation** — values are emitted literally. See [[#Static module frontmatter]]. |
| `frontmatter_date_format` | string | no | strftime format for the auto-injected create-mode `date` key. Absent → `%Y-%m-%d`. Per-module only; there is no global equivalent. Create mode only. |
| `post_write_shell` | string | no | Shell command run after a successful write. Only the tokens in [[#`post_write_shell`]] may be interpolated; anything else is rejected at load. |
| `post_write_shell_on_serve` | boolean | no | Whether `post_write_shell` also fires for captures submitted over the LAN (`pour serve`). **Defaults to `false`.** Requires `post_write_shell`. |

## `update` mode

`update` resolves `path` (strftime-templated, like `append`) to an **existing** note and merges only the frontmatter keys named by the module's fields. The body is never touched, key order and quoting survive byte-for-byte, and no key outside the field list is rewritten.

```toml
[modules.habit]
mode = "update"
path = "06 - Periodic/00 - Daily/%Y%m%d.md"
display_name = "Habits"
icon = "🌱"

[[modules.habit.fields]]
name = "cannabis"
field_type = "toggle"
prompt = "Partaken?"
```

__Transports.__ Over the API, one `PATCH /vault/{path}` per key with `Operation: replace`, `Target-Type: frontmatter`, `Target: <key>` — Obsidian's own metadata layer does the mutation, so pour never re-emits YAML. Over the filesystem it is a guarded surgical edit: capture mtime → read → replace exactly one line → re-verify mtime → atomic rename. The stat comes **before** the read deliberately, so a concurrent save that lands while pour is reading is caught as well. A mismatch aborts **without writing**. An API too old to serve the PATCH degrades to the filesystem path rather than failing the capture.

__Multi-key writes are not atomic.__ The patch operation is single-key on both transports, so a module with two live fields writes two keys in sequence. Every *value* error (a bad token, a non-numeric current value) is raised before the first byte is written; only a transport failure — a concurrent save landing between the two patches, or I/O — can strike mid-write. When one does, the keys that already landed stay landed and the error names them, so a retry is an informed choice rather than a blind double-count.

__Missing note.__ Pour **never fabricates a daily note** — the template owns that shape. Over the API it fires the `daily-notes` command and retries the read once; over the filesystem it fails loudly with an actionable message.

__Missing key.__ If the note exists but the key doesn't (a stale template), the key is inserted into the existing frontmatter block, the capture succeeds, and a one-line notice tells you the template needs updating. Capture-first: refusing to log because a template drifted is exactly the friction pour exists to kill.

__A note with no frontmatter block at all__ is an error on both transports. Pour mutates properties; it does not restructure notes.

__Keys rejected on `update` modules__, because they belong to another mode and silently ignoring them would be a trap: `append_under_header`, `append_template`, `append_shallow`, `daily_link`, `frontmatter_date_format`, `[modules.<n>.frontmatter]`, plus any field that is `composite_array`, `list = true`, or targets the body.

### One-shot capture

```
pour <module> <field> [value]      # no TUI, write, echo, exit 0
```

```
$ pour habit water 16
water: 64/96 oz · ✓ 20260805.md

$ pour habit cannabis
cannabis: true · ✓ 20260805.md
```

`pour <module>` with no field argument opens the TUI form exactly as before. Unknown module, unknown field, or a bad value token exits non-zero with an actionable message *before* the vault is touched. v1 wires the grammar for `toggle`/`counter` fields on `update` modules only; text-bearing one-shots (`pour me "thought"`) are deferred.

## Per-module root (`base_path`)

By default every module writes under `[vault].base_path`. A module that sets its own `base_path` writes somewhere else entirely — useful when a capture target is genuinely not part of the vault (e.g. an agent inbox kept in a different git repo).

Resolution order, first match wins:

```
[modules.<name>.platform][OS]  →  [modules.<name>].base_path  →  [vault.platform][OS]  →  [vault].base_path
```

```toml
[modules.inbox]
mode = "create"
base_path = "/home/user/notes-repo"    # absolute; `~` is NOT expanded
path = "inbox/%Y%m%d-%H%M%S{{slug}}.md"

# Optional: same logical root, different mount per machine.
[modules.inbox.platform]
windows = "C:\\Users\\user\\notes-repo"
```

Notes and limits:

- **`path` stays root-relative.** The override moves the root; it does not relax any guard. Absolute paths, drive letters, UNC paths, and `..` traversal are still rejected — now against the module's root.
- **Always filesystem transport.** The Obsidian Local REST API can only address notes inside the vault it serves, so a root-overriding module bypasses the API even when it is connected. The TUI summary and the `/api/v1/submit` response both report `FileSystem` for these captures — truthfully, not as a fallback.
- **Known limit:** a `dynamic_select` `source` on a root-overriding module still resolves against the **vault**, not the module root. Reads and auto-creates stay consistent with each other; only the module's own note is redirected.
- Windows-style and Unix-style absolute paths both validate on every OS — one `config.toml` is meant to describe every machine.

## Static module frontmatter

`[modules.<name>.frontmatter]` holds keys that are a property of the *module* rather than of the capture:

```toml
[modules.inbox.frontmatter]
tags = ["inbox", "capture"]
cssclasses = ["poured"]
author = "sam"
```

```yaml
---
date: 2026-07-16T14:32
kind: musing
author: sam
cssclasses:
  - poured
tags:
  - inbox
  - capture
---
```

- **Arrays always become block sequences**, even single-element ones. This is *not* the `list = true` path, which only splits a comma-joined string and would render `tags = ["inbox"]` as the scalar `tags: inbox`.
- **Values are literal.** There is no token interpolation here, deliberately — it is static config, not a template.
- **Emitted after captured fields**, in alphabetical key order (stable across runs).
- **The capture wins on collision.** A static whose key matches a captured field (or the injected `icon`/`daily`) is skipped — a static is a default, not an override.
- **Create mode only.** Append mode has no frontmatter block; setting it there is a validation error rather than a silent no-op.
- `date` is rejected as a static key — use `frontmatter_date_format` to change its shape.

> [!warning] TOML ordering
> Every plain module key (`path`, `post_write_shell`, …) must appear **before** `[modules.<name>.frontmatter]` and `[modules.<name>.platform]`. Once a sub-table header is opened, following bare keys belong to *it* — a `post_write_shell` written after `[modules.inbox.frontmatter]` silently becomes a frontmatter entry instead of a hook, with no error.

## `post_write_shell`

An optional command run after a successful write — the step that turns a local file into something delivered (committed and pushed, synced, indexed).

```toml
[modules.inbox]
post_write_shell = "git add '{{rel_path}}' && git commit -q -m 'capture: {{slug_or_time}}' -- '{{rel_path}}' && git push -q"
# post_write_shell_on_serve = true   # default false
```

**Tokens — this list is exhaustive:**

| Token | Value |
|---|---|
| `{{base_path}}` | the module's resolved root |
| `{{rel_path}}` | written file, relative to `base_path` |
| `{{abs_path}}` | written file, absolute |
| `{{slug}}` | kebab-cased `title` field, dash-prefixed (`-my-title`); empty when untitled |
| `{{slug_or_time}}` | bare slug (`my-title`), or a `%Y%m%d-%H%M%S` timestamp when untitled |

**Any other token — including `{{field_name}}` and `{{title}}` — is rejected at config-load time**, not silently stripped (contrast `path`, where unknown placeholders *are* stripped). Token spelling is exact: `{{ slug }}` is an error, not a synonym for `{{slug}}`.

Semantics:

- **Working directory is `base_path`** — so `git add <rel_path>` needs no `git -C`.
- Runs through the OS shell (`sh -c` / `cmd /C`), so `&&` and pipelines work.
- **Best-effort, never fatal.** The note is on disk before the hook runs. A non-zero exit, a spawn failure, or a 30s timeout surfaces as a warning (TUI summary message, or a `post_write_shell_failed` warning on the 201) — the capture is never lost.
- The child gets **null stdin and piped stdout/stderr**: a command that prompts fails fast instead of hanging behind the TUI, and its output cannot corrupt the terminal.
- **`post_write_shell_on_serve` defaults to `false`.** A LAN capture does not run commands unless the module opts in. (The wire DTO cannot carry config keys at all, so this is a second gate, not the only one.)

> [!danger] This is arbitrary command execution from config
> It is safe to interpolate without quoting **only because no token can carry raw user text** — the slug is `[a-z0-9-]` by construction. If a hook ever needs a token that *can* carry user text, that is the trigger to move to argv-style execution, **not** to extend the token list. Note also that a hook which auto-commits and pushes makes a bad capture public history.

## `{{slug}}` in `path`

`{{slug}}` and `{{slug_or_time}}` are **special** path tokens (like `{{date}}` and `{{time}}`), derived from the module's `title` field:

```
title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")
```

- Runs of non-`[a-z0-9]` collapse to a single `-`; leading/trailing dashes are trimmed. **All non-ASCII is dropped** — `café` → `caf` — matching the Obsidian Templater's JS exactly, so hand-authored and poured notes land on the same name.
- `{{slug}}` is dash-*prefixed* so `%Y%m%d-%H%M%S{{slug}}.md` reads `20260716-143255-my-title.md` when titled and `20260716-143255.md` when not.
- A field literally named `slug` is shadowed by the token (as a field named `date` already is).
- **Use `%S` in an untitled-capable path.** Two untitled captures in the same minute collide, and a filesystem collision is a hard error that discards the entry just typed — not a silent overwrite.

## Top-Level Config Keys

| Key | Type | Description |
|-----|------|-------------|
| `config_version` | string | Optional semver string declaring the config schema version (e.g. `"1.0.0"`). Defaults to `"0.1.0"` when absent. Non-semver values and unsupported major versions are rejected at load. Current version: `"1.0.0"`. Existing `0.x.y` configs continue to load unchanged. |
| `[vault].base_path` | string | Absolute path to the Obsidian vault root |
| `[vault].api_port` | integer | REST API port (default: `27124`) |
| `[vault].api_key` | string | Bearer token for API auth (overridden by `POUR_API_KEY` env var). Prefer `~/.pour/secrets.toml` over storing here. |
| `[vault].date_format` | string | strftime format string used to expand the `{{date}}` placeholder in module `path` and `append_template` values. Defaults to `"%Y%m%d"` when absent. Example: `"%Y-%m-%d"` produces `2026-04-21`. |
| `module_order` | string[] | Optional dashboard display ordering. Modules not listed appear alphabetically after listed ones |

### `date_format` Example

`date_format` controls what `{{date}}` resolves to in path and template strings. It does not affect strftime tokens (`%Y`, `%m`, `%d`) — those always expand using the standard strftime rules.

```toml
[vault]
base_path = "/path/to/vault"
date_format = "%Y-%m-%d"   # {{date}} → "2026-04-21" (default: "%Y%m%d" → "20260421")
```

Use `{{date}}` in a path or append template:

```toml
[modules.note]
mode = "create"
path = "Notes/%Y/%m/{{date}}-{{title}}.md"

[modules.journal]
mode = "append"
path = "Daily/%Y%m%d.md"
append_template = "- {{date}} {{time}} | {{body}}"
```

`date_format` is editable from the dashboard via vault settings (`v` → date_format field).

## Templates

Templates define the note structure created when a `dynamic_select` field with `create_template` triggers inline creation. Each `[templates.<name>]` block specifies the output path and a set of fields that appear in a sub-form overlay.

### Template Config Keys

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `path` | string | yes | Vault-relative output path for the created note. Must contain `{{name}}` (replaced with the user's typed value). Supports strftime tokens (`%Y`, `%m`, `%d`). Must not contain `..` path traversal. |
| `fields` | array | yes | At least one field definition (see below) |

### Template Field Keys

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | yes | Field identifier, used as the YAML frontmatter key. Must not be `date` or `name` (these are auto-generated). |
| `field_type` | string | yes | `text`, `number`, or `static_select` only |
| `prompt` | string | yes | Label shown in the sub-form overlay |
| `options` | string[] | conditional | Required for `static_select` |
| `default` | string | no | Pre-filled value. If the user leaves a field empty and no default exists, the key is omitted from frontmatter. |

### How Pour Templates Relate to Obsidian Templater

Pour templates and Obsidian's Templater plugin serve __complementary roles__:

- __Pour templates__ define and collect structured frontmatter at capture time (in the terminal, before the file exists).
- __Templater templates__ add body content, dynamic expressions, and formatting after the file is created (inside Obsidian).

The `post_create_command` config key bridges the two: after Pour writes the note with frontmatter, it fires an Obsidian command (e.g. `templater:run`) via the REST API, which triggers Templater to process the file. The Templater template can read Pour's frontmatter with `tp.frontmatter` and use it to build the note body.

__Example coordination:__

1. Pour's `[templates.bean]` collects `roaster`, `origin`, `process`, `roast_level`, `bag_weight_g` and writes them as YAML frontmatter.
2. `post_create_command = "templater:run"` fires Templater.
3. Templater's `(TEMPLATE) Bean.md` reads `tp.frontmatter.roaster` and `tp.frontmatter.origin` to build a wikilinked header, brew log table, and metadata block.

This means Pour handles *data capture* and Templater handles *presentation* — each doing what it's best at. Users who don't use Templater still get a fully functional note with clean frontmatter.

### Validation Rules

- Template `path` must contain `{{name}}`
- Template `path` must not contain `..` segments
- `static_select` template fields require non-empty `options`
- Template field names must be unique within a template
- Field names `date` and `name` are reserved (auto-generated in frontmatter)
- `create_template` is only valid on `dynamic_select` fields with `allow_create = true`
- `post_create_command` requires `create_template` to be set on the same field
- Referenced template names must exist in `[templates]`

### Module Validation Rules (root, frontmatter, hooks)

- `base_path` and every `[modules.<name>.platform]` value must be **absolute** and must not start with `~`
- `module.path` must stay root-relative even when `base_path` is set (no absolute, drive-qualified, UNC, or `..` paths)
- `[modules.<name>.frontmatter]` values must be a string, number, boolean, or a flat array of those — nested tables and TOML datetimes are rejected
- `date` is not a permitted `[modules.<name>.frontmatter]` key (auto-injected; use `frontmatter_date_format`)
- `[modules.<name>.frontmatter]` and `frontmatter_date_format` are `create`-mode only
- `post_write_shell` may only interpolate `{{base_path}}`, `{{rel_path}}`, `{{abs_path}}`, `{{slug}}`, `{{slug_or_time}}`; any other token is a load error
- `post_write_shell` must not be empty
- `post_write_shell_on_serve` requires `post_write_shell`

