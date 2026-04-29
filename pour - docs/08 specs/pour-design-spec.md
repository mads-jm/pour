---
tags:
  - architecture
  - design
  - spec
aliases:
  - design spec
  - pour spec
date created: Tuesday, March 31st 2026, 12:14:29 am
date modified: Wednesday, April 29th 2026, 5:31:42 pm
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

__API Authentication:__ The REST API plugin requires a Bearer token. Pour resolves the key in this order (first found wins):

1. `POUR_API_KEY` environment variable.
2. `api_key` key in `~/.pour/secrets.toml` (preferred — keep out of version control).
3. `api_key` field under `[vault]` in `config.toml` (legacy fallback; auto-migrated to `secrets.toml` on first load).

### __3.2 Dynamic Data Fetching & Caching__

3-tier fallback with async background refresh. The three data source tiers are: (1) transport layer (API or FS scan), (2) JSON cache (`~/.pour/cache/state.json`), (3) empty vector — which causes the field to accept freetext. Freetext is a UI mode, not a data source, which is why the count is three rather than four. See also [[The-3-Tier-Data-Fallback]].

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

### __3.4 Module Presets__

*[Deviation: not in original spec. Added to support fast re-entry for repeated captures with the same base configuration (e.g., same bean, method, and dose across a brew series).]*

Per-module named presets let users save the current form's field values and recall them on future runs. Presets are stored in `~/.pour/presets.json` keyed by module name. The hierarchical drilldown picker on top of this is specified in [[pour-preset-hierarchy]].

__Keybindings:__
- `Ctrl+S` — save current form values as a named preset (name input overlay appears)
- `Ctrl+D` — delete the selected preset (y/n confirmation)
- `Left/Right` on the preset selector row — cycle through `<none>` and saved presets
- `Ctrl+Left/Right` — reorder presets

A preset selector row appears at the top of every form. Applying a preset is deterministic: fields present in the preset are populated; fields absent from the preset reset to their config defaults.

__`preset_exclude`__ — a boolean field-level config key (`Option<bool>`, default `false`). When `true`, the field is excluded from both preset capture and preset application. Intended for notes, observations, or any value that changes on every entry. `composite_array` fields are implicitly excluded regardless of this flag.

### __3.5 File Write Modes & Field → Output Mapping__

Append vs. create modes, and how fields map to frontmatter/body.

### __3.6 Mobile Visibility (`mobile_visible`)__

*[Deviation: not in original spec. Added in Step B of the mobile/PWA initiative alongside `/api/v1/config` to support per-module opt-out from the phone interface.]*

An optional boolean key on each module section:

```toml
[modules.secret]
mobile_visible = false   # hides this module from the PWA
```

- __Default:__ `true` — all modules are mobile-visible unless explicitly opted out.
- __Effect:__ When `false`, the module is entirely omitted from the `GET /api/v1/config` response. The PWA cannot see or submit to hidden modules.
- __Persistence:__ Togglable from the module configure screen (Left/Right cycling with `◂ ▸` indicators). Persisted via `update_module_on_disk`.
- __Getter:__ `ModuleConfig::is_mobile_visible()` returns `mobile_visible.unwrap_or(true)` — call sites never need to handle `None` vs `Some(true)` distinctly.

#### Wikilink Output (`wikilink`)

When `wikilink = true` on a `text`, `static_select`, or `dynamic_select` field, the output value is wrapped in Obsidian wikilink syntax (`[[value]]`) before being written to frontmatter. This creates graph edges between the current note and the named note. For comma-separated multi-values, each item is wrapped individually. See [[field-types]] for the full key reference. *[Deviation: wikilink wrapping was not in the original field spec; added alongside [[Inline-Note-Creation|inline creation]].]*

## __4. Configuration, Field Types & Validation__

Full TOML schema, field type reference, and validation rules — see config schema section.

### __4.1 `config_version`__

An optional top-level string field in `config.toml` that declares the schema version the file was written against.

```toml
config_version = "0.3.0"
```

