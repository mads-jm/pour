---
tags:
  - reference
  - config
  - fields
date created: Wednesday, April 1st 2026, 10:49:25 pm
date modified: Thursday, April 2nd 2026, 9:18:47 am
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

## `text`

Single-line free text input.

```toml
[[modules.coffee.fields]]
name = "origin"
field_type = "text"
prompt = "Bean origin"
```

__TUI__: Inline text input with cursor. Accepts any characters.
__Output__: Value written as-is to frontmatter (or body if overridden).

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
__Output__: Selected string written to frontmatter.
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
__Output__: Selected string written to frontmatter.
__Validation__: `source` must be present and must be a vault-relative path (no absolute, drive-qualified, UNC, or `..` traversal paths). Config load fails otherwise.
__Source path__: Relative to the vault root. Example: `"Coffee/Beans"` resolves to `<vault_base_path>/Coffee/Beans/`.

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
| `append_template` | string | no | Template for append-mode content. Supports `{{time}}` and field name placeholders |

## Top-Level Config Keys

| Key | Type | Description |
|-----|------|-------------|
| `[vault].base_path` | string | Absolute path to the Obsidian vault root |
| `[vault].api_port` | integer | REST API port (default: `27124`) |
| `[vault].api_key` | string | Bearer token for API auth (overridden by `POUR_API_KEY` env var) |
| `module_order` | string[] | Optional dashboard display ordering. Modules not listed appear alphabetically after listed ones |
