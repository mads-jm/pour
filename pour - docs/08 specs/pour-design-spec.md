---
tags:
  - architecture
  - design
  - spec
aliases:
  - design spec
  - pour spec
date created: Tuesday, March 31st 2026, 12:14:29 am
date modified: Friday, April 3rd 2026, 4:11:41 am
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

#### Inline Creation (`allow_create`)

When `allow_create = true` on a `dynamic_select` field, the dropdown enters search mode as the user types — filtering options by case-insensitive substring match and showing a "Create new" affordance for unmatched text. Submitting a novel value auto-creates a bare note at `{source}/{sanitized_value}.md` (via the transport layer) before the module output is written. The new entry is appended to the in-memory cache immediately. *[Deviation: the original spec described dynamic_select as a closed list only; inline creation was added post-spec.]*

#### Template-Driven Creation (`create_template` + `post_create_command`)

When a `dynamic_select` field has `create_template` referencing a `[templates.<name>]` section, novel values trigger a __sub-form overlay__ instead of bare stub creation. The overlay prompts the user for template-defined fields (text, number, static_select), then writes a note with full YAML frontmatter. *[Deviation: not in original spec. Added to support richer inline-created notes without leaving the TUI.]*

An optional `post_create_command` fires an Obsidian plugin command (e.g. `templater:run`) via the REST API's `/commands/` endpoint after note creation. This bridges Pour's structured data capture with Obsidian's plugin ecosystem — Pour handles frontmatter, the plugin handles body/presentation. The command is best-effort: silently skipped on filesystem transport. *[Deviation: command execution via REST API was not in original spec.]*

### __3.3 Conditional Field Visibility (`show_when`)__

*[Deviation: not in original spec. Added to support method-specific form layouts (e.g., espresso-only fields that are meaningless for pour-over).]*

Any field can declare a `show_when` block that gates its visibility on the value of another field in the same module:

```toml
show_when = { field = "brew_method", equals = "Espresso" }
# or match multiple values:
show_when = { field = "brew_method", one_of = ["Espresso", "Moka"] }
```

The visibility computation lives in `src/visibility.rs` (`is_field_visible`, `visible_field_indices`) — pure functions called on every key event. Hidden fields are excluded from TUI rendering, navigation, validation, and output. Hidden `required` fields do not block submit. Hidden field values are cleared on submit.

Config validation enforces: exactly one of `equals`/`one_of`, no self-reference, no `composite_array` controllers, no circular chains. Forward references are allowed.

v1 limitations: no AND/OR combinators, no negation, case-sensitive matching only, not supported on `composite_array` sub-fields.

### __3.4 File Write Modes & Field → Output Mapping__

Append vs. create modes, and how fields map to frontmatter/body.

#### Wikilink Output (`wikilink`)

When `wikilink = true` on a `text`, `static_select`, or `dynamic_select` field, the output value is wrapped in Obsidian wikilink syntax (`[[value]]`) before being written to frontmatter. This creates graph edges between the current note and the named note. For comma-separated multi-values, each item is wrapped individually. *[Deviation: wikilink wrapping was not in the original field spec; added alongside inline creation.]*

## __4. Configuration, Field Types & Validation__

Full TOML schema, field type reference, and validation rules — see config schema section.

### __4.1 `config_version`__

An optional top-level string field in `config.toml` that declares the schema version the file was written against.

```toml
config_version = "0.2.0"
```

- __Format:__ Semver string (e.g. `"0.2.0"`). Non-semver values are rejected at config load.
- __Default:__ When absent, Pour treats the file as `"0.1.0"` — all existing configs without this field continue to work unchanged.
- __Validation:__ Unsupported major versions are rejected with a clear error. All versions with major version `0` are currently accepted (e.g., `0.1.0`, `0.2.0`). The current version is `0.2.0`.
- __Purpose:__ Enables forward migration paths as the config schema evolves — Pour can detect the file's declared version and apply any necessary transformations before parsing. *[Deviation: no migration/transformation logic exists yet — version is validated but not used for schema migration.]*

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
- `allow_create` on `dynamic_select` fields — inline creation with freetext filtering, "Create new" affordance, and auto-created bare notes
- `wikilink` on `text`, `static_select`, and `dynamic_select` fields — wraps output in `[[...]]` for Obsidian graph connectivity
- Configurable theme (accent color, border style) *[Deviation: not implemented in v1 — all styling is inline via ratatui's Style builder.]*
- Post-execution summary view
- `required` field validation

The following are explicitly __in scope__ for v0.2:

- Template-driven inline creation with sub-form overlay (`create_template` + `[templates]`)
- Post-creation command hook (`post_create_command`) for Obsidian plugin integration
- Command execution via REST API transport (`/commands/{commandId}/`)
- Conditional field visibility (`show_when`) — gates field rendering, navigation, validation, and output on another field's value

The following are explicitly __deferred__:

- `pour music` module (generic config supports it when ready)
- Rich validation (min/max, regex)
- Tag-based dynamic_select sources
- Plugin/extension system
- Nested templates / recursive sub-forms
- Dynamic data sources in template fields (only static_select, not dynamic_select)
- TUI configure screen support for `create_template` / `post_create_command` fields








