# POUR.md — AI Agent Reference

Reference for AI coding agents operating on a user's Pour install. Covers the config schema, preset format, file layout, and common pitfalls. Keep this file alongside the binary in the user's install directory, or point the agent at its location.

## What Pour is

Pour is a terminal-native capture tool that logs structured data into an Obsidian vault. It runs entirely off a TOML config (`~/.pour/config.toml`) — every module, field, path, and template is declared there. The binary has no hardcoded knowledge of specific modules. Each `[modules.<name>]` block becomes a `pour <name>` command; `pour` with no arg opens a dashboard.

Output paths: Pour writes to the vault via the Obsidian Local REST API when available, falling back to direct filesystem writes when it's not. Behavior is identical from the user's perspective.

## File locations

| Path | Purpose |
|------|---------|
| `~/.pour/config.toml` | Main config (modules, fields, templates, vault connection) |
| `~/.pour/secrets.toml` | API key storage (sibling of config.toml) |
| `~/.pour/cache/state.json` | Dynamic-select options cache (auto-managed) |
| `~/.pour/presets.json` | Saved per-module field-value presets |

Overrides:

- `POUR_HOME` — override the `~/.pour/` directory entirely
- `POUR_CONFIG` — override the config file path (ignores `POUR_HOME` for config only)
- `POUR_API_KEY` — override the Obsidian REST API bearer token (highest precedence)

API-key precedence: `POUR_API_KEY` env var > `secrets.toml` > `config.toml [vault].api_key`. Writing `api_key` into `config.toml` still works but Pour auto-migrates it to `secrets.toml` on next load.

## Top-level config structure

```toml
config_version = "0.2.0"                           # schema version
module_order = ["me", "note", "coffee"]             # dashboard display order

[vault]
base_path = "/abs/path/to/vault"                    # required, absolute
api_port = 27124                                    # optional, default 27124
api_key = "..."                                     # prefer secrets.toml
date_format = "%Y%m%d"                              # strftime for {{date}} token

[modules.<name>]                                    # one per `pour <name>` command
# ... (see Modules section)

[templates.<name>]                                  # referenced via create_template
# ... (see Templates section)
```

## Modules

Two modes:

- **`mode = "append"`** — adds a line/block under a heading in an existing file. Required: `append_under_header`. Usually paired with `append_template` to format the output line.
- **`mode = "create"`** — writes a new file per entry. `path` typically uses `%Y%m%d` or `{{field}}` interpolation to produce unique filenames.

Module keys:

| Key | Required | Notes |
|-----|----------|-------|
| `mode` | yes | `"append"` or `"create"` |
| `path` | yes | Vault-relative. strftime tokens (`%Y %m %d %H %M %S`), `{{date}}`, `{{time}}`, `{{field_name}}` |
| `display_name` | no | Dashboard label; defaults to module key |
| `icon` | no | Emoji shown on dashboard. In create mode, also written to output frontmatter |
| `append_under_header` | append only | Heading text to append under (e.g. `"## Log"`) |
| `append_template` | append only | Output line template. Supports `{{time}}`, `{{date}}`, `{{callout}}`, `{{field_name}}` |
| `append_shallow` | no | If `true`, append_under_header matches any heading depth (e.g. `### Tasks` under `## Today`) |
| `callout_type` | no | Default Obsidian callout type resolved as `{{callout}}` in templates |
| `daily_link` | no | Create mode: inserts a wikilink to today's daily note at the top of the created file |

## Field types

Six types. Each field lives in `[[modules.<module>.fields]]` array of tables.

| Type | Default target | Notes |
|------|----------------|-------|
| `text` | frontmatter | Single-line input |
| `textarea` | body | Multi-line, opens an editor overlay. Supports field-level `callout` to wrap output in `> [!type]` |
| `number` | frontmatter | Digits/`.`/`-` only. Written unquoted if parseable as YAML number |
| `static_select` | frontmatter | Dropdown with `options`. Supports `allow_create` to extend options at runtime |
| `dynamic_select` | frontmatter | Dropdown from vault directory contents (`source`). Supports `allow_create`, `create_template`, `post_create_command` |
| `composite_array` | frontmatter | Tabular input; `sub_fields` defines columns. Sub-fields restricted to text/number/static_select |

