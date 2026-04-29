# [Pour](https://pour.madigan.app/)

A terminal-native capture tool that writes structured Markdown to a folder. Config-driven, keyboard-first, no friction. Built for [Obsidian](https://obsidian.md) users, works without it.

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

**Prebuilt binary (recommended)**

Linux / macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/mads-jm/pour/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/mads-jm/pour/main/install.ps1 | iex
```

The installer downloads the latest release from GitHub, places the binary at `~/.local/bin/pour` (Unix) or `%LOCALAPPDATA%\Programs\pour\pour.exe` (Windows), bundles the `resources/` folder alongside (sample configs, presets, AI-agent reference), and adds it to your PATH. Pin a specific version: `curl ... | sh -s -- 0.2.2` on Unix, or `$env:POUR_VERSION = '0.2.2'; irm ... | iex` on Windows.

**From crates.io** (requires Rust toolchain):

```bash
cargo install --git https://github.com/mads-jm/pour
```

**From source**:

```bash
cargo build --release
# Binary is at target/release/pour
```

Requires Rust 2024 edition. No other system dependencies — Obsidian Local REST API is optional.

## Quick Start

**0. Initialize**

```bash
pour init
```

`pour init` creates the default layout at `~/.pour/` — `config.toml`, `secrets.toml`, and example modules — guided by interactive prompts. Run this once before first use. All Pour state lives under `~/.pour/` (override with `POUR_HOME`).

**1. Create the config file (manual alternative)**

```bash
mkdir -p ~/.pour
touch ~/.pour/config.toml
```

All Pour state lives under `~/.pour/` (override with `POUR_HOME`):

```
~/.pour/
  config.toml
  secrets.toml
  presets.json
  field_presets.json
  cache/
    state.json
    history.jsonl
    history-summary.json
```

**2. Point it at your vault**

```toml
config_version = "0.3.0"

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
| `p` / `Ctrl+P` | Open hierarchical picker (when `preset_axes` is configured) |
| `Left / Right` | Cycle through saved presets (only when no `preset_axes`) |
| `Ctrl+Left/Right` | Reorder presets |

Fields with `preset_exclude = true` are skipped during save and apply - useful for notes or observations that change every entry.

#### Preset Hierarchy (drilldown picker)

When you have many presets organised by brewer × bean × intent, add `preset_axes` to the module and press `p` to drill through them one level at a time instead of cycling linearly.

```toml
[modules.coffee]
mode = "create"
path = "Coffee/log.md"
preset_axes = ["method", "bean"]   # drilldown order: Method → Bean → preset list
```

Inside the picker: `↑↓` navigate, `Enter` drill/apply, `Backspace`/`←` pop back, `Esc` cancel. The name field is auto-filled with `<method> · <bean>` when saving a new preset.

### Per-field presets (composite_array)

Composite-array fields like `recipe` or `pressure_profile` carry their own preset list, scoped per `module.field`, so a "Hoffmann 4:6" recipe can be replayed without touching the rest of the form. Presets live in `~/.pour/field_presets.json` and only surface inside the composite editor overlay.

| Key (inside the composite overlay) | Action |
|-----|--------|
| `s` | Save current rows as a named preset |
| `l` | Open the load picker (Up/Down · Enter · `Ctrl+D` delete · Esc) |
| `p` | Quick-cycle to the next saved preset |

Apply replaces the current rows silently. If `sub_fields` were added or removed since the preset was saved, rows are padded or truncated to match the current schema.

## Transport

Pour writes to Obsidian via two paths, falling back automatically:

1. **API** - HTTPS to [Obsidian Local REST API](https://github.com/coddingtonbear/obsidian-local-rest-api) at `https://127.0.0.1:27124` with Bearer token auth. Set `api_key` in `~/.pour/secrets.toml` or via `POUR_API_KEY` env var.
2. **Filesystem** - Direct `std::fs` writes to `vault.base_path`. Always available.

```toml
# ~/.pour/config.toml
[vault]
base_path = "/path/to/vault"
api_port = 27124
```

```toml
# ~/.pour/secrets.toml  (keep out of version control)
api_key = "your-key-here"   # or POUR_API_KEY env var
```

**`dynamic_select` data fallback chain**: API query → disk scan of `source` path → `~/.pour/cache/state.json` → freetext input. The TUI renders immediately from cache while fetching fresh data in the background.

## Do I Need Obsidian?

No. Pour writes plain Markdown files with YAML frontmatter to any directory. Obsidian is one excellent consumer of those files, but anything that reads Markdown works — Logseq, Dendron, Hugo, Zola, `grep`, a 10-line Python script.

**Without Obsidian**: set `base_path` to any folder, skip the `api_key` and `api_port` settings. Pour writes files directly to disk. You get structured, queryable Markdown — no plugins, no proprietary format.

**With Obsidian**: you get the REST API transport (faster writes, richer directory listing), `[[wikilink]]` support, and post-create plugin commands. These are opt-in — if you don't configure them, none of that code runs.

A few config options are Obsidian-flavored (`wikilink = true`, `post_create_command`), but they're all opt-in. The core — config, TUI, form engine, presets, YAML output — is vault-agnostic.

For the full story, see [Pour Without Obsidian](pour%20-%20docs/07%20stories/pour_without_obsidian.md).

## Capturing from your phone

The terminal is the right answer at your desk. It's the wrong answer at the kitchen counter or away from the laptop. `pour serve` is the second front door — same engine, same vault, same Markdown output, accessible from any browser on your LAN.

```bash
pour serve            # default port 8421
pour serve --port 9000
```

On startup, a QR code prints to the terminal alongside the raw URL. Scan it with your phone. The URL carries your auth token as a query param for the first-visit bootstrap; after that the PWA stores the token client-side and sends it as a header.

The PWA lists your modules as tappable tiles. Tap a module, fill the form, submit. The capture path is:

```
Phone → POST /api/v1/submit/:module → axum → engine → vault
```

Identical to what the TUI writes. The vault never knows or cares which front door was used.

**Add to Home Screen** — on iOS and Android the browser offers an "Add to Home Screen" prompt. This gives the PWA an app-like icon and launch behavior; no app store involved.

**PNG icons.** The PWA ships a vector `web/icon.svg`. Android and iOS also accept raster PNGs for the home screen icon. ImageMagick is not bundled with the project; to generate them yourself run:

```bash
magick web/icon.svg -resize 180x180 web/icon-180.png
magick web/icon.svg -resize 192x192 web/icon-192.png
magick web/icon.svg -resize 512x512 web/icon-512.png
```

The Rust server embeds everything under `web/` and serves files under `web/` at `/static/{filename}`. Place PNGs in `web/` and they will be served at `/static/icon-{size}.png` automatically.

**LAN-only by design.** The server binds `0.0.0.0:<port>` — reachable from any device on the same network, not exposed to the internet. Off-LAN access is your responsibility. [Tailscale](https://tailscale.com) and ZeroTier are both effective zero-config options.

**Auth.** A `mobile_token` is generated on first `pour serve` run and written to `~/.pour/secrets.toml`. All token comparisons are constant-time; the query-param bootstrap is only accepted on first contact. Rotate by deleting the key from `secrets.toml` — the next `pour serve` generates a new one and prints a new QR code.

**Logs.** `pour serve` emits structured, level-filtered logs to stderr. The default level is `info`. Control it with the `POUR_LOG` env var:

```bash
POUR_LOG=debug pour serve            # verbose — all targets
POUR_LOG=pour=debug,tower_http=warn  # fine-grained
```

What is logged at `info`: server startup (bind address, transport mode, vault path), every API request/response (method, URI, status, latency), auth outcomes (`accepted_via_query`, `rejected`), and submit results (module name, vault path, autocreate count). What is **never** logged: token values, request bodies, field values, or any user content.

**Per-module opt-out.** Add `mobile_visible = false` to any module section to hide it from the phone entirely. The PWA cannot see or submit to hidden modules.

```toml
[modules.secret]
mobile_visible = false
```

**Today vs. Phase 2.** Phase 1 (shipped): module list, form rendering for all field types, submit, history list. Phase 2 (deferred): offline queue via IndexedDB, service worker app-shell cache, sub-form overlay for `create_template` fields.

## Tech Stack

| Area | Crate |
|------|-------|
| TUI | `ratatui` + `crossterm` |
| HTTP server | `axum` + `tokio` |
| HTTP client | `reqwest` |
| Static assets | `rust-embed` — PWA shell embedded in the binary at compile time |
| Serialization | `serde` + `toml` + `toml_edit` + `serde_json` |
| Time | `chrono` |
| URL encoding | `percent-encoding` — encodes vault paths with spaces in REST API URLs |
| Shell open | `open` — opens notes in Obsidian via the `o` key on the summary screen |

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
