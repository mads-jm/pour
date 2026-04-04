# AGENT.md — Pour Config Authoring for LLMs

This file helps LLM agents (Claude, GPT, Copilot, etc.) assist users in creating and adapting Pour config files. It complements `CLAUDE.md` (which covers the codebase) with config authoring guidance.

## What Pour Does

Pour is a TUI capture tool that writes structured data into an Obsidian vault. All behavior is defined in `~/.config/pour/config.toml`. There is no hardcoded module logic — the config IS the product.

## Config Schema (v0.2.0)

### Top-Level Structure

```toml
config_version = "0.2.0"
module_order = ["me", "todo", "note", "coffee"]   # dashboard order

[vault]
base_path = "/absolute/path/to/vault"
api_port = 27124                                    # optional
api_key = "key"                                     # optional, or POUR_API_KEY env var

[modules.<name>]          # each becomes a `pour <name>` command
[templates.<name>]        # inline creation templates referenced by dynamic_select fields
```

### Module Config

```toml
[modules.<name>]
mode = "append" | "create"
path = "vault-relative/path/%Y-%m-%d.md"      # strftime + {{field_name}} interpolation
display_name = "Human Name"                     # optional
append_under_header = "## Heading"              # required for append mode
append_template = "#### {{time}}\n{{body}}"     # optional, append mode only
callout_type = "note"                           # optional, resolves as {{callout}}
```

**Append mode**: Inserts content under an existing heading in an existing note. The note must already exist (daily note plugins typically handle creation). The heading must match exactly, including any wikilinks or formatting.

**Create mode**: Generates a new file. Path supports strftime tokens (`%Y`, `%m`, `%d`, `%H`, `%M`, `%S`) and field interpolation (`{{field_name}}`).

### Field Types

| Type | Config keys | Default target | Notes |
|------|------------|----------------|-------|
| `text` | — | frontmatter | Single line. `wikilink = true` wraps in `[[...]]` |
| `textarea` | `callout` | body | Multi-line editor overlay |
| `number` | — | frontmatter | Digits, `.`, `-` only |
| `static_select` | `options` (required) | frontmatter | Fixed dropdown |
| `dynamic_select` | `source` (required) | frontmatter | Vault folder scan. See below |
| `composite_array` | `sub_fields` (required) | frontmatter | Table editor. Sub-fields: text, number, static_select only |

Every field supports: `name`, `field_type`, `prompt`, `required`, `default`, `target`, `show_when`.

### Dynamic Select

```toml
[[modules.<name>.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"               # vault-relative folder containing .md files
allow_create = true                    # user can type novel values
wikilink = true                        # wraps output in [[...]]
create_template = "bean"               # opens sub-form overlay for structured creation
post_create_command = "templater:run"  # fires Obsidian command after note creation
```

**3-tier fallback**: API directory listing → filesystem scan → JSON cache → freetext input.

**Source folder**: Must exist in the vault. Pour lists `.md` files and strips extensions to get option names. Subdirectories are excluded.

### Conditional Visibility (show_when)

```toml
show_when = { field = "brew_method", equals = "Espresso" }
show_when = { field = "brew_method", one_of = ["Espresso", "Moka"] }
```

- Exactly one of `equals` or `one_of` — not both.
- Hidden fields are excluded from validation, output, and template rendering.
- Hidden `required` fields do NOT block submit.
- No AND/OR combinators, no negation, no sub-field conditions. Case-sensitive only.
- The referenced field must exist in the same module. No self-references. No cycles.

### Templates (inline creation)

```toml
[templates.<name>]
path = "Coffee/Beans/{{name}}.md"         # {{name}} = user's typed value

[[templates.<name>.fields]]
name = "roaster"
field_type = "text"                        # text, number, static_select only
prompt = "Roaster"
```

- Template field names cannot be `date` or `name` (auto-generated).
- `path` must contain `{{name}}`. Can also interpolate template field values: `{{category}}`.
- Template path must not contain `..` traversal.

## Config Authoring Rules

When helping a user write config, follow these rules:

