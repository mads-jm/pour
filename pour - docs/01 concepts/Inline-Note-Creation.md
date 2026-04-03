---
tags:
  - concept
  - data
  - tui
  - dynamic_select
date created: Friday, April 3rd 2026, 2:20:35 am
date modified: Friday, April 3rd 2026, 4:11:41 am
---

# Inline Note Creation

When a `dynamic_select` field has `allow_create = true`, Pour extends the capture flow to include vault-side note creation. This closes the loop between data entry and data management — the user never leaves the TUI to scaffold a new option.

## How It Works

1. The user types a value that doesn't match any existing option in the dropdown.
2. The dropdown shows `+ Create "{value}"` as a visual affordance.
3. On form submit, Pour detects the novel value and auto-creates a bare note at `{source}/{value}.md` with minimal frontmatter (`date` only).
4. The note is created __before__ the main module output is written, so any `wikilink = true` references resolve immediately in Obsidian.
5. The local cache is updated so subsequent form loads include the new option without a transport round-trip.

## Relationship to the 3-Tier Fallback

This feature extends [[The-3-Tier-Data-Fallback]] with an __output side__. The fallback pipeline handles *reading* options into the TUI; inline creation handles *writing* new options back to the vault. Together they form a closed loop:

```ts
Vault ──[fetch]──> Cache ──> Dropdown ──[novel value]──> Auto-create ──> Vault
```

When all three tiers fail and the dropdown is empty, `allow_create` fields accept freetext input (Tier 3 behavior). The submitted value triggers note creation, seeding the cache for future sessions.

## Filename Sanitization

User-typed values are sanitized before becoming filenames:

- Characters invalid in cross-platform filenames (`: ? * < > | " \ /`) are replaced with `-`
- Consecutive dashes are collapsed
- Leading/trailing dashes and whitespace are trimmed
- Windows reserved device names (`CON`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`) are rejected
- Empty results after sanitization are skipped

Duplicate detection is __case-insensitive__ and checks both the raw typed value and its sanitized form against existing options.

## Best-Effort Semantics

Note creation is best-effort. If the transport layer fails (network error, permission issue), the main form write still proceeds. The failure is logged to stderr but does not block the user's capture flow.

## Template-Driven Creation

By default, inline creation produces a __bare stub__ — a note with only a `date` field. For richer notes, add a `create_template` reference to the field:

```toml
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"
allow_create = true
wikilink = true
create_template = "bean"
post_create_command = "templater:run"  # optional — fires an Obsidian command after creation
```

When the user types a novel value, a __sub-form overlay__ appears with the template's fields. The user fills them in without leaving Pour, and the created note gets full structured frontmatter.

### Sub-Form Overlay

The overlay is a centered modal that appears over the main form:

- Shows the template's fields (text, number, static_select)
- `Tab`/`Shift+Tab` navigates fields
- `Enter` on the submit button creates the note
- `Esc` cancels without creating anything
- Falls back to bare stub creation if the terminal is too small

### Post-Creation Command Hook

`post_create_command` fires an Obsidian command via the REST API after note creation. This bridges Pour's data capture with Obsidian plugins:

- Pour writes frontmatter (structured data it collected)
- Templater (or any plugin) adds body content, formatting, and dynamic expressions
- Only fires when connected via API; silently skipped on filesystem transport

### Template Definition

Templates are defined at the top level of `config.toml`:

```toml
[templates.bean]
path = "Coffee/Beans/{{name}}.md"

[[templates.bean.fields]]
name = "roaster"
field_type = "text"
prompt = "Roaster"

[[templates.bean.fields]]
name = "origin"
field_type = "static_select"
prompt = "Origin"
options = ["Ethiopia", "Colombia", "Kenya"]
```

`{{name}}` is replaced with the user's typed value. The path also supports strftime tokens (`%Y`, `%m`, `%d`). See [[field-types#Templates]] for the full schema reference.

## Config

```toml
# Basic — bare stub creation
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"
allow_create = true
wikilink = true

# Advanced — template-driven creation with Templater hook
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"
allow_create = true
wikilink = true
create_template = "bean"
post_create_command = "templater:run"
```

See [[field-types]] for the full `allow_create`, `wikilink`, `create_template`, and `post_create_command` reference.