Target override: any field can force routing via `target = "frontmatter"` or `target = "body"`.

## Field keys (common)

| Key | Applies to | Notes |
|-----|-----------|-------|
| `name` | all | Required. YAML key in output |
| `field_type` | all | Required |
| `prompt` | all | Required. TUI label |
| `required` | all | Block submit if empty (hidden fields are skipped) |
| `default` | all | Pre-filled value |
| `target` | all | `"frontmatter"` or `"body"` |
| `icon` | all | Emoji shown next to prompt (cosmetic only) |
| `preset_exclude` | all | Exclude from preset capture/apply (useful for freeform textareas, unique titles) |
| `show_when` | all | Conditional visibility. See below |
| `options` | static_select | Required. `allow_create = true` permits extension |
| `source` | dynamic_select | Required. Vault-relative dir path; no `..` / absolute / UNC |
| `sub_fields` | composite_array | Required. Array of column definitions |
| `wikilink` | text / static_select / dynamic_select | Wrap output in `[[...]]` |
| `list` | text / static_select / dynamic_select | Split `", "`-separated values into YAML sequence. Combines with `wikilink` |
| `callout` | textarea | Wrap body output in `> [!type]` callout |
| `callout_title` | textarea with `callout` | Default title on the `[!type]` line. User can override per entry with `t` key |
| `allow_create` | static_select / dynamic_select | Accept novel values; persist back to config (static) or vault (dynamic) |
| `create_template` | dynamic_select with `allow_create` | References `[templates.<name>]` for sub-form inline creation |
| `post_create_command` | dynamic_select with `create_template` | Obsidian command ID fired after creation (e.g. `"templater:run"`). REST-only; no-op on filesystem transport |

## Conditional visibility (`show_when`)

Any field can be gated on the value of another field in the same module:

```toml
show_when = { field = "brew_method", equals = "Espresso" }
# or
show_when = { field = "brew_method", one_of = ["Pour Over", "Immersion"] }
```

Rules:

- Exactly one of `equals` or `one_of` — never both, never neither
- Hidden fields: skip validation (even `required`), exclude from output, resolve to empty in template placeholders
- `show_when.field` must name an existing field in the same module
- A field cannot reference itself; circular chains (A→B→A) are rejected at load
- Controller cannot be a `composite_array` field
- Forward references (referencing a field defined later in the array) are allowed
- Case-sensitive matching. No AND/OR, no negation operators

## Templates (inline creation)

Templates define the schema of notes created via `dynamic_select` + `allow_create` + `create_template`. When a user types a novel value, Pour opens a sub-form to collect the template's fields, then writes the new note with full YAML frontmatter.

```toml
[templates.bean]
path = "Coffee/Beans/{{name}}.md"   # must contain {{name}}; no `..` traversal

[[templates.bean.fields]]
name = "roaster"                    # cannot be "date" or "name" (reserved)
field_type = "text"                 # text / number / static_select only
prompt = "Roaster"
```

Auto-generated frontmatter keys: `date` (today) and `name` (the typed value). Template fields follow.

Constraints:

- Sub-form field types restricted to `text`, `number`, `static_select`
- Template `static_select` fields support `allow_create`; novel values persist back to the template's `options` array
- Template `path` must contain `{{name}}` and must not contain `..`
- `post_create_command` only valid when `create_template` is set
- Referenced template names in `create_template` must exist in `[templates]`

## Presets (`presets.json`)

User-savable shortcuts for common field-value combos. Stored separately from `config.toml` so they don't pollute the schema.

Shape:

```json
{
  "modules": {
    "<module_name>": [
      {
        "name": "Display name",
        "description": "Optional one-liner",
        "values": {
          "<field_name>": "<string value>"
        }
      }
    ]
  }
}
```

Behavior:

- Fields marked `preset_exclude = true` in config are omitted on save and unchanged on apply
- Values are always strings (matches TUI input). Number fields coerce at apply time
- Fields not present in `values` keep the form's current value when the preset is applied
- Safe to hand-edit — Pour re-reads on form open

## Common pitfalls

- **`base_path` must be absolute.** Relative paths fail silently in filesystem fallback mode.
- **`append_under_header` must match the file's actual heading exactly.** Append mode does not create missing headings; it aborts. Use `append_shallow = true` to match regardless of depth.
- **`dynamic_select` with no `source` dir present** — falls back to cache, then freetext. If the agent is adding a new `dynamic_select`, make sure the source dir exists in the vault first (or expect freetext on first run).
- **`show_when` referencing an empty field** — hidden. Empty string does not match `equals = ""` (that's a validation error anyway).
- **Template `{{name}}` sanitization** — Windows-reserved filenames (`CON`, `NUL`, `COM1`–`COM9`) and cross-platform invalid chars are replaced with `-`. Don't rely on exact `{{name}}` round-tripping into the filename.
- **Moving `api_key` into `config.toml`** — works, but Pour will migrate it into `secrets.toml` on next load. Write to `secrets.toml` directly if you're generating config programmatically.
- **Per-platform path separators** — Pour accepts forward slashes in paths on all platforms and normalizes them internally. Use `/` in TOML regardless of OS.
- **`composite_array` sub-fields** can't use `show_when` and can't be nested. Only `text`, `number`, `static_select` allowed inside.
- **Changing `config_version`** — supported majors only. Downgrading past a breaking bump fails at load.

## Validating a config

No `pour --check` flag (v1). The cheapest check is `pour <module> --help` or opening the dashboard — both parse and validate the full config, surfacing errors to stderr with specific field paths. If the dashboard opens without error, the config is valid.

## Runtime API for agents and remote clients

The capture-time surface above (config edits, file inspection) is *one* way for an agent to operate on a Pour install. The other is the runtime HTTP API exposed by `pour serve`.

**When to use which:**

- **Edit `config.toml` / `presets.json` directly** when the user is asking you to add a module, change a field, define a template, or curate presets. These are schema changes — they belong in the file, not behind an API call. The rules in this document govern.
- **Use the HTTP API** when the user is asking you to *capture* something ("log that I just had a V60 with the Onyx Ethiopia at 1:15"), query their history, or retrieve a prior capture. These are runtime operations against a running daemon.

The HTTP API is a complete agent surface: schema discovery (`GET /api/v1/config`), valid-choices for dynamic dropdowns (`GET /api/v1/options/{module}/{field}`), idempotent submits with offline-time-preserving `captured_at` (`POST /api/v1/submit/{module}`), read-back of any prior capture (`GET /api/v1/captures/{history_id}`), and history queries (`GET /api/v1/history`). Auth is a Bearer token from `~/.pour/secrets.toml` `mobile_token` field.

A hand-written OpenAPI 3.1 spec ships at `pour - docs/02 references/pour-openapi.yaml` (Phase 1) and will be auto-generated from the Rust handler signatures via `utoipa` in Phase 2. An MCP companion (`pour mcp`) is on the Phase 4 roadmap.

**Authoritative spec:** `pour - docs/08 specs/pour-api-contract.md`. When in doubt, the contract is canonical over this document for runtime operations.

## Where to find more

Everything in this file is derived from the full reference docs in the (Pour)[https://github.com/mads-jm/pour] source repo:

- `pour - docs/02 references/field-types.md` — authoritative field-type reference, including edge cases and output examples
- `pour - docs/02 references/pour-openapi.yaml` — machine-readable runtime API spec (Phase 1: hand-written; Phase 2: `utoipa`-generated)
- `pour - docs/08 specs/pour-design-spec.md` — design vision (aspirational; deviations annotated inline)
- `pour - docs/08 specs/pour-api-contract.md` — runtime HTTP API contract (human narrative)
- `pour - docs/04 architecture/System-Architecture-Overview.md` — subsystem map

When in doubt, field-types.md is canonical for the config schema; pour-api-contract.md is canonical for the runtime API.