### Path Rules
- All paths are vault-relative (no leading `/`, no `C:\`, no `\\`, no `..`)
- Append-mode paths must point to an existing file — verify the user's daily note naming convention
- Create-mode paths should include at least a date token (`%Y%m%d`) to avoid collisions
- Use `{{field_name}}` in create paths to make filenames descriptive

### Field Ordering
Fields appear in the TUI form in the order they're defined. Follow this pattern:
1. **Category selectors** (controlling fields for `show_when`)
2. **Conditional fields** (gated by the selector)
3. **Universal fields** (always visible)
4. **Wrap-up** (notes, rating — last in the form)

### show_when Design
- Put the controlling `static_select` first in the field array
- Group dependent fields by category immediately after the controller
- Forward references are allowed (field can reference a field defined later)
- Each conditional field can only depend on ONE other field

### Dynamic Select Design
- Each `source` folder must exist in the vault and contain `.md` files
- For category-dependent equipment, use separate dynamic_selects with different `source` folders and `show_when` conditions
- `allow_create` enables typing novel values — add `create_template` for structured creation
- `wikilink = true` creates Obsidian backlinks in frontmatter

### Append Template Design
- `{{time}}` and `{{date}}` are built-in
- `{{callout}}` resolves to the module's `callout_type`
- Any field name works as `{{field_name}}`
- Hidden fields resolve to empty string in templates
- Match the target note's existing format (headings, callout style, list style)

### Common Patterns

**Journal append** (add thoughts to daily note):
```toml
[modules.journal]
mode = "append"
path = "Daily/%Y-%m-%d.md"
append_under_header = "## Journal"
append_template = "#### {{time}}\n{{body}}"
```

**Task capture** (checkbox to daily note):
```toml
[modules.todo]
mode = "append"
path = "Daily/%Y-%m-%d.md"
append_under_header = "## Tasks"
append_template = "- [ ] {{body}}"
```

**Fleeting note** (quick standalone capture):
```toml
[modules.note]
mode = "create"
path = "Fleeting/%Y%m%d-{{title}}.md"
```

**Category-dependent create** (coffee, workout, etc.):
```toml
[modules.coffee]
mode = "create"
path = "Coffee/{{bean}}-%Y%m%d.md"

# Category field drives conditional visibility
[[modules.coffee.fields]]
name = "brew_method"
field_type = "static_select"
options = ["Pour Over", "Espresso"]
required = true

# Conditional per category
[[modules.coffee.fields]]
name = "brewer"
field_type = "dynamic_select"
source = "Coffee/Brewers/Pour Over"
show_when = { field = "brew_method", equals = "Pour Over" }

[[modules.coffee.fields]]
name = "machine"
field_type = "dynamic_select"
source = "Coffee/Brewers/Espresso"
show_when = { field = "brew_method", equals = "Espresso" }
```

**Equipment with inline creation** (reusable across modules):
```toml
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
source = "Coffee/Beans"
allow_create = true
wikilink = true
create_template = "bean"
post_create_command = "templater:run"

[templates.bean]
path = "Coffee/Beans/{{name}}.md"
# ... template fields
```

## Validation Checklist

Before presenting a config to the user, verify:

- [ ] `config_version = "0.2.0"` is present
- [ ] `vault.base_path` is an absolute path
- [ ] Every module has at least one field
- [ ] Append modules have `append_under_header`
- [ ] All paths are vault-relative (no `/`, `C:\`, `\\`, `..`)
- [ ] `static_select` fields have non-empty `options`
- [ ] `dynamic_select` fields have `source`
- [ ] `composite_array` fields have `sub_fields` with unique names
- [ ] `show_when.field` references an existing field in the same module
- [ ] `show_when` has exactly one of `equals` or `one_of` (not both, not neither)
- [ ] `equals` is not empty string; `one_of` is not empty array
- [ ] No circular `show_when` dependencies
- [ ] `create_template` references an existing `[templates.<name>]`
- [ ] `allow_create` is only on `dynamic_select` fields
- [ ] `post_create_command` requires `create_template`
- [ ] Template paths contain `{{name}}`
- [ ] Template field names are not `date` or `name`
- [ ] `module_order` lists module keys, not display names

## Reference Files

- `CLAUDE.md` — codebase development guidance
- `pour - docs/02 references/field-types.md` — exhaustive field type reference
- `pour - docs/08 specs/pour-design-spec.md` — product design spec
- `pour - docs/03 guides/Guide-Config-to-Vault.md` — human-readable vault adaptation guide
- `resources/default_config.toml` — default config with all field types demonstrated
- `resources/mads_config.toml` — real-world config with advanced patterns (show_when, templates, composite_array)
