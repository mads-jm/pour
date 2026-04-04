---
tags:
  - guide
  - config
  - vault
  - onboarding
date created: Friday, April 4th 2026
---

# Guide: Adapting Pour to Your Vault

Pour is config-driven — every module, field, path, and template is defined in `~/.config/pour/config.toml`. This guide walks through mapping that config to **your** Obsidian vault structure, starting from the default config and building up to fully customized modules.

For the complete field reference, see [[field-types]]. For module patterns, see [[pour-design-spec]].

---

## 1. Establish the Vault Connection

```toml
[vault]
base_path = "/absolute/path/to/your/vault"
```

This is the root — every `path`, `source`, and template path is relative to this. Find it by looking at where your `.obsidian/` folder lives.

**Windows**: Use escaped backslashes or forward slashes.

```toml
base_path = "C:\\Users\\You\\Documents\\MyVault"
base_path = "C:/Users/You/Documents/MyVault"
```

**API (optional)**: If you run the [Obsidian Local REST API](https://github.com/coddingtonbear/obsidian-local-rest-api) plugin, Pour can write via API and fall back to filesystem when Obsidian is closed.

```toml
api_port = 27124
api_key = "your-key"   # or set POUR_API_KEY env var
```

---

## 2. Understand Write Modes

Every module is either **append** or **create**. This is the most important design decision per module.

### Append Mode

Adds content under a heading in an existing note. Best for:
- Daily journals — append thoughts to today's note
- Running logs — add entries to a single note over time
- Task capture — add checkboxes under a heading

```toml
[modules.me]
mode = "append"
path = "Journal/%Y/%Y-%m-%d.md"           # strftime tokens resolve at runtime
append_under_header = "## Log"             # must match an existing heading
append_template = "#### {{time}}\n{{body}}" # {{time}}, {{field_name}} placeholders
```

**Key constraint**: The note must already exist with the target heading. Pour doesn't create the file in append mode — it finds the heading and inserts below it. Daily note plugins (Templater, Periodic Notes) typically handle file creation.

**Mapping to your vault**: Open your daily note template. Find the heading you want to append under. Copy it exactly (including any wikilinks or formatting).

### Create Mode

Generates a new file per entry. Best for:
- Brew logs, recipes, workouts — one note per event
- Fleeting notes — standalone captures
- Anything that becomes its own page in the vault graph

```toml
[modules.coffee]
mode = "create"
path = "Coffee/%Y/%Y-%m-%d-{{bean}}.md"   # {{field_name}} interpolation
```

**Mapping to your vault**: Decide where these notes should live. Use existing folder structures. The path supports both strftime tokens (`%Y`, `%m`, `%d`) and field value interpolation (`{{field_name}}`).

---

## 3. Map Your Vault Folders to Config Paths

The most common mistake is getting paths wrong. Here's how to audit your vault structure and map it to config.

### Step 1: Identify your folder scheme

Common Obsidian patterns:

| Scheme | Example | How to reference |
|--------|---------|-----------------|
| PARA | `02 - Areas/204 - Cooking/Coffee/` | Use the full relative path |
| Flat | `Coffee/Beans/` | Short relative path |
| Date-nested | `Journal/2026/2026-04-04.md` | Use `%Y/%Y-%m-%d.md` |
| Periodic | `Periodic/Daily/20260404.md` | Use `%Y%m%d.md` |

### Step 2: Map module paths

For each module, trace the path from vault root to where notes should land:

```
Vault root
└── 02 - Areas/
    └── 204 - Cooking/
        └── Coffee/
            ├── Beans/          ← dynamic_select source
            ├── Brewers/        ← dynamic_select source (subfolder per category)
            └── Brews/          ← create-mode output path
```

This maps to:

```toml
[modules.coffee]
path = "02 - Areas/204 - Cooking/Coffee/Brews/{{bean}}@{{time}}-%Y%m%d.md"

[[modules.coffee.fields]]
name = "bean"
source = "02 - Areas/204 - Cooking/Coffee/Beans"
```

### Step 3: Verify source folders exist

Every `dynamic_select` field's `source` path must point to an existing folder in the vault. Pour lists `.md` files in that folder to populate the dropdown. If the folder doesn't exist, Pour falls back to the cache, then to freetext input.

Create the folders first, then add at least one `.md` file so the dropdown has content.

---

## 4. Design Your Fields

Fields flow top-to-bottom in the TUI form. Group them by workflow:

1. **Category/selector fields first** — these control conditional visibility via `show_when`
2. **Conditional fields next** — equipment, method-specific params
3. **Universal fields** — fields that appear regardless of selection
4. **Wrap-up last** — rating, notes, tasting notes

### Field Type Decision Tree

```
Is the set of values fixed and small?
  → static_select (options in config)

Does the set of values come from vault folders?
  → dynamic_select (source = vault path)
  → Add allow_create = true if the user should be able to add new values

Is it a number?
  → number

Is it multi-line text?
  → textarea (defaults to body output)

Is it tabular / multi-row data?
  → composite_array (sub_fields define columns)

Otherwise:
  → text
```

### Conditional Fields

Use `show_when` to hide fields that don't apply to the current context:

```toml
# This field only appears when brew_method is "Espresso"
[[modules.coffee.fields]]
name = "shot_style"
field_type = "static_select"
options = ["Standard", "Turbo", "Soup"]
show_when = { field = "brew_method", equals = "Espresso" }
```

The pattern: a controlling `static_select` at the top, then dependent fields gated by its value. Hidden fields are excluded from validation and output — a hidden `required` field doesn't block submit.

---

## 5. Wire Up Dynamic Selects

Dynamic selects are Pour's most powerful feature — they connect the config to your vault's living data.

### Basic Setup

```toml
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"           # vault-relative folder
```

Pour lists `.md` files in `<vault>/Coffee/Beans/` and strips the extension to get option names. Subdirectories are excluded.

### Adding Inline Creation

```toml
allow_create = true                # enable novel value entry
wikilink = true                    # wrap output in [[...]]
create_template = "bean"           # open sub-form for structured creation
post_create_command = "templater:run"  # fire Templater after creation
```

This requires a matching `[templates.bean]` section — see step 6.

### Conditional Equipment Selects

For category-dependent equipment (e.g., different brewers per brew method):

```toml
# Each category gets its own dynamic_select pointing to a subfolder
[[modules.coffee.fields]]
name = "brewer"
source = "Coffee/Brewers/Pour Over"
show_when = { field = "brew_method", equals = "Pour Over" }

[[modules.coffee.fields]]
name = "machine"
source = "Coffee/Brewers/Espresso"
show_when = { field = "brew_method", equals = "Espresso" }
```

This pattern is reusable for any domain with category-dependent options.

---

## 6. Templates for Inline Creation

When a `dynamic_select` has `allow_create = true` and `create_template`, typing a novel value opens a sub-form overlay. The template defines what fields to collect and where to save the file.

```toml
[templates.bean]
path = "Coffee/Beans/{{name}}.md"         # {{name}} = the typed value

[[templates.bean.fields]]
name = "roaster"
field_type = "text"
prompt = "Roaster"

[[templates.bean.fields]]
name = "origin"
field_type = "static_select"
prompt = "Origin"
options = ["Ethiopia", "Colombia", "Guatemala", "Kenya", "Brazil"]
```

**Path routing with fields**: Template paths can interpolate template field values. This is useful for sorting new notes into subfolders:

```toml
[templates.brewer]
path = "Coffee/Brewers/{{category}}/{{name}}.md"

[[templates.brewer.fields]]
name = "category"
field_type = "static_select"
prompt = "Brewer category"
options = ["Pour Over", "Espresso", "Immersion"]
```

### Coordinating with Obsidian Templater

Pour writes frontmatter. Templater adds body content. The bridge is `post_create_command`:

1. Pour creates `Beans/Ethiopia Guji.md` with YAML frontmatter
2. `post_create_command = "templater:run"` fires via the REST API
3. Your Obsidian Templater template reads `tp.frontmatter.roaster`, etc. and adds the body

This separation means Pour handles *data capture* and Templater handles *presentation*. If you don't use Templater, the note still has clean frontmatter — it just won't have body structure.

---

## 7. Append Templates

For append-mode modules, the `append_template` controls what gets inserted:

```toml
append_template = "#### {{time}}\n> [!{{callout}}] {{title}}\n> {{body}}"
```

Available placeholders:
- `{{time}}` — current time (HH:MM format)
- `{{date}}` — current date
- `{{callout}}` — value of `callout_type` on the module
- `{{field_name}}` — any field's value by name

**Matching your daily note structure**: Your template's output should be consistent with the note's existing format. If your daily note uses callout blocks under headings, design the append template to match.

---

## 8. Module Order and Dashboard

```toml
module_order = ["me", "todo", "note", "coffee"]
```

Controls dashboard display order. Modules not listed appear alphabetically after listed ones. Put your most-used modules first for quick access.

---

## 9. Worked Example: Adapting to a New Vault

Say your vault looks like this:

```
MyVault/
├── Daily/
│   └── 2026-04-04.md          (daily notes with ## Journal heading)
├── Projects/
│   └── ...
├── Recipes/
│   ├── Ingredients/
│   │   ├── Flour.md
│   │   └── Sugar.md
│   └── ...
└── Notes/
    └── ...                    (fleeting notes)
```

Your config might be:

```toml
config_version = "0.2.0"
module_order = ["journal", "recipe", "note"]

[vault]
base_path = "/Users/you/MyVault"

# Journal — append to daily note
[modules.journal]
mode = "append"
path = "Daily/%Y-%m-%d.md"
display_name = "Journal"
append_under_header = "## Journal"
append_template = "#### {{time}}\n{{body}}"

[[modules.journal.fields]]
name = "body"
field_type = "textarea"
prompt = "What's on your mind?"
required = true
target = "body"

# Recipe — create a new recipe note
[modules.recipe]
mode = "create"
path = "Recipes/%Y-%m-%d-{{title}}.md"
display_name = "Recipe"

[[modules.recipe.fields]]
name = "title"
field_type = "text"
prompt = "Recipe name"
required = true
target = "frontmatter"

[[modules.recipe.fields]]
name = "servings"
field_type = "number"
prompt = "Servings"
default = "4"
target = "frontmatter"

[[modules.recipe.fields]]
name = "ingredients"
field_type = "composite_array"
prompt = "Ingredients"
target = "frontmatter"

[[modules.recipe.fields.sub_fields]]
name = "item"
field_type = "dynamic_select"   # ERROR: sub_fields don't support dynamic_select
# Use text instead:
# field_type = "text"
prompt = "Item"

[[modules.recipe.fields.sub_fields]]
name = "amount"
field_type = "number"
prompt = "Amount"

[[modules.recipe.fields]]
name = "instructions"
field_type = "textarea"
prompt = "Instructions"
target = "body"

# Quick note — fleeting capture
[modules.note]
mode = "create"
path = "Notes/%Y%m%d-{{title}}.md"
display_name = "Note"

[[modules.note.fields]]
name = "title"
field_type = "text"
prompt = "Title"
required = true
target = "frontmatter"

[[modules.note.fields]]
name = "body"
field_type = "textarea"
prompt = "Content"
target = "body"
```

---

## 10. Validation and Testing

After editing your config, test it:

```bash
cargo run                        # opens dashboard — catches parse errors
cargo run -- <module_name>       # test a specific module form
```

Common errors:
- **"field requires source"** — `dynamic_select` is missing `source` path
- **"options must not be empty"** — `static_select` is missing `options`
- **"path is not vault-relative"** — path starts with `/`, `C:\`, `\\`, or contains `..`
- **"circular show_when dependency"** — field A depends on B which depends on A
- **"unknown template reference"** — `create_template` names a template that doesn't exist in `[templates]`

---

## Checklist: New Module

- [ ] Decide mode: `append` (add to existing note) or `create` (new file)
- [ ] Set `path` using vault-relative path with strftime tokens and/or `{{field}}` interpolation
- [ ] For append: set `append_under_header` matching an exact heading in the target note
- [ ] For append: design `append_template` matching the note's existing format
- [ ] Define fields top-to-bottom: selectors → conditional → universal → wrap-up
- [ ] For dynamic_selects: verify source folders exist in vault with `.md` files
- [ ] For allow_create: add `[templates.<name>]` section with path and fields
- [ ] For Templater coordination: set `post_create_command = "templater:run"` and create matching Obsidian template
- [ ] Add module to `module_order` for dashboard positioning
- [ ] Test with `cargo run -- <module>`