- __Format:__ Semver string (e.g. `"0.3.0"`). Non-semver values are rejected at config load.
- __Default:__ When absent, Pour treats the file as `"0.1.0"` — all existing configs without this field continue to work unchanged.
- __Validation:__ Unsupported major versions are rejected with a clear error. All versions with major version `0` are currently accepted (e.g., `0.1.0`, `0.2.0`, `0.3.0`). The current version is `0.3.0`.
- __Purpose:__ Enables forward migration paths as the config schema evolves — Pour can detect the file's declared version and apply any necessary transformations before parsing. *[Deviation: no migration/transformation logic exists yet — version is validated but not used for schema migration.]*

#### Version History

| Version | Changes |
|---------|---------|
| `0.1.0` | Initial schema. |
| `0.2.0` | Added `mobile_token` to `secrets.toml`, `pour serve` command, `/api/health` endpoint (Step A). |
| `0.3.0` | Added `mobile_visible` module-level key. Bumped alongside `/api/v1/config` (Step B). |

## __5. Technical Stack__

- __Language:__ Rust (2024 Edition)
- __TUI Framework:__ [[ratatui]] + [[crossterm]]
- __Serialization:__ `serde`, `serde_json`, [[toml-serde|toml]], `toml_edit` *[Deviation: `serde_yaml` was originally included but removed — YAML frontmatter uses custom serialization instead. See [[ADR-002-Custom-YAML-Serialization]].]*
- __Network:__ [[reqwest]] (with `tokio` for async fetching)
- __Time:__ [[chrono]] (for file formatting and timestamps)
- __URL encoding:__ `percent-encoding` — encodes vault paths containing spaces in REST API request URLs
- __Shell open:__ `open` — cross-platform crate for opening a file or URL in the system default handler (used for "Open in Obsidian" via `obsidian://` URI)

## __6. Scope — v0.1__

The following are explicitly __in scope__ for v0.1:

- Dashboard with connection status and module menu
- `pour me` (append mode with Templater integration + [[Atomic-Note-Fallback|atomic note fallback]])
- `pour coffee` (create mode with frontmatter generation)
- [[ADR-001-Hybrid-Transport-Layer|Hybrid transport layer]] (API → filesystem fallback)
- [[The-3-Tier-Data-Fallback|Dynamic data fetching]] (API → disk scan → cache → freetext)
- Configurable append templates with `{{callout}}` placeholder resolved from module-level `callout_type`; field-level `callout` wraps textarea body output in `> [!type]` blockquote syntax
- `allow_create` on `dynamic_select` fields — [[Inline-Note-Creation|inline creation]] with freetext filtering, "Create new" affordance, and auto-created bare notes
- `wikilink` on `text`, `static_select`, and `dynamic_select` fields — wraps output in `[[...]]` for Obsidian graph connectivity (see [[field-types]])
- Configurable theme (accent color, border style) *[Deviation: not implemented in v1 — all styling is inline via ratatui's Style builder.]*
- Post-execution summary view
- `required` field validation

The following are explicitly __in scope__ for v0.2:

- Template-driven [[Inline-Note-Creation|inline creation]] with [[Sub-Form-Overlay|sub-form overlay]] (`create_template` + `[templates]`)
- Post-creation command hook (`post_create_command`) for Obsidian plugin integration
- Command execution via REST API transport (`/commands/{commandId}/`)
- [[Conditional-Visibility|Conditional field visibility]] (`show_when`) — gates field rendering, navigation, validation, and output on another field's value

The following are explicitly __deferred__:

- `pour music` module (generic config supports it when ready)
- Rich validation (min/max, regex)
- Tag-based dynamic_select sources
- Plugin/extension system
- Nested templates / recursive sub-forms
- Dynamic data sources in template fields (only static_select, not dynamic_select)
- TUI configure screen support for `create_template` / `post_create_command` fields

## __7. Mobile Capture (PWA Companion)__

*[Deviation: the original product framing was terminal-only. The mobile companion was always directionally correct given the manifesto ("near-zero time between thought and capture") but landed as a separate initiative. See [[ADR-005-PWA-Companion]] for the decision record.]*

The engine is library-shaped. The TUI has always been a thin presentation layer over pure data functions. `pour serve` is a second presentation layer over that same engine — distributed in the same binary, writing the same Markdown.

### __7.1 `pour serve` Subcommand__

```
pour serve            # bind 0.0.0.0:8421
pour serve --port 9000
```

On startup: binds `0.0.0.0:<port>` (LAN-accessible, not loopback), generates a `mobile_token` if absent, writes it to `~/.pour/secrets.toml`, and prints a QR code + raw URL to stdout. The URL carries `?token=<token>` for the bootstrap first-visit. Off-LAN access is the user's responsibility (Tailscale / ZeroTier are orthogonal to Pour's scope).

