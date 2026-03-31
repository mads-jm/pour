# Pour

A blazing-fast, terminal-native (TUI) capture tool that logs structured data directly into an [Obsidian](https://obsidian.md) vault. Built in Rust with `ratatui`.

> *Pour is not a workspace. It is a reflex.*

## Why

If the process of logging a thought takes longer than the thought itself, the thought dies. Opening a GUI, waiting for Electron to render, navigating folders, formatting frontmatter — that isn't friction, it's a barrier. Pour eliminates it.

- `pour coffee` — log a brew with bean, dose, yield, and tasting notes
- `pour me` — capture a thought into your daily note
- `pour` — open the dashboard

Capture first, synthesize later. You pour the raw data into the vault flawlessly formatted. Open Obsidian on Sunday to make sense of it all.

## Features

- **Hybrid Transport** — writes via the [Obsidian Local REST API](https://github.com/coddingtonbear/obsidian-local-rest-api) when available, falls back to direct filesystem writes when the vault is closed
- **Config-Driven** — no hardcoded modules; all fields, paths, and templates are defined in `~/.config/pour/config.toml`
- **Dynamic Dropdowns** — populates select fields from your vault (API query -> disk scan -> cache -> freetext fallback)
- **Strict Output** — generates YAML frontmatter and Markdown compatible with Obsidian Properties, Bases, and Dataview
- **Instant Boot** — terminal-native, no Electron, no GUI

## Quick Start

```bash
# Build
cargo build

# Run the dashboard
cargo run

# Run a specific module
cargo run -- coffee
```

### Configuration

Pour is driven by `~/.config/pour/config.toml`. See the [design spec](pour%20-%20docs/04%20architecture/pour-design-spec.md) for the full schema.

```toml
[vault]
base_path = "/path/to/your/vault"
api_port = 27124

[modules.coffee]
mode = "create"
path = "02-Logbook/Beans/%Y-%m-%d_%H%M_{method}.md"

[[modules.coffee.fields]]
name = "bean"
type = "dynamic_select"
prompt = "Bean"
source = "vault://02-Logbook/Beans/"
```

## Tech Stack

| Area | Crate |
|------|-------|
| TUI | `ratatui` + `crossterm` |
| HTTP | `reqwest` + `tokio` |
| Serialization | `serde` + `toml` + `serde_yaml` + `serde_json` |
| Time | `chrono` |

## Architecture

```
pour <module>
    │
    ├─ config.toml ──► fields, paths, templates
    │
    ├─ Transport
    │   ├─ API (reqwest → Obsidian REST API :27124)
    │   └─ Filesystem (std::fs → vault path)
    │
    └─ Output
        ├─ Create mode → new file with YAML frontmatter
        └─ Append mode → content under header in daily note
```

## Development

```bash
cargo test               # run all tests
cargo clippy             # lint
cargo fmt -- --check     # check formatting
```

## Documentation

Full documentation lives in [`pour - docs/`](pour%20-%20docs/index.md), an Obsidian vault with interconnected design specs, library references, and the project manifesto.
Also accessible [in-browser](https://pour.madigan.app)

## License

MIT
