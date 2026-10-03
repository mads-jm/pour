---
tags:
  - guide
  - config
  - vault
  - onboarding
date created: Sunday, April 5th 2026, 9:34:22 pm
date modified: Saturday, October 3rd 2026, 6:47:17 am
---

# Guide: Adapting Pour to Your Vault

Pour is config-driven — every module, field, path, and template is defined in `~/.pour/config.toml`. This guide walks through mapping that config to __your__ Obsidian vault structure, starting from the default config and building up to fully customized modules.

For the complete field reference, see [[field-types]]. For module patterns, see [[pour-design-spec]].

---

## 1. Establish the Vault Connection

```toml
[vault]
base_path = "/absolute/path/to/your/vault"
```

This is the root — every `path`, `source`, and template path is relative to this. Find it by looking at where your `.obsidian/` folder lives.

__Windows__: Use escaped backslashes or forward slashes.

```toml
base_path = "C:\\Users\\You\\Documents\\MyVault"
base_path = "C:/Users/You/Documents/MyVault"
```

__API (optional)__: If you run the [Obsidian Local REST API](https://github.com/coddingtonbear/obsidian-local-rest-api) plugin, Pour can write via API and fall back to filesystem when Obsidian is closed.

```toml
api_port = 27124
api_key = "your-key"   # or put it in ~/.pour/secrets.toml, or set POUR_API_KEY
```

__Several machines__: If the vault mounts at a different path on each machine, keep `base_path` as the default and add per-OS overrides under `[vault.platform]`. The key is the OS name (`linux`, `macos`, `windows`), and a matching key wins over `base_path`.

```toml
[vault]
base_path = "/home/you/Vault"

[vault.platform]
windows = "D:/Vault"
```

---

## 2. Understand Write Modes

Every module is __append__, __create__, or __update__. This is the most important design decision per module. [[Pour-Types]] explains which kind of signal belongs in which mode.

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

__Key constraint__: The note must already exist with the target heading. Pour doesn't create the file in append mode — it finds the heading and inserts below it. If the note or the heading is missing, the capture fails on both transports. Daily note plugins (Templater, Periodic Notes) typically handle file creation.

__Mapping to your vault__: Open your daily note template. Find the heading you want to append under. Copy it exactly (including any wikilinks or formatting).

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

__Mapping to your vault__: Decide where these notes should live. Use existing folder structures. The path supports both strftime tokens (`%Y`, `%m`, `%d`) and field value interpolation (`{{field_name}}`).

### Update Mode

Changes frontmatter keys on a note that already exists. Best for:
- Daily habits, a yes/no for the day or a running total
- Any property your periodic note template already defines

```toml
[modules.habit]
mode = "update"
path = "Journal/%Y/%Y-%m-%d.md"            # the note must already exist

[[modules.habit.fields]]
name = "water"                             # must match the frontmatter key exactly
field_type = "counter"
prompt = "Water"
unit = "oz"
goal = 96
```

__Key constraint__: Pour never creates the note and never touches its body. The field name is the frontmatter key it writes, with no mapping in between, so match your template's property names. Every field must target frontmatter. A field that targets the body (a `textarea` does by default), a `composite_array`, or `list = true` fails validation, and so do the append-only keys (`append_under_header`, `append_template`, `append_shallow`) and `daily_link`.

Update modules also have a one-shot form that skips the TUI: `pour habit water 16` adds 16 to the current value, and `pour habit water =64` sets it. See [[field-types#`update` mode]] for the full rules.

---

## 3. Map Your Vault Folders to Config Paths

The most common mistake is getting paths wrong. Here's how to audit your vault structure and map it to config.

### Step 1: Identify Your Folder Scheme

Common Obsidian patterns:

| Scheme | Example | How to reference |
|--------|---------|-----------------|
| PARA | `02 - Areas/204 - Cooking/Coffee/` | Use the full relative path |
| Flat | `Coffee/Beans/` | Short relative path |
| Date-nested | `Journal/2026/2026-04-04.md` | Use `%Y/%Y-%m-%d.md` |
| Periodic | `Periodic/Daily/20260404.md` | Use `%Y%m%d.md` |

### Step 2: Map Module Paths

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

### Step 3: Verify Source Folders Exist

Every `dynamic_select` field's `source` path must point to an existing folder in the vault. Pour lists `.md` files in that folder to populate the dropdown. If the folder doesn't exist, Pour falls back to the cache, then to freetext input.

Create the folders first, then add at least one `.md` file so the dropdown has content.

---

## 4. Design Your Fields

Fields flow top-to-bottom in the TUI form. Group them by workflow:

1. __Category/selector fields first__ — these control conditional visibility via `show_when`
2. __Conditional fields next__ — equipment, method-specific params
3. __Universal fields__ — fields that appear regardless of selection
4. __Wrap-up last__ — rating, notes, tasting notes

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

Is it a yes/no property?
  → toggle

Is it a total that grows through the day?
  → counter (meant for update mode)

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

Use `one_of` when a field should appear for multiple values:

```toml
# Shows for both Pour Over and Immersion methods
[[modules.coffee.fields]]
name = "water_temp_c"
field_type = "number"
prompt = "Water temp (°C)"
show_when = { field = "brew_method", one_of = ["Pour Over", "Immersion"] }
```

The pattern: a controlling `static_select` at the top, then dependent fields gated by its value. Hidden fields are excluded from validation and output — a hidden `required` field doesn't block submit.

### Field-Level Callouts

Textarea fields targeting `body` can be wrapped in an Obsidian callout:

```toml
[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Tasting notes"
target = "body"
callout = "quote"    # wraps output in > [!quote] block
```

This is separate from the module-level `callout_type` (which controls `{{callout}}` in append templates). Field-level `callout` wraps individual field output.

### Preset Exclusion

Fields that shouldn't be captured or restored by presets (e.g., free-form notes that change every entry) can be excluded:

```toml
[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Tasting notes"
preset_exclude = true   # skipped when saving/applying presets
```

Use `preset_exclude = true` on:
- `textarea` body fields where content varies every entry (journal thoughts, tasting notes)
- `title` or `name` text fields on create-mode modules where each entry has a unique title

Fields marked `preset_exclude` are still filled and submitted normally — they're just invisible to the preset save/apply cycle.

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

__Path tokens__: A template path substitutes `{{name}}` (the typed value, cleaned up for use as a filename) and strftime tokens (`%Y`, `%m`, `%d`). The path must contain `{{name}}`. Template field values are not substituted into the path, so `Coffee/Brewers/{{category}}/{{name}}.md` keeps `{{category}}` as written. To sort new notes into subfolders, give each subfolder its own template and point each conditional select at the matching one:

```toml
[templates.brewer_pour_over]
path = "Coffee/Brewers/Pour Over/{{name}}.md"

[[templates.brewer_pour_over.fields]]
name = "brand"
field_type = "text"
prompt = "Brand"
```

`date` and `name` are reserved template field names, because Pour writes both into the new note's frontmatter itself.

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
- `{{date}}` — current date as `YYYY-MM-DD` (`date_format` does not apply here)
- `{{callout}}` — value of `callout_type` on the module
- `{{field_name}}` — any field's value by name

strftime tokens such as `%A` also expand in an append template.

__Matching your daily note structure__: Your template's output should be consistent with the note's existing format. If your daily note uses callout blocks under headings, design the append template to match.

---

## 8. Module Order and Dashboard

```toml
module_order = ["me", "todo", "note", "coffee"]
```

Controls dashboard display order. Modules not listed appear alphabetically after listed ones. Put your most-used modules first for quick access. `Ctrl+Up` / `Ctrl+Down` on the dashboard moves the selected module and writes the new order back to `module_order`.

---

## 9. Icons

Modules and fields support an optional `icon` for visual identification.

### Module Icons

Module icons appear on the TUI dashboard next to the module name. For create-mode modules, they're also written to the output YAML frontmatter as `icon: <value>`, making them queryable by Dataview and compatible with Obsidian plugins like Iconize and Supercharged Links.

```toml
[modules.coffee]
mode = "create"
path = "Coffee/%Y/%Y-%m-%d-{{bean}}.md"
display_name = "Coffee"
icon = "☕"           # shown on dashboard, written to frontmatter
```

Dashboard appearance: `▸ ☕ [coffee]  Coffee`

### Field Icons

Field icons appear next to the prompt label in the TUI form. They are purely cosmetic — not written to output.

```toml
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
icon = "🫘"           # shown in form only
```

Form appearance: `▸ 🫘 Bean*: Ethiopian Guji`

### Format

Icons are free-form strings. Use Unicode emoji (`"☕"`) for widest compatibility, or Iconize pack format (`"LiCoffee"`) if your vault uses the Iconize plugin. Pour stores whatever you configure — it doesn't validate or interpret the value.

---

## 10. Presets

Presets let you save a snapshot of a module's current field values and restore them on future entries. They're designed for workflows where most fields stay the same across entries (same bean, same grinder, same dose) but a few fields vary (tasting notes, entry title).

### How Presets Work

Each module maintains its own preset list, stored in `~/.pour/presets.json`. Presets are per-module — a coffee preset won't appear in the journal form. The phone PWA ([[Guide-Phone-Capture]]) reads and writes the same file.

At the top of every module form, Pour shows a preset row:

```
▸ Preset: ◂ <none> ▸
```

When you apply a preset, Pour resets __all non-excluded fields__ to the preset's saved values. Fields absent from the preset receive their configured `default`. This is deterministic: applying the same preset always produces the same starting state.

### TUI Keybindings

| Key | Action |
|-----|--------|
| `Left` / `Right` on the preset row | Cycle through saved presets (only when the module has no `preset_axes`) |
| `s` on the preset row or the submit button, or `Ctrl+S` from any field | Save the current field values as a preset |
| `d` on the preset row | Delete the selected preset (asks `y/n`) |
| `Ctrl+Left` / `Ctrl+Right` on the preset row | Reorder presets (move selected preset left or right) |
| `p` on the preset row, or `Ctrl+P` from any field | Open the drilldown picker (needs `preset_axes`) |

The preset row is always at the top of the form. Navigate to it with `Up` from the first field, or `Down` from the preset row to enter the form.

### Preset Names and Descriptions

Each preset has a required `name` and an optional `description`. The name is what you cycle through on the preset row. The description is a short sentence that renders as a dim subtitle under the preset name when that preset is selected — useful for disambiguating similar presets (e.g., two espresso recipes on the same machine).

When you press `Ctrl+S` to save, the overlay shows two inputs:

```
Name: V60 - KUltra
Desc: Afternoon pour over — standard 1:15 ratio
```

Use `Tab` (or `Up`/`Down`) to switch between the name and description inputs. `Enter` saves; `Esc` cancels. Saving under a name that already exists asks before overwriting it. The description is optional — leave it blank and nothing extra is written to `presets.json`. Legacy preset files (no `description` key) continue to load unchanged.

For an example `presets.json`, see `resources/presets.json` in the repo.

### Drilldown Picker

Cycling a flat list stops working past a dozen presets. Set `preset_axes` on the module to an ordered list of field names, and Pour groups presets by those fields' values. `p` opens a picker that drills down one axis at a time, and the save dialog suggests a name built from the axis values.

```toml
[modules.coffee]
preset_axes = ["brew_method", "brewer"]
```

See [[pour-preset-hierarchy]] for the full behavior.

### When to Use Presets

Presets pay off most for create-mode modules with many fields that rarely change. A coffee log is the canonical case: bean, grinder, dose, yield, and time are consistent for a given setup, while tasting notes differ every brew. Save one preset per common setup (e.g., "V60 light roast", "Espresso morning shot") and cycle between them at the top of the form.

Append-mode modules with short forms (journal, task) typically don't need presets.

### Configuring Preset Exclusion

See the `preset_exclude` key under [section 4](#4-design-your-fields). Mark fields like `textarea` body content and unique entry titles with `preset_exclude = true` so they aren't overwritten when a preset is applied.

---

## 11. Worked Example: Adapting to a New Vault

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
config_version = "0.4.0"
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
field_type = "text"   # sub_fields take text, number, or static_select only
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

## 12. Validation and Testing

After editing your config, test it:

```bash
pour                        # opens dashboard; config errors print and exit
pour <module_name>          # test a specific module form
```

From a source checkout, `cargo run` and `cargo run -- <module_name>` do the same. Pour validates the whole config on load and lists every problem at once. Once the config loads, the dashboard opens with a warnings overlay if a `dynamic_select` source folder is missing, or if an append or update target is missing and its path has no date or field tokens.

Common errors:
- __"dynamic_select requires 'source'"__: `dynamic_select` is missing `source` path
- __"static_select requires 'options'"__ or __"static_select 'options' must not be empty"__: `static_select` has no options
- __"append mode requires 'append_under_header'"__: an append module has no target heading
- __"path must be vault-relative"__: path starts with `/`, `C:\`, or `\\`; a path containing `..` gets __"path must not contain '..' traversal"__
- __"Circular show_when dependency detected"__: field A depends on B which depends on A
- __"create_template references unknown template"__: `create_template` names a template that doesn't exist in `[templates]`
- __"path must contain the {{name}} placeholder"__: a template path has no `{{name}}`
- __"is not valid on update mode modules"__: an update module uses a key or field shape that belongs to another mode
- __"Config version … is not supported by this version of Pour"__: `config_version` is newer than your build; update Pour

---

## Checklist: New Module

- [ ] Decide mode: `append` (add to existing note), `create` (new file), or `update` (change properties on an existing note)
- [ ] Set `path` using vault-relative path with strftime tokens and/or `{{field}}` interpolation
- [ ] For append: set `append_under_header` matching an exact heading in the target note
- [ ] For append: design `append_template` matching the note's existing format
- [ ] For update: name each field exactly like the frontmatter key it changes
- [ ] Define fields top-to-bottom: selectors → conditional → universal → wrap-up
- [ ] For dynamic_selects: verify source folders exist in vault with `.md` files
- [ ] For allow_create: add `[templates.<name>]` section with path and fields
- [ ] For Templater coordination: set `post_create_command = "templater:run"` and create matching Obsidian template
- [ ] Set `icon` on the module for dashboard display (and create-mode frontmatter)
- [ ] Set `icon` on key fields for form display
- [ ] Mark notes/textarea fields with `preset_exclude = true` if they shouldn't be part of presets
- [ ] Add module to `module_order` for dashboard positioning
- [ ] Test with `pour <module>`
