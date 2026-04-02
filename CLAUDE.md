# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Pour** is a terminal-native (TUI) capture tool written in Rust that logs structured data into an Obsidian vault. It acts as a headless data-entry client driven entirely by a TOML config file (`~/.config/pour/config.toml`). Users run `pour` for a dashboard or `pour <module>` (e.g., `pour me`, `pour coffee`) for fast direct entry.

## Build & Development Commands

```bash
cargo build              # compile
cargo run                # run dashboard
cargo run -- coffee      # run a specific module
cargo test               # run all tests
cargo test <test_name>   # run a single test
cargo clippy             # lint
cargo fmt                # format
cargo fmt -- --check     # check formatting without modifying
```

## Architecture

### Hybrid Transport Layer

Pour writes to Obsidian via two paths, falling back automatically:

1. **API** — HTTPS requests via `reqwest` to Obsidian Local REST API (`https://127.0.0.1:27124`) with Bearer token auth (accepts self-signed certs)
2. **File System** — Direct `std::fs` writes to the vault path if the API is unavailable

### Dynamic Data Fetching (3-tier fallback)

For populating dropdowns (e.g., bean list): API query -> disk scan -> `~/.cache/pour/state.json` cache -> freetext input. The TUI renders immediately from cache while async-fetching fresh data in the background.

### File Write Modes

- **Append** (`pour me`): Appends under a header in an existing daily note (API) or creates a standalone atomic note with timestamped filename (filesystem fallback)
- **Create** (`pour coffee`): Generates a new file with YAML frontmatter

### Field-to-Output Mapping

Fields go to YAML frontmatter by default (`text`, `number`, `static_select`, `dynamic_select`). `textarea` fields go to the Markdown body. Each field can override via `target = "frontmatter"` or `target = "body"`. See `pour - docs/02 references/field-types.md` for the full field type reference.

### Config-Driven Design

The app has no hardcoded knowledge of specific modules. All modules, fields, paths, and templates are defined in the user's `config.toml`. See `pour - docs/08 specs/pour-design-spec.md` for the design spec.

## Tech Stack

- **Rust 2024 Edition**
- `ratatui` + `crossterm` (TUI)
- `serde` + `toml` + `toml_edit` + `serde_json` (serialization)
- `reqwest` + `tokio` (async HTTP)
- `chrono` (timestamps/date formatting in file paths)

## Testing

- Tests live in dedicated files under `tests/` mirroring `src/` structure — NOT inline `#[cfg(test)]` blocks
- Example: `src/config.rs` tests are in `tests/config.rs`
- `tempfile` is available as a dev-dependency for filesystem tests
- Use `POUR_CONFIG` env var to point tests at temporary config files

## Documentation

Project documentation lives in `pour - docs/`, an Obsidian vault. **After any task that changes behavior, config schema, or architecture, update the affected docs before considering the task complete.** This includes:

- `pour - docs/08 specs/pour-design-spec.md` — Design spec. Aspirational; annotate deviations inline with `*[Deviation: ...]*` rather than rewriting the vision.
- `pour - docs/04 architecture/System-Architecture-Overview.md` — Subsystem map. Keep in sync with `src/` structure.
- `pour - docs/02 references/field-types.md` — Field types, config keys, validation rules, output targets. Update when adding or changing field types or config schema.
- `pour - docs/09 milestones/v1.0.0-Release.md` — Current release state. Update known limitations as they are resolved.
- `pour - docs/00 index/` — Index files for architecture, specs, references. Link new docs here.
- `README.md` — Config examples and tech stack. Keep in sync with actual schema and dependencies.
- Sprint reports (`pour - docs/06 reports/sprints/`) are **frozen historical records** — do not update them.

Library-specific API references are in `pour - docs/02 references/`.
