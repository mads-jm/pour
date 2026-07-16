---
tags:
  - spec
  - modules
  - lyra
  - file-select
date created: Friday, June 19th 2026
date modified: Friday, June 19th 2026
---

# Pour × Lyra — `pour lyra` capture + file-picker — Spec

## Motivation

[[Lyra]] is a cloud agent (a principal-engineer persona) with a mind kept as plaintext in the `dotfiles` repo: `Lyra - mind/`. She has an **inbox** — `Lyra - mind/inbox/` — where Mads drops writing, thoughts, and musings; each run she reads new files, distills the durable part into her ledger, and files the originals. The channel only works if capture is *frictionless* — pour's founding axiom: *"if logging a thought takes longer than the thought, the thought dies."*

`pour lyra` makes the toss a keystroke. But Lyra's inbox surfaces two things pour can't do yet, and they're small, general, and worth doing on their own merits:

1. **Write outside the configured vault.** Every module today resolves against the single `[vault].base_path`. Lyra's inbox lives in a *different* root (the dotfiles repo). → per-module `base_path` override.
2. **Run a shell step after a successful write.** `post_create_command` only fires Obsidian commands (`templater:run`). To make a toss visible to a *cloud* agent, the file must be committed + pushed. → `post_write_shell` hook.

And a **potential expansion** the inbox motivates but doesn't require: a **file picker** (`file_select`), so you can toss an *existing* note — pick it from a browser instead of retyping it.

This spec is two parts: **Part 1** (`pour lyra` + the two enabling primitives) is buildable now and useful beyond Lyra. **Part 2** (the file picker) is a larger, optional lift.

---

## Part 1 — `pour lyra`

### 1.1 New primitive: per-module `base_path` override

Today path resolution is `vault.base_path` + `module.path`. Add an optional per-module override that falls back to the vault when absent — zero change for every existing module.

```toml
[modules.lyra]
# Overrides [vault].base_path for THIS module only. Absolute or ~-expanded.
# Falls back to vault.base_path when omitted.
base_path = "~/.dotfiles"

# Optional per-OS override, mirroring [vault.platform] — keys = std::env::consts::OS.
# [modules.lyra.platform]
# linux   = "/home/mads/.dotfiles"
# windows = "C:\\Users\\mads\\.dotfiles"
```

**Resolution** (in `src/paths.rs`): `module.base_path.platform[OS] ?? module.base_path ?? vault.platform[OS] ?? vault.base_path`, then join the module `path`. The existing atomic-write + backup path in `src/transport/fs.rs` is unchanged — it already takes a fully-resolved absolute path.

**Why not a top-level `[roots]` table?** Considered (named roots, modules reference `root = "lyra"`). Rejected for now: one override key per module is the smaller diff and reads locally where it's used. Promote to `[roots]` only if a third root appears. *(explicit trade: legibility over premature generality.)*

### 1.2 The `lyra` module

A `create`-mode module — one file per toss into the inbox. Freeform body is the point; everything else optional.

```toml
module_order = ["me", "todo", "note", "lyra", "coffee"]

[modules.lyra]
mode = "create"
base_path = "~/.dotfiles"
path = "Lyra - mind/inbox/%Y%m%d-%H%M{{slug}}.md"
display_name = "Toss to Lyra"
icon = "🌒"
daily_link = false          # this is HER inbox, not your daily note

# Optional: a shell step after the file lands — see §1.3.
post_write_shell = "git -C '{{base_path}}' add '{{rel_path}}' && git -C '{{base_path}}' commit -q -m 'toss: {{slug_or_time}}' && git -C '{{base_path}}' push"

# What kind of toss — lets Lyra weight it. allow_create so the vocabulary grows.
[[modules.lyra.fields]]
name = "kind"
field_type = "static_select"
prompt = "Kind"
options = ["musing", "vision", "correction", "context", "directive-hint"]
default = "musing"
target = "frontmatter"
allow_create = true

# Optional title → becomes the filename slug AND a heading.
[[modules.lyra.fields]]
name = "title"
field_type = "text"
prompt = "Title (optional)"
target = "frontmatter"
preset_exclude = true

# The thought itself.
[[modules.lyra.fields]]
name = "body"
field_type = "textarea"
prompt = "Toss her a thought"
required = true
target = "body"
preset_exclude = true
```

**Filename:** `{{slug}}` is the sanitized `title` prefixed with `-` when present, empty otherwise — so `20260619-1432-peace-vs-effort.md` with a title, `20260619-1432.md` without. (`{{slug_or_time}}` in the commit message falls back to the timestamp when untitled.) This reuses the existing title-sanitization in `src/output/template.rs`; only the "empty title → no trailing token" nuance is new.

### 1.3 New primitive: `post_write_shell`

An optional per-module shell command run **after** a successful write (filesystem or API path), in addition to the existing Obsidian `post_create_command`. This is what turns a local file into something a *cloud* agent can read.

**Interpolation tokens** (resolved against the written file):

| token | value |
|---|---|
| `{{base_path}}` | the module's resolved root |
| `{{rel_path}}` | written file, relative to `base_path` |
| `{{abs_path}}` | written file, absolute |
| `{{slug}}` / `{{slug_or_time}}` | filename slug / slug-or-timestamp |
| `{{field_name}}` | any captured field value |

