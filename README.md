<div align="center">

# ▽ pour

**Log the things you care about without leaving the terminal.**

A config-driven capture tool that writes structured Markdown into your [Obsidian](https://obsidian.md) vault, or any folder you point it at.

[![ci](https://github.com/mads-jm/pour/actions/workflows/ci.yml/badge.svg)](https://github.com/mads-jm/pour/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/mads-jm/pour?color=4c1)](https://github.com/mads-jm/pour/releases/latest)
[![license](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

[Website](https://pour.madigan.app/) · [Install](#install) · [Docs](pour%20-%20docs/index.md) · [Changelog](CHANGELOG.md)

</div>

![pour running in a terminal](https://github.com/user-attachments/assets/8c658f4f-2b3c-43d5-ada3-8d44b12221c6)

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

## What a pour looks like

You describe a module once in `~/.pour/config.toml`. After that, `pour coffee` opens a form, you fill it in, and a note lands in your vault:

```markdown
---
date: 2026-10-02
bean: "[[Ethiopia Guji]]"
method: V60
ratio: "1:16"
---

Blueberry up front, tea-like finish. Grind one step finer next time.
```

That's `Coffee/2026-10-02-Ethiopia Guji.md`, written by pour from a short block of config. Fields become YAML frontmatter, so Dataview, Bases, or a ten-line script can query a year of brews later. The tasting notes go to the body. The bean list in the form came from the files in `Coffee/Beans/`, and the bean is written as a wikilink back to its note.

Pour knows nothing about coffee. Everything it knows lives in your config, so the same engine logs a set list, a book, a workout, or the thing you thought of in the shower.

Some captures don't need a form at all:

```bash
$ pour habit water 16
water: 64/96 oz · ✓ 20260805.md
```

## Install

**Prebuilt binary** (recommended)

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/mads-jm/pour/main/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/mads-jm/pour/main/install.ps1 | iex
```

The installer puts `pour` at `~/.local/bin/pour` (Unix) or `%LOCALAPPDATA%\Programs\pour\pour.exe` (Windows), adds it to your PATH, and drops the `resources/` folder next to it (sample configs, presets, an AI-agent reference). To pin a version, run `curl ... | sh -s -- 0.2.2` on Unix, or set `$env:POUR_VERSION = '0.2.2'` before the `irm` line on Windows.

**With cargo**

```bash
cargo install --git https://github.com/mads-jm/pour
# or, from a clone: cargo build --release  →  target/release/pour
```

Needs a Rust toolchain with 2024 edition support.

**Platform notes**

- **Linux** needs ALSA. Pour links `libasound.so.2` for the [completion sound](#and-the-rest), so the binary won't start without it, even with sound off. Desktop distros ship it. On a minimal server or container image, install `libasound2` (`libasound2t64` on Ubuntu 24.04+) or `alsa-lib` on Fedora and Arch. Building from source also needs `pkg-config` and the headers: `libasound2-dev` on Debian/Ubuntu, `alsa-lib-devel` on Fedora.
- **macOS** binaries are Apple Silicon only and need macOS 14.2 or later, because `cpal` (the audio library) does. An older macOS refuses to launch the binary even with sound off. CI compiles every change for macOS, but nobody has run pour on a Mac yet. If you do, please open an issue and say how it went, good or bad.
- **Windows** needs nothing extra.

## Quick start

```bash
pour init
```

`pour init` walks you through creating `~/.pour/` with a `config.toml`, a `secrets.toml`, and a few example modules. Everything pour keeps lives in that folder. Set `POUR_HOME` to move it.

Or write the config by hand. Point it at your vault:

```toml
# ~/.pour/config.toml
config_version = "0.4.0"

[vault]
base_path = "/path/to/your/vault"
```

Add a module:

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

And run it:

```bash
pour todo
```

## Three ways to write

| Mode | What it does |
|------|--------------|
| `create` | A new note per entry, with YAML frontmatter. A brew, a show, a book. |
| `append` | Adds a line under a header in a note that already exists, like your daily note. |
| `update` | Rewrites a few frontmatter properties on an existing note and leaves everything else alone. Pour never creates that note. Your template owns it. |

Which mode a thing belongs in is a design question with a real answer. [Pour Types](pour%20-%20docs/01%20concepts/Pour-Types.md) explains how to tell.

`update` mode is what makes habit tracking a one-liner. Give a module `toggle` and `counter` fields, then call it with a field name:

```toml
[modules.habit]
mode = "update"
path = "Daily/%Y%m%d.md"
icon = "🌱"

[[modules.habit.fields]]
name = "water"
field_type = "counter"
prompt = "Water"
unit = "oz"          # display only, never written to YAML
goal = 96            # renders as 64/96 oz
```

```bash
$ pour habit water 16           # a counter adds; =16 would set it
water: 64/96 oz · ✓ 20260805.md

$ pour habit                    # no field → the usual form
```

If the daily note is missing a key, pour adds it and tells you your template is stale, so the capture still lands. If the note itself is missing, pour fails loudly instead of making one up.

## And the rest

Each of these is optional and documented in the [field and config reference](pour%20-%20docs/02%20references/field-types.md).

- **Eight field types.** `text`, `number`, `textarea`, `static_select`, `dynamic_select` (options read from a vault folder), `composite_array` (repeatable rows, like the stages of a pour-over recipe), `toggle`, and `counter`.
- **Conditional fields.** `show_when` hides a field until another field has a given value. Hidden fields skip validation and stay out of the note.
- **New notes from inside the form.** Type a bean that doesn't exist yet and `allow_create` opens a sub-form, writes `Coffee/Beans/<name>.md` from a template, and carries on with your brew.
- **Presets.** `Ctrl+S` saves the current form, `←`/`→` cycles through saved ones. Set `preset_axes` and `p` opens a picker instead, drilling down by method, then bean. Composite fields keep their own presets, so a favorite recipe can be replayed on its own.
- **Paths that fill themselves in.** strftime tokens, `{{field_name}}`, and `{{slug}}` in the filename: `inbox/%Y%m%d-%H%M%S{{slug}}.md`.
- **Fixed frontmatter per module.** `tags`, `cssclasses`, a custom `date` format.
- **Captures outside the vault.** A module can set its own `base_path` and write to a notes repo or anywhere else on disk.
- **Your past captures beside the form.** The TUI shows the earlier captures from the same module that look most like the one you're filling in, three by default, one column each, with every row beside its form field. A `·` means the capture had what the form holds now, so the differences stand out. `[modules.<name>.priors]` decides what counts as similar. Without the block, pour scores on every select and `wikilink` field. `Ctrl+R` collapses the panel.
- **Post-write hooks.** `post_write_shell` runs a command after a save, for example to commit and push a note.
- **A completion sound.** `[sound] on_save = true` plays one short tone when a capture saves in the TUI. It's off by default, and when it's off pour never opens an audio device.

> [!WARNING]
> `post_write_shell` runs arbitrary commands from your config. Only pour-generated tokens (`{{base_path}}`, `{{rel_path}}`, `{{abs_path}}`, `{{slug}}`, `{{slug_or_time}}`) interpolate. A `{{field_name}}` in a hook is **rejected at load**, so nothing you type into a form can reach a shell. Hooks don't run for phone captures unless you set `post_write_shell_on_serve = true`. And a hook that pushes to a public repo makes a bad capture public history.

Every key the TUI listens to is in [keyboard shortcuts](pour%20-%20docs/02%20references/keyboard-shortcuts.md). For a walkthrough that maps a real vault onto a config, see [Adapting Pour to Your Vault](pour%20-%20docs/03%20guides/Guide-Config-to-Vault.md).

## From your phone

The terminal is the right tool at your desk and the wrong one at the kitchen counter. `pour serve` runs the same engine behind a small web app on your LAN:

```bash
pour serve            # port 8421
pour serve --port 9000
```

It prints a QR code. Scan it, add the page to your home screen, and every module is a tile you can tap. The phone writes exactly the same Markdown the terminal does. It also queues captures while you're offline, shows a 90-day history heatmap, and lets you manage presets. From the dashboard, press `s` to start the server in place, and `Ctrl+C` to drop back into the TUI.

It's LAN-only, guarded by a token pour generates and keeps in `secrets.toml`. For access away from home, Tailscale or ZeroTier work well. Add `mobile_visible = false` to keep a module off the phone entirely. Token rotation, logging, and home-screen icons are covered in [Capturing From Your Phone](pour%20-%20docs/03%20guides/Guide-Phone-Capture.md).

## Do I need Obsidian?

No. Pour writes plain Markdown with YAML frontmatter to a folder. Obsidian reads that well, and so do Logseq, Hugo, Zola, `grep`, and anything else that reads text files.

Without Obsidian, set `base_path` to any folder and skip the API settings. Pour writes straight to disk.

With Obsidian and the [Local REST API](https://github.com/coddingtonbear/obsidian-local-rest-api) plugin, pour writes through the API instead. That gets you faster writes, richer folder listings for dropdowns, and plugin commands like Templater fired after a note is created. Put the key in `~/.pour/secrets.toml` or `POUR_API_KEY`:

```toml
# ~/.pour/secrets.toml  (keep this out of version control)
api_key = "your-key-here"
```

If the API isn't reachable, pour falls back to writing files directly, and it reports which one it used on every save. Dropdowns work the same way. Pour lists the source folder through whichever path is live and caches the result, so if the vault can't be read, the last known list still shows up. With no list at all, the field takes free text. More in [Pour Without Obsidian](pour%20-%20docs/07%20stories/pour_without_obsidian.md).

## Built with

Rust, [ratatui](https://ratatui.rs) and crossterm for the TUI, axum for `pour serve`, reqwest for the Obsidian API.

<details>
<summary>Full dependency list</summary>

| Area | Crate |
|------|-------|
| TUI | `ratatui` + `crossterm` |
| HTTP server | `axum` + `tower` + `tower-http` + `tokio` |
| HTTP client | `reqwest` |
| Static assets | `rust-embed` (PWA shell embedded in the binary at compile time) |
| Serialization | `serde` + `toml` + `toml_edit` + `serde_json` |
| Time | `chrono` |
| URL encoding | `percent-encoding` (vault paths with spaces in REST API URLs) |
| QR codes | `qrcode` (terminal QR code for `pour serve`) |
| LAN address | `local-ip-address` (the LAN-routable address printed at startup) |
| Identifiers | `uuid` (idempotency keys, capture IDs) |
| Constant-time compare | `subtle` (bearer-token comparison) |
| Logging | `tracing` + `tracing-subscriber` (structured server logs, `POUR_LOG`) |
| Unicode width | `unicode-width` (terminal cursor and column accounting) |
| Filesystem paths | `dirs` (finds the home directory for `~/.pour/`) |
| Errors | `anyhow` |
| Shell open | `open` (the `o` key on the summary screen opens the note in Obsidian) |
| Audio | `cpal` plays the completion sound and links ALSA on Linux. `alsa` (Linux only) keeps alsa-lib's diagnostics off the TUI's terminal |

</details>

## Contributing

```bash
cargo build
cargo test
cargo clippy
cargo fmt -- --check
```

Tests live in `tests/`, mirroring `src/`. [CONTRIBUTING.md](CONTRIBUTING.md) covers branching, commit style, and the `just` recipes for working against a live config. The design spec, architecture notes, and decision records live in [`pour - docs/`](pour%20-%20docs/index.md), which is itself an Obsidian vault. Open it in Obsidian and the graph view shows how it fits together.

## License

MIT. See [LICENSE](LICENSE).

<div align="center">

▽

</div>
