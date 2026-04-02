---
tags:
  - architecture
  - design
  - spec
aliases:
  - design spec
  - pour spec
date created: Tuesday, March 31st 2026, 12:14:29 am
date modified: Thursday, April 2nd 2026, 9:18:47 am
---

# Project Pour — Design Specification (v0.2)

## __1. Product Overview__

__Pour__ is a blazing-fast, terminal-native (TUI) capture tool designed to eliminate the friction of logging structured data into Obsidian. Built in Rust (using [[ratatui]]), it acts as a headless data-entry client, allowing users to rapidly "pour" thoughts and coffee logs directly into their vault without breaking their CLI workflow.

__Core Philosophy:__

- __Offline-First Resilience:__ Works with the Obsidian Local REST API, but falls back seamlessly to direct file-system operations if the vault is closed.
- __Snappy & Rhythmic:__ Designed around the `pour` command. Fast muscle memory.
- __Data Integrity:__ Generates strict YAML frontmatter and standard Markdown to ensure 100% compatibility with Obsidian Properties (Bases) and Dataview.

## __2. User Experience & Command Routing__

The application has two primary execution paths:

### __2.1 The Dashboard (`pour`)__

Running the base command opens the main interactive hub.

- __Header:__ Displays vault connection status (🟢 API Connected | 🟡 Direct File Mode).
- __Body:__ Shows a summary of today's stats (e.g., "Poured today: 2 Coffees, 1 Journal"). *[Deviation: not implemented in v1 — dashboard shows module list with connection status only.]*
- __Menu:__ Navigable list to launch specific modules (`me`, `coffee`).

### __2.2 The Fast Path (`pour <module>`)__

Bypasses the dashboard and launches directly into a specific data-entry view.

- `pour me` — Opens the journal appending view.
- `pour coffee` — Opens the coffee logging form.

### __2.3 Post-Execution Summary__

Upon submitting a form, the app does *not* immediately exit. It transitions to a __Summary View__ displaying:

- A success message with the destination file path.
- Options: `[Enter]` Main Menu, `[A]` Pour Another → .., `[Q]` Quit, `[O]` Open file in `$EDITOR`. *[Deviation: `[O]` not implemented in v1 — summary supports Enter, A, and Q only.]*

## __3. Architecture & Data Layer__

### __3.1 Hybrid Transport Layer__

Pour uses a dual-pronged approach to writing data:

1. __Primary (API):__ Attempts a fast local HTTPS request via [[reqwest]] to the [[obsidian-local-rest-api|Obsidian Local REST API]] (`https://127.0.0.1:27124`, accepts self-signed certs). *[Deviation: originally spec'd as HTTP; implementation uses HTTPS with `danger_accept_invalid_certs`.]*
2. __Fallback (File System):__ If the connection is refused, it gracefully falls back to `std::fs` to write directly to the absolute vault path defined in the configuration.

__API Authentication:__ The REST API plugin requires a Bearer token. Pour supports two sources:

- `api_key` field under `[vault]` in `config.toml`.
- `POUR_API_KEY` environment variable (takes precedence over config if set).

### __3.2 Dynamic Data Fetching & Caching__

3-tier fallback (API → disk scan → cache → freetext) with async background refresh.

### __3.3 File Write Modes & Field → Output Mapping__

Append vs. create modes, and how fields map to frontmatter/body.

## __4. Configuration, Field Types & Validation__

Full TOML schema, field type reference, and validation rules — see config schema section.

## __6. Technical Stack__

- __Language:__ Rust (2024 Edition)
- __TUI Framework:__ [[ratatui]] + [[crossterm]]
- __Serialization:__ `serde`, `serde_json`, [[toml-serde|toml]], `toml_edit` *[Deviation: `serde_yaml` was originally included but removed — YAML frontmatter uses custom serialization instead. See [[ADR-002-Custom-YAML-Serialization]].]*
- __Network:__ [[reqwest]] (with `tokio` for async fetching)
- __Time:__ [[chrono]] (for file formatting and timestamps)

## __7. Scope — v0.1__

The following are explicitly __in scope__ for v0.1:

- Dashboard with connection status and module menu
- `pour me` (append mode with Templater integration + atomic note fallback)
- `pour coffee` (create mode with frontmatter generation)
- Hybrid transport layer (API → filesystem fallback)
- Dynamic data fetching (API → disk scan → cache → freetext)
- Configurable append templates with `{{callout}}` placeholder resolved from module-level `callout_type`; field-level `callout` wraps textarea body output in `> [!type]` blockquote syntax
- Configurable theme (accent color, border style) *[Deviation: not implemented in v1 — all styling is inline via ratatui's Style builder.]*
- Post-execution summary view
- `required` field validation

The following are explicitly __deferred__:

- `pour music` module (generic config supports it when ready)
- Rich validation (min/max, regex)
- Tag-based dynamic_select sources
- Plugin/extension system