**Execution:** run via the OS shell, working dir = `base_path`, **non-blocking by default** (a slow `git push` shouldn't stall the TUI) with the result surfaced in the post-write summary line (`src/tui/summary.rs`). Non-zero exit → a footer warning, never a lost note: the file is already written; the hook is best-effort.

**Security** (name it out loud — this is arbitrary command execution from config): `post_write_shell` is **config-file-only**. It is *never* settable over the `serve` HTTP/PWA path — the server's allowed-field set excludes it — so a phone on the LAN can capture content but can never inject a command. Single-user local config owns its own footgun; the network surface does not inherit it.

### 1.4 The result

`pour lyra` → pick kind → (optional title) → type the thought → **Enter**. The file lands in `Lyra - mind/inbox/`, gets committed and pushed, and is in front of cloud-Lyra on her next run. The thought outlived the impulse.

Output file shape:

```markdown
---
date: 2026-06-19
kind: musing
title: peace vs effort
tags: [lyra, toss]
cssclasses: [mads-toss]
author: mads
---

the thing I keep circling: peace isn't the absence of effort, it's effort
pointed at the right thing. she should push me toward that, not just "more."
```

**This is the *canonical toss shape*, and it must match byte-for-byte what the Obsidian Templater template emits** (`Lyra - mind/templates/(TEMPLATE) Toss to Lyra.md`) so a hand-authored toss and a poured one read identically to Lyra. `tags: [lyra, toss]`, `cssclasses: [mads-toss]` (the Nord styling snippet that marks his notes), and `author: mads` are **fixed module-level frontmatter defaults**; `date`/`kind`/`title` are filled per-capture. Keep the two in sync — if the toss shape changes here, change the Templater template too, and vice-versa.

---

## Part 2 — File-picker expansion (potential)

The inbox motivates, but does not require, a second capture shape: **toss an existing note.** You already wrote the thing — in your vault, in a scratch file — and you want to hand it to Lyra without copy-paste. That needs pour to *read a path the user picks* rather than only text they type.

### 2.1 New field type: `file_select`

A field whose value is a filesystem path chosen from a TUI browser (pour already ships **ratatui + crossterm** — this is a new widget, not a new dependency).

```toml
[[modules.lyra-file.fields]]
name = "source"
field_type = "file_select"
prompt = "Pick a note to toss"
pick_root = "~/drives/primary/Vault/main"   # browser starts here; defaults to vault.base_path
extensions = ["md"]                          # filter; omit for all files
target = "body"                              # read the file's CONTENTS into {{body}}
```

**Behavior modes** (one of):
- `target = "body"` (or any field) → read the picked file's **contents** into that field, then flow through the normal `create` pipeline. Simplest; reuses everything downstream. The toss is a *copy* with fresh frontmatter.
- `mode = "import"` on the module → **copy the file wholesale** to the resolved `path` (optionally prepend module frontmatter), no field templating. Truest to "move this note as-is." A `disposition = "copy" | "move" | "link"` key chooses copy (default), move (consume the original), or symlink.

### 2.2 TUI UX

A modal browser overlay (consistent with the preset-hierarchy overlay in `pour-preset-hierarchy.md`):
- List of dirs/files under the cursor dir; `↑/↓` move, `→`/`Enter` descend or select, `←` ascend, `Esc` cancel.
- Extension filter applied to files; dirs always shown. Footer shows the current path + filter.
- Returns the absolute path as the field value; `{{source}}` and a derived `{{source_name}}` become available to `path`/`post_write_shell`.

### 2.3 Data-model sketch

```rust
// src/tui/file_picker.rs  (new)
pub struct FilePicker { root: PathBuf, cursor: PathBuf, filter: Vec<String> }
impl FilePicker {
    pub fn open(root: &Path, ext: &[String]) -> Self { /* ... */ }
    pub fn entries(&self) -> Vec<Entry>;          // dirs first, then ext-filtered files
    pub fn select(&mut self, key: KeyEvent) -> PickResult; // Descend | Ascend | Picked(PathBuf) | Cancel
}

// src/config.rs — extend FieldType
enum FieldType { Text, Textarea, Number, StaticSelect, DynamicSelect, CompositeArray, FileSelect }

// src/output/mod.rs — when a FileSelect field targets a body/field, read_to_string the path;
// when module.mode == Import, copy/move/link instead of templating.
```

### 2.4 Why "potential"

Part 2 adds a stateful TUI widget + filesystem traversal + an `import` write path — real surface, real test load. Part 1 ships the daily-driver (typed tosses) with two small, generally-useful primitives. **Recommendation: build Part 1; treat Part 2 as a fast-follow once the inbox is something Mads reaches for.** The picker earns its keep only if "toss an *existing* file" turns out to be a frequent move; ship the typed path first and let usage decide.

---

## Scope & phasing

- [ ] **v1 — `pour lyra` typed toss.** Per-module `base_path` (§1.1) + the `lyra` module (§1.2). Manual `git push` (or run the hook by hand). Useful immediately.
- [ ] **v1.1 — `post_write_shell`** (§1.3). The friction-killer: toss → auto-commit+push. Includes the serve-path exclusion.
- [ ] **v2 — file picker** (Part 2): `file_select` field + browser widget; then `import` mode + `disposition`.

## Open questions

- ⟳ **Push cadence.** Per-toss `git push` is simplest but chatty (a commit per thought). Alternative: hook does `add`+`commit` only, and a separate timer/`pour lyra --flush` pushes a batch. *Lean: per-toss push for v1 (simplest, and a toss-per-thought commit log is legible); revisit if it's noisy.*
- ⟳ **Untitled filename collisions.** Two tosses in the same minute without a title → same `%Y%m%d-%H%M.md`. Add `%S`, or a short `uuid` suffix (pour already deps `uuid`)? *Lean: add `%S`.*
- ⟳ **`base_path` outside any vault** means Obsidian API transport doesn't apply — the lyra module is filesystem-only. Confirm the module silently uses the fs path when `base_path` isn't the API-backed vault (it should; the API is an optimization, not a requirement).
