---
tags:
  - reference
  - config
  - fields
date created: Wednesday, April 1st 2026, 10:49:25 pm
date modified: Friday, April 3rd 2026, 4:11:41 am
---

# Field Types Reference

This document covers all field types available in Pour's `config.toml` schema, their config keys, validation rules, default output targets, and TUI rendering behavior.

## Field Config Keys

Every field in a module's `[[modules.<name>.fields]]` array supports these keys:

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | yes | Field identifier, used as the YAML frontmatter key |
| `field_type` | string | yes | One of the six types below |
| `prompt` | string | yes | Label shown in the TUI form |
| `required` | bool | no | If `true`, submit is blocked when the field is empty |
| `default` | string | no | Pre-filled value on form init |
| `options` | string[] | conditional | Required for `static_select`; ignored otherwise |
| `source` | string | conditional | Required for `dynamic_select`; vault-relative directory path |
| `target` | string | no | `"frontmatter"` or `"body"` — overrides the default routing |
| `sub_fields` | array | conditional | Required for `composite_array`; column definitions |
| `callout` | string | no | Obsidian callout type (e.g. `"note"`, `"tip"`). When set on a `textarea` field targeting body, the output is wrapped in `> [!type]` blockquote syntax. |
| `allow_create` | bool | no | Only valid on `dynamic_select`. When `true`, the user can type characters to filter options and enter a completely novel value if nothing matches. Defaults to `false` (closed list). |
| `wikilink` | bool | no | If `true`, wraps the output value in Obsidian wikilink syntax: `[[value]]`. Applies to `text`, `static_select`, and `dynamic_select` fields. No-ops if the value is already wrapped. Defaults to `false`. |
| `create_template` | string | no | Only valid on `dynamic_select` fields with `allow_create = true`. References a template name from `[templates.<name>]`. When set, typing a novel value opens a sub-form overlay to fill in the template's fields before creating the note. Without this key, novel values create a bare stub note. |
| `post_create_command` | string | no | Obsidian command ID to execute after template-driven note creation (e.g. `"templater:run"`). Only valid when `create_template` is set. Fires via the REST API `/commands/` endpoint; silently skipped on filesystem transport. |
| `show_when` | object | no | Conditional visibility rule. When present, the field is only rendered and navigable if the condition is satisfied. If the condition becomes false while the field is focused, focus moves to the nearest visible field. See **Conditional Visibility** below. |

## Output Target Defaults

| Field Type | Default Target |
|------------|---------------|
| `text` | frontmatter |
| `number` | frontmatter |
| `static_select` | frontmatter |
| `dynamic_select` | frontmatter |
| `textarea` | body |
| `composite_array` | frontmatter |

Any field can override its default via `target = "frontmatter"` or `target = "body"`.

---

## Conditional Visibility

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

**Visibility rules:**
- `equals`: visible if `field_values[field] == equals` (case-sensitive).
- `one_of`: visible if `field_values[field]` matches any listed value (case-sensitive).
- If the controlling field is absent or empty, the conditional field is hidden.

**Submit behavior:**
- Hidden fields are skipped during validation — a hidden `required` field does not block submit.
- Hidden field values are cleared on submit, so no stale data appears in output.
- Hidden fields are excluded from frontmatter, body, and template placeholder resolution. Template placeholders for hidden fields resolve to empty string.

**Navigation behavior:**
- Tab/Shift-Tab/Up/Down bounds are computed from the *visible* field set, not total field count.
- If a field becomes hidden while focused (e.g. the user changes a controlling field), focus moves to the next visible field, then previous, then the submit button.
- New fields becoming visible do **not** steal focus.

**Config validation rules:**
- Exactly one of `equals` or `one_of` must be specified — not both, not neither.
- `equals` must not be an empty string.
- `one_of` must not be an empty array.
- `show_when.field` must reference an existing field in the same module.
- A field cannot reference itself.
- A field cannot reference a `composite_array` field as the controller.
- Circular dependencies are rejected (A→B→A, or longer chains).
- Forward references (referencing a field defined later in the array) are allowed.

**Limitations (v1):**
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
__Callout wrapping__: When `callout = "note"` (or any Obsidian callout type) is set, the body output is automatically wrapped in blockquote callout syntax:

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
__Validation__: `options` must be present and non-empty. Config load fails otherwise.

## `dynamic_select`

Dropdown populated from vault directory contents at runtime.

```toml
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"
```

__TUI__: Same dropdown interaction as `static_select`. Options are populated via the 3-tier fallback: API directory listing, filesystem scan, JSON cache (`~/.cache/pour/state.json`), then freetext input if all fail.

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

__TUI__: Enter opens a bordered table editor overlay. Navigate cells with arrow keys. Tab advances to next cell. Enter adds a new row. Escape closes the overlay. Empty rows are stripped on output.
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
| `mode` | string | yes | `"create"` (new file per entry) or `"append"` (add to existing note) |
| `path` | string | yes | Vault-relative output path. Supports strftime tokens: `%Y`, `%m`, `%d`, `%H`, `%M`, `%S` |
| `fields` | array | yes | At least one field definition |
| `display_name` | string | no | Human-readable name shown in the dashboard (defaults to module key) |
| `append_under_header` | string | conditional | Required when `mode = "append"`. The Markdown heading to append under |
| `append_template` | string | no | Template for append-mode content. Supports `{{time}}`, `{{date}}`, `{{callout}}`, and field name placeholders |
| `callout_type` | string | no | Obsidian callout type (e.g. `"note"`, `"tip"`). Resolved as `{{callout}}` in `append_template` |

## Top-Level Config Keys

| Key | Type | Description |
|-----|------|-------------|
| `config_version` | string | Optional semver string declaring the config schema version (e.g. `"0.2.0"`). Defaults to `"0.1.0"` when absent. Non-semver values and unsupported major versions are rejected at load. |
| `[vault].base_path` | string | Absolute path to the Obsidian vault root |
| `[vault].api_port` | integer | REST API port (default: `27124`) |
| `[vault].api_key` | string | Bearer token for API auth (overridden by `POUR_API_KEY` env var) |
| `module_order` | string[] | Optional dashboard display ordering. Modules not listed appear alphabetically after listed ones |

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

