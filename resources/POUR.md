# POUR.md — AI Agent Reference

Reference for AI coding agents operating on a user's Pour install. Covers the config schema, preset format, file layout, and common pitfalls. Keep this file alongside the binary in the user's install directory, or point the agent at its location.

## What Pour is

Pour is a terminal-native capture tool that logs structured data into an Obsidian vault. It runs entirely off a TOML config (`~/.pour/config.toml`) — every module, field, path, and template is declared there. The binary has no hardcoded knowledge of specific modules. Each `[modules.<name>]` block becomes a `pour <name>` command; `pour` with no arg opens a dashboard. Other entry points: `pour <module> <field> [value]` writes one value with no TUI (update modules only, see below), `pour init` writes a starter config, and `pour serve [--port N]` (default 8421) runs the HTTP API and mobile PWA.

Output paths: Pour writes to the vault via the Obsidian Local REST API when available, falling back to direct filesystem writes when it's not. Behavior is identical from the user's perspective.

## File locations

| Path | Purpose |
|------|---------|
| `~/.pour/config.toml` | Main config (modules, fields, templates, vault connection) |
| `~/.pour/secrets.toml` | API key storage (sibling of config.toml) |
| `~/.pour/cache/state.json` | Dynamic-select options cache (auto-managed) |
| `~/.pour/cache/history.jsonl` | Capture history for dashboard stats and `pour serve` read-back (auto-managed) |
| `~/.pour/presets.json` | Saved per-module field-value presets |
| `~/.pour/field_presets.json` | Saved row sets for `composite_array` fields, keyed `module.field` |

Overrides:

- `POUR_HOME` — override the `~/.pour/` directory entirely
- `POUR_CONFIG` — override the config file path (ignores `POUR_HOME` for config only)
- `POUR_API_KEY` — override the Obsidian REST API bearer token (highest precedence)
- `POUR_MOBILE_TOKEN`: override the `pour serve` bearer token (otherwise `mobile_token` in `secrets.toml`, generated on first serve)

API-key precedence: `POUR_API_KEY` env var > `secrets.toml` > `config.toml [vault].api_key`. Writing `api_key` into `config.toml` still works but Pour auto-migrates it to `secrets.toml` on next load.

## Top-level config structure

```toml
config_version = "0.4.0"                           # schema version
module_order = ["me", "note", "coffee"]             # dashboard display order

[vault]
base_path = "/abs/path/to/vault"                    # required, absolute
api_port = 27124                                    # optional, default 27124
api_key = "..."                                     # prefer secrets.toml
date_format = "%Y%m%d"                              # strftime for {{date}} in paths and the daily_link target

[vault.platform]                                    # optional per-OS base_path overrides
windows = 'D:\Vault'                                # keyed by std::env::consts::OS

[sound]                                             # optional
on_save = true                                      # tone on a saved TUI capture; default false

[modules.<name>]                                    # one per `pour <name>` command
# ... (see Modules section)

[templates.<name>]                                  # referenced via create_template
# ... (see Templates section)
```

## Modules

Three modes:

- **`mode = "append"`** — adds a line/block under a heading in an existing file. Required: `append_under_header`. Usually paired with `append_template` to format the output line.
- **`mode = "create"`** — writes a new file per entry. `path` typically uses `%Y%m%d` or `{{field}}` interpolation to produce unique filenames.
- **`mode = "update"`**: rewrites named frontmatter keys on a note that already exists (typically today's daily note). It never creates the note and never touches the body. Meant for `toggle` and `counter` habit fields. If the note lacks a key, pour inserts it and prints a notice that the template is stale. A note with no frontmatter block is an error. Rejects `append_under_header`, `append_template`, `append_shallow`, `daily_link`, `frontmatter`, `frontmatter_date_format`, and any field that is `composite_array`, `list = true`, or targets the body.

Rule of thumb from the design: ambient day state (did it, how much so far) is an `update` property on the periodic note; anything with detail worth remembering is a `create` note. Never record the same signal both ways, and never mirror into frontmatter a count that can be derived from event notes.

Module keys:

| Key | Required | Notes |
|-----|----------|-------|
| `mode` | yes | `"append"`, `"create"`, or `"update"` |
| `path` | yes | Vault-relative (or relative to `base_path`, if set). strftime tokens (`%Y %m %d %H %M %S`), `{{date}}` (uses `[vault].date_format`), `{{time}}`, `{{slug}}`, `{{slug_or_time}}`, `{{field_name}}`. Unknown placeholders are stripped. A create-mode path with no extension is treated as a directory and gets a `<date> <HH-MM-SS>.md` filename |
| `display_name` | no | Dashboard label; defaults to module key |
| `icon` | no | Emoji shown on dashboard. In create mode, also written to output frontmatter |
| `append_under_header` | append only | Heading text to append under (e.g. `"## Log"`) |
| `append_template` | append only | Output line template. Supports strftime tokens, `{{time}}` (`HH:MM`), `{{date}}` (always `%Y-%m-%d`, ignores `date_format`), `{{callout}}`, `{{field_name}}`. A multi-line value on a line starting with `>` gets `> ` on each continuation line, so it stays inside the callout |
| `append_shallow` | no | If `true`, the appended section ends at the next heading of any level, not just an equal-or-higher one. Use it when the target heading has sub-headings the entry should land above (e.g. `### Tasks` with `#### Completed` under it) |
| `callout_type` | no | Default Obsidian callout type resolved as `{{callout}}` in templates |
| `daily_link` | no | Create mode: adds a `daily` frontmatter key linking today's daily note, named with `[vault].date_format` (e.g. `daily: "[[20260405]]"`) |
| `mobile_visible` | no | `false` hides the module from the `pour serve` PWA and API (`/api/v1/config` omits it, `/options` and `/submit` treat it as unknown). Default `true` |
| `preset_axes` | no | Ordered field names used as drilldown levels in the preset picker. Absent means presets cycle with Left/Right |
| `[modules.<n>.priors]` | no | Read-only review panel beside the TUI form showing similar past captures. See "Priors panel" below |
| `base_path` | no | Per-module root override. **Absolute only — `~` is NOT expanded.** Resolution: `[modules.<n>.platform][OS]` → `base_path` → `[vault.platform][OS]` → `[vault].base_path`. Such a module always writes via the filesystem transport (the Obsidian API can only reach its own vault) and reports `FileSystem` in the summary. `path` stays root-relative — traversal is still rejected |
| `[modules.<n>.platform]` | no | Per-OS overrides for this module's `base_path`, keyed by `std::env::consts::OS` (`linux`/`macos`/`windows`). Mirrors `[vault.platform]` |
| `[modules.<n>.frontmatter]` | no | Create mode only. Static key→value frontmatter merged **after** captured fields, in alphabetical key order. Arrays → YAML block sequences (even single-element); scalars → scalars. **No token interpolation — values are literal.** A key that collides with a captured field is skipped (the capture wins). `date` is not permitted here |
| `frontmatter_date_format` | no | Create mode only. strftime for the auto-injected `date` key. Absent → `%Y-%m-%d`. Emitted **unquoted**, so a format containing `": "` yields invalid YAML |
| `post_write_shell` | no | Shell command run after a successful write, from `base_path`. Only `{{base_path}}`, `{{rel_path}}`, `{{abs_path}}`, `{{slug}}`, `{{slug_or_time}}` interpolate — **any other token is rejected at load**. Best-effort: a failure or a 30s timeout warns, never loses the note. Runs after TUI saves (and serve captures when opted in), not after one-shot capture |
| `post_write_shell_on_serve` | no | Whether `post_write_shell` also fires for LAN (`pour serve`) captures. **Defaults to `false`.** Requires `post_write_shell` |

## Field types

Eight types. Each field lives in `[[modules.<module>.fields]]` array of tables.

| Type | Default target | Notes |
|------|----------------|-------|
| `text` | frontmatter | Single-line input |
| `textarea` | body | Multi-line, opens an editor overlay. Supports field-level `callout` to wrap output in `> [!type]` |
| `number` | frontmatter | Digits/`.`/`-` only; must parse as a number on submit. **Create mode writes it as a quoted string** (`dose_g: "15"`), from the TUI and `pour serve` alike. Update mode writes it bare |
| `static_select` | frontmatter | Dropdown with `options`. Supports `allow_create` to extend options at runtime |
| `dynamic_select` | frontmatter | Dropdown from vault directory contents (`source`). Supports `allow_create`, `create_template`, `post_create_command` |
| `composite_array` | frontmatter | Tabular input; `sub_fields` defines columns. Sub-fields restricted to text/number/static_select. The frontmatter target also writes a Markdown table to the body. Not valid on update modules |
| `toggle` | frontmatter | Boolean. Space flips it in the TUI. On update modules it is written bare (`cannabis: true`) and seeded from the note when the form opens |
| `counter` | frontmatter | Accumulating number for update modules: `16` adds to the note's current value, `=16` sets it. Missing, empty, or `null` reads as `0`; a non-numeric current value is an error. Takes `unit` and `goal` |

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
| `unit` | counter | Display-only suffix (e.g. `"oz"`); never written to the vault |
| `goal` | counter | Target shown as `64/96`; never written to the vault |

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

## One-shot capture

`pour <module> <field> [value]` writes one value and exits, with no TUI. It only works on `update` modules and on `toggle` or `counter` fields.

```
$ pour habit water 16        # add 16
water: 64/96 oz · ✓ 20260805.md
$ pour habit water =160      # set
$ pour habit cannabis        # toggle on; `false`/`off`/`no`/`0` turns it off
```

An unknown module, unknown field, unsupported field type, bad token, or extra argument exits non-zero before the vault is touched.

## Priors panel

`[modules.<n>.priors]` configures a read-only panel in the TUI form that lists similar past captures from the module's folder. It never writes and never blocks submit. Every key is optional; without the block pour matches on the first wikilink or select field, ranks by recency, shows up to four number/select fields, and lists 5 rows.

```toml
[modules.coffee.priors]
match_on = ["bean", "roaster", "method"]   # most specific first; the front key drops when nothing matches
rank_by  = "rating desc"                   # "<field> desc|asc", "recent", or "none"
show     = ["dose_g", { field = "time_s", agg = "mean" }]   # agg: median (default), mean, max, min, latest
limit    = 5                               # must be > 0
```

All referenced fields must exist on the module. `match_on` object form takes `mode = "equality"` or `"wikilink"`; `"overlap"`, `"window"`, and `rank_by` `max`/`min` are reserved and rejected at load. `Ctrl+R` collapses the panel. Captures are read from the folder part of `path` (before the last `/`) as written, so a token there, as in `Coffee/%Y/...`, is not expanded and the panel finds nothing.

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
- Template `path` must contain `{{name}}` and be vault-relative (no absolute, drive, or UNC path, no `..`)
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
- Values are always strings (matches TUI input), and they are applied as-is
- Applying a preset resets every non-excluded field missing from `values` to its config `default` (or empty). `composite_array` fields are never touched; they have their own presets in `field_presets.json`
- Pour reads the file once at startup (the TUI and `pour serve` each keep their own copy) and rewrites the whole file when it saves a preset. Hand-edit it while Pour is not running, or the next save from a running process overwrites your edit

## Common pitfalls

- **`base_path` must be absolute.** Relative paths fail silently in filesystem fallback mode.
- **`append_under_header` must match the file's actual heading line exactly, `#` marks included** (e.g. `"## Log"`). Append mode does not create missing headings or missing files; it aborts. `append_shallow` does not loosen the match; it only changes where the section ends.
- **`dynamic_select` with no `source` dir present** — falls back to cache, then freetext. If the agent is adding a new `dynamic_select`, make sure the source dir exists in the vault first (or expect freetext on first run).
- **`show_when` referencing an empty field** — hidden. Empty string does not match `equals = ""` (that's a validation error anyway).
- **Template `{{name}}` sanitization** — Windows-reserved filenames (`CON`, `NUL`, `COM1`–`COM9`) and cross-platform invalid chars are replaced with `-`. Don't rely on exact `{{name}}` round-tripping into the filename.
- **Moving `api_key` into `config.toml`** — works, but Pour will migrate it into `secrets.toml` on next load. Write to `secrets.toml` directly if you're generating config programmatically.
- **Per-platform path separators** — Pour accepts forward slashes in paths on all platforms and normalizes them internally. Use `/` in TOML regardless of OS.
- **`composite_array` sub-fields** can't use `show_when` and can't be nested. Only `text`, `number`, `static_select` allowed inside.
- **NEVER put a field token in `post_write_shell`.** `{{title}}`/`{{field_name}}` are **rejected at config-load time**, not stripped — that rejection is a security boundary, not a limitation to work around. The allowed tokens are all Pour-generated and cannot carry user text, which is why the command needs no quoting. If a hook seems to need a captured value, that is a request to change the execution model (argv-style), not the token list.
- **`post_write_shell` is arbitrary command execution from config.** Do not add one on a user's behalf without asking. A hook that auto-commits and pushes turns a bad capture into public history.
- **TOML ordering around `[modules.<n>.frontmatter]` / `[modules.<n>.platform]`** — every plain module key must come **before** these sub-table headers. A `post_write_shell` written after `[modules.<n>.frontmatter]` silently becomes a frontmatter entry instead of a hook, with no error.
- **`{{slug}}` needs a `title` field, and `%S` in the path.** It is derived from the module's `title` field, is dash-prefixed (`-my-title`), and is empty when untitled — so two untitled captures in the same minute collide on `%Y%m%d-%H%M`. A filesystem collision is a hard error that discards the entry just typed. Use `%Y%m%d-%H%M%S{{slug}}.md`.
- **`config_version` is checked against the build.** The current schema is `"0.4.0"`; absent means `"0.1.0"`. While the major is 0, a higher *minor* than the build's is rejected at load (a `0.5.0` config fails on a `0.4.0` build); from 1.0 on, only a higher major is. Older versions always load. Set it truthfully: an older binary silently ignores keys it does not know (there is no `deny_unknown_fields`), so the version check is the only thing that stops a config using `post_write_shell` from loading on a build that never runs it.
- **Number fields are quoted in create-mode frontmatter.** `dose_g: "15"`, not `dose_g: 15`. A YAML parser, Dataview included, reads that as the string `"15"`. Only `update` modules, composite number cells, and static `[modules.<n>.frontmatter]` numbers are written bare.

## Validating a config

There is no check command. Every entry point loads and validates the whole config before doing anything else, and a bad config exits 1 with `pour: config validation failed:` followed by one line per problem. Without a TUI, run `pour <module> --help`. Pour has no `--help` flag, so with a valid config it fails with `module '<module>' has no field '--help'`, which tells you the config loaded. Opening the dashboard also shows a dismissable warning for module paths or `dynamic_select` sources that do not exist in the vault.

## Runtime API for agents and remote clients

The capture-time surface above (config edits, file inspection) is *one* way for an agent to operate on a Pour install. The other is the runtime HTTP API exposed by `pour serve`.

**When to use which:**

- **Edit `config.toml` / `presets.json` directly** when the user is asking you to add a module, change a field, define a template, or curate presets. These are schema changes — they belong in the file, not behind an API call. The rules in this document govern.
- **Use the HTTP API** when the user is asking you to *capture* something ("log that I just had a V60 with the Onyx Ethiopia at 1:15"), query their history, or retrieve a prior capture. These are runtime operations against a running daemon.

The HTTP API is a complete agent surface: schema discovery (`GET /api/v1/config`), valid-choices for dynamic dropdowns (`GET /api/v1/options/{module}/{field}`), idempotent submits with offline-time-preserving `captured_at` (`POST /api/v1/submit/{module}`), read-back of any prior capture (`GET /api/v1/captures/{history_id}`), history queries (`GET /api/v1/history`), preset management (`/api/v1/presets/{module}`), and `GET /api/v1/health`. Auth is a Bearer token from the `mobile_token` field of `~/.pour/secrets.toml`, or `POUR_MOBILE_TOKEN`. A module with `mobile_visible = false` is left out of `/config`, and `/options` and `/submit` answer 404 for it.

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
