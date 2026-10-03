# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Pour** is a terminal-native (TUI) capture tool written in Rust that logs structured data into an Obsidian vault. It acts as a headless data-entry client driven entirely by a TOML config file (`~/.pour/config.toml`). All runtime state lives under `~/.pour/` (override with `POUR_HOME` env var). Users run `pour` for a dashboard or `pour <module>` (e.g., `pour me`, `pour coffee`) for fast direct entry. `pour init` writes a starter config, and `pour serve` runs the LAN server behind the mobile PWA (`web/`, embedded in the binary).

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

`Transport::connect` picks one backend at startup and again on a transport refresh or a vault-settings save; it does not re-choose per write. A module with its own `base_path` root always writes over the filesystem (`Transport::for_module`), since the API can only reach the vault it serves.

### Dynamic Data Fetching (3-tier fallback)

For populating dropdowns (e.g., bean list): directory listing over the active transport (API if connected, otherwise a disk scan) -> `~/.pour/cache/state.json` cache -> freetext input. The fetch is awaited inline before the form opens (ADR-003); there is no background refresh.

### File Write Modes

- **Append** (`pour me`): Inserts under a heading in an existing daily note. Both transports splice in place (API: GET, splice, PUT; filesystem: read, splice, atomic replace). If the note or heading is missing, the write fails; there is no standalone-note fallback.
- **Create** (`pour coffee`): Generates a new file with YAML frontmatter
- **Update** (`pour habit`): Rewrites named frontmatter keys on a note that already exists (API `PATCH` with `Target-Type: frontmatter`, or a guarded single-line filesystem edit: stat before read, never re-emit YAML, atomic replace). Never creates the note. One-shot form skips the TUI: `pour <module> <field> [value]`.

Which mode a signal belongs in is a design rule, not a preference. See `pour - docs/01 concepts/Pour-Types.md`.

### Field-to-Output Mapping

Fields go to YAML frontmatter by default (`text`, `number`, `static_select`, `dynamic_select`, and `composite_array`, which also renders a Markdown table in the body). `textarea` fields go to the Markdown body. `toggle` and `counter` default to frontmatter and are valid in any mode, but are built for `update` mode, which rejects body-target fields. Each field can override via `target = "frontmatter"` or `target = "body"`. See `pour - docs/02 references/field-types.md` for the full field type reference.

### Config-Driven Design

The app has no hardcoded knowledge of specific modules. All modules, fields, paths, and templates are defined in the user's `config.toml`. See `pour - docs/08 specs/pour-design-spec.md` for the design spec.

## Tech Stack

- **Rust 2024 Edition**
- `ratatui` + `crossterm` (TUI)
- `serde` + `toml` + `toml_edit` + `serde_json` (serialization)
- `reqwest` + `tokio` (async HTTP)
- `axum` + `rust-embed` (`pour serve` HTTP server and the embedded PWA)
- `chrono` (timestamps/date formatting in file paths)
- `cpal` (audio output for the opt-in completion sound; links ALSA/libasound on Linux)

## Testing

- Tests live in dedicated files under `tests/` mirroring `src/` structure — NOT inline `#[cfg(test)]` blocks
- Example: `src/config.rs` tests are in `tests/config.rs`
- `tempfile` is available as a dev-dependency for filesystem tests
- Use `POUR_CONFIG` env var to point tests at temporary config files
- Use `POUR_HOME` env var to override the `~/.pour/` state directory in tests or alternate environments. `paths::pour_home()` panics inside integration tests when `POUR_HOME` is unset, so tests that touch state must set it

## Documentation

Project documentation lives in `pour - docs/`, an Obsidian vault. **After any task that changes behavior, config schema, or architecture, update the affected docs before considering the task complete.** This includes:

- `pour - docs/08 specs/pour-design-spec.md` — Design spec. Aspirational; annotate deviations inline with `*[Deviation: ...]*` rather than rewriting the vision.
- `pour - docs/04 architecture/System-Architecture-Overview.md` — Subsystem map. Keep in sync with `src/` structure.
- `pour - docs/02 references/field-types.md` — Field types, config keys, validation rules, output targets. Update when adding or changing field types or config schema.
- `pour - docs/09 milestones/v0.2.0-Foundation.md` — Foundation retrospective (capture loop shipped in v0.2.x). Update known limitations as they are resolved.
- `pour - docs/09 milestones/v1.0.0-Release.md` — Aspirational freeze criteria. Update gates as they close.
- `pour - docs/00 index/` — Index files for architecture, specs, references. Link new docs here.
- `README.md` — Config examples and tech stack. Keep in sync with actual schema and dependencies.
- Sprint reports (consolidated into `pour - docs/06 reports/v0.1.0-report.md`) are **frozen historical records** — do not update them.

Library-specific API references are in `pour - docs/02 references/`.
