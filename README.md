# [Pour](https://pour.madigan.app/)

A terminal-native capture tool that logs structured data into an [Obsidian](https://obsidian.md) vault. Config-driven, keyboard-first, no friction.

## Why

We don't write enough about the things that matter to us. Not because we don't want to - because the friction kills the impulse before we act on it.

Pour exists to close that gap. One command, a few keystrokes, back to what you were doing. If we can capture the thought, the meaning isn't lost to time. A moment, a cup, a song, a passing thought - permanent in your hands.

Write more... pour.

```
pour coffee       # log a brew
pour me           # capture a thought into your daily note
pour todo         # add a task
pour note         # create a fleeting note
pour              # open the dashboard
```

![WindowsTerminal_MrF7aYYLa6](https://github.com/user-attachments/assets/8c658f4f-2b3c-43d5-ada3-8d44b12221c6)

## Install

```bash
# From source
cargo build --release
# Binary is at target/release/pour - put it on your PATH

# Or install directly
cargo install --path .
```

Requires Rust 2024 edition. No other system dependencies - Obsidian Local REST API is optional.

## Quick Start

**1. Create the config file**

```bash
mkdir -p ~/.config/pour
touch ~/.config/pour/config.toml
```

**2. Point it at your vault**

```toml
config_version = "0.2.0"

[vault]
base_path = "/path/to/your/vault"
```

**3. Define a module and run it**

```toml
[modules.todo]
mode = "append"
path = "Daily/%Y%m%d.md"
icon = "✅"
append_under_header = "### Tasks"
append_template = "- [ ] {{body}}"
append_shallow = true

[[modules.todo.fields]]
name = "body"
field_type = "text"
prompt = "Task"
required = true
target = "body"
```

```bash
pour todo
```

## Config Overview

### Modes

Each module uses one of two modes:

| Mode | Behavior |
|------|----------|
| `append` | Appends content under a header in an existing note (e.g. a daily note) |
| `create` | Creates a new file per entry with YAML frontmatter |

### Field Types

- `text` - single-line input
- `number` - numeric input
- `textarea` - multi-line; goes to the Markdown body by default
- `static_select` - fixed options list
- `dynamic_select` - options pulled from your vault (see Transport below)
- `composite_array` - repeatable set of sub-fields (e.g. brew recipe stages)

Fields go to YAML frontmatter by default. Override with `target = "body"` or `target = "frontmatter"`.

### Path Interpolation

Paths support strftime tokens and field name tokens:

```toml
# strftime: %Y, %m, %d, %H, %M, %S
path = "Daily/%Y%m%d.md"

# field token: {{field_name}}
path = "Coffee/{{bean}}@%Y%m%d.md"

# special tokens: {{date}}, {{time}}
path = "Notes/%Y/%m/{{title}}.md"
```

### Conditional Fields (`show_when`)

Gate field visibility on another field's value. Hidden fields are excluded from validation and output.

```toml
[[modules.coffee.fields]]
name = "shot_style"
field_type = "static_select"
prompt = "Shot style"
options = ["Standard", "Turbo", "Ristretto"]
show_when = { field = "brew_method", equals = "Espresso" }

# or match multiple values
show_when = { field = "brew_method", one_of = ["Pour Over", "Immersion"] }
```

### Templates (inline note creation)

When a `dynamic_select` field has `allow_create = true`, typing a novel value opens a sub-form overlay to capture structured frontmatter for the new note before continuing.

```toml
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
source = "Coffee/Beans"
allow_create = true
create_template = "bean"
post_create_command = "templater:run"   # fires an Obsidian plugin command after creation
wikilink = true                         # wraps the value in [[...]]

[templates.bean]
path = "Coffee/Beans/{{name}}.md"

[[templates.bean.fields]]
name = "origin"
field_type = "static_select"
options = ["Ethiopia", "Colombia", "Kenya"]
```

### Presets

Save and recall named field-value sets per module.

| Key | Action |
|-----|--------|
| `Ctrl+S` | Save current form as preset |
| `Ctrl+D` | Delete current preset |
| `Left / Right` | Cycle through saved presets |
| `Ctrl+Left/Right` | Reorder presets |

Fields with `preset_exclude = true` are skipped during save and apply - useful for notes or observations that change every entry.

## Transport

Pour writes to Obsidian via two paths, falling back automatically:

1. **API** - HTTPS to [Obsidian Local REST API](https://github.com/coddingtonbear/obsidian-local-rest-api) at `https://127.0.0.1:27124` with Bearer token auth. Set `api_key` in config or via `POUR_API_KEY` env var.
2. **Filesystem** - Direct `std::fs` writes to `vault.base_path`. Always available.

```toml
[vault]
base_path = "/path/to/vault"
api_port = 27124
api_key = "your-key-here"   # or POUR_API_KEY env var
```

**`dynamic_select` data fallback chain**: API query → disk scan of `source` path → `~/.cache/pour/state.json` → freetext input. The TUI renders immediately from cache while fetching fresh data in the background.

## Tech Stack

| Area | Crate |
|------|-------|
| TUI | `ratatui` + `crossterm` |
| HTTP | `reqwest` + `tokio` |
| Serialization | `serde` + `toml` + `toml_edit` + `serde_json` |
| Time | `chrono` |

## Development

```bash
cargo build
cargo test
cargo clippy
cargo fmt -- --check
```

Tests live in `tests/` mirroring `src/` structure. Use `POUR_CONFIG` env var to point tests at a temporary config file.

## Documentation

Full docs in [`pour - docs/`](pour%20-%20docs/index.md) - design spec, field type reference, architecture overview, and release notes.

## License

MIT - see [LICENSE](LICENSE).