### __7.2 Auth__

Bearer token stored as `mobile_token` in `~/.pour/secrets.toml`. Two acceptance paths, with explicit precedence:

1. `Authorization: Bearer <token>` header — authoritative. Used by the PWA after first contact.
2. `?token=<token>` query param — bootstrap-only. The QR code URL uses this so the phone can open the app without a manual paste step. Once the PWA has stored the token client-side, it sends the header exclusively — keeping the token out of access logs after first contact.

All comparisons are constant-time (`subtle::ConstantTimeEq`). Plain `==` is forbidden on secret values. Static assets (the PWA shell itself) require no auth — the browser must load the page before it can present a token.

*[Deviation: an earlier draft specified "32 bytes base64-url." The implementation uses `Uuid::new_v4().simple()` — 122 bits of entropy, URL-safe by construction, consistent with `uuid` already being a dependency.]*

### __7.3 Endpoint Surface__

Nine endpoints under `/api/v1/`, all responding with `Cache-Control: no-store`. Full shape definitions: [[pour-api-contract]].

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/health` | Transport status, vault path, server uptime |
| `GET` | `/api/v1/config` | All mobile-visible modules and fields |
| `GET` | `/api/v1/options/:module/:field` | Dynamic select options (3-tier fallback) |
| `POST` | `/api/v1/submit/:module` | Submit a form — full engine reuse |
| `GET` | `/api/v1/captures/:history_id` | Read back a vault file by history ID |
| `GET` | `/api/v1/history` | Recent captures |
| `GET` | `/api/v1/presets/:module` | Saved presets for a module |
| `POST` | `/api/v1/presets/:module` | Save a preset |
| `PUT` | `/api/v1/presets/:module/reorder` | Reorder presets |

The submit handler is the same call sequence as the TUI's `handle_submit` — `autocreate` → `write_create` / `write_append` → `History::record` — driven by JSON request body instead of TUI state.

### __7.4 Embedded PWA__

Vanilla HTML/CSS/JS (~600 LOC), embedded in the binary at compile time via `rust-embed`. Served at `/`. No build step, no `npm`. Static assets carry no auth requirement — the page must load before a token can be presented.

Phase 1 (shipped): module list as tappable tiles, per-module forms rendered dynamically from `/api/v1/config`, form submit with success/error feedback, history list. "Add to Home Screen" gives an app-like icon on iOS and Android.

Why vanilla JS and not a Rust→WASM framework: see [[Rust-WASM-Frontend-Tradeoffs]].

*[Deviation: the plan sketched ~300 LOC. Shipped closer to ~600. The form rendering is fully data-driven from the config JSON, so the delta is mostly defensive handling and mobile UX polish.]*

### __7.5 Offline-Correctness Foundations__

Phase 1 ships two correctness primitives:

- __`Idempotency-Key` header__ (optional, 1–128 bytes): the server maintains an in-memory LRU (capacity 1024) per [[pour-api-contract]] §9. In-flight entries TTL at 60 s; completed entries TTL at 5 min. Duplicate submits within that window return the original response without re-writing to the vault.
- __`captured_at` timestamp__: the submit body may include a client-side ISO 8601 timestamp. If present and parseable, it is used as the entry's timestamp instead of server time — preserving the moment of capture even when connectivity is restored later.

Phase 2 (deferred): offline queue via IndexedDB, service worker app-shell cache, background sync on reconnect. See §6 deferred list.

### __7.6 Module Visibility (`mobile_visible`)__

Described in §3.6. Repeated here for completeness: `mobile_visible = false` in a module section hides it entirely from `/api/v1/config`. The PWA cannot see or submit to hidden modules. Default is `true`. Bumped `config_version` to `0.3.0`.








