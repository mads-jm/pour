---
tags:
  - spec
  - modules
  - roots
  - hooks
aliases:
  - pour-lyra-capture
  - roots and hooks
date created: Friday, June 19th 2026
date modified: Wednesday, August 5th 2026
---

# Pour roots & write hooks — capture beyond the vault — Spec

## Motivation

Two gaps kept whole categories of capture out of pour's reach:

1. **Some capture targets aren't in the vault.** Every module used to resolve against the single `[vault].base_path`. But a capture may belong to a *different* root entirely — another git repo, an agent's inbox, a shared notes directory. → per-module `base_path` override (§1.1).
2. **Some writes aren't *delivered* until a command runs.** A file landing on disk is sometimes only half the capture — it still needs a `git commit && git push`, a sync trigger, an indexer poke. → `post_write_shell` hook (§1.3).

Both are general primitives, in keeping with the project rule that pour ships no module-specific code. And a **potential expansion** the same use case motivates but doesn't require: a **file picker** (`file_select`), so you can capture an *existing* file — pick it from a browser instead of retyping it (Part 2).

*Origin note: these primitives shipped 2026-07-16 to serve a personal workflow — tossing thoughts into a cloud agent's inbox kept in a dotfiles repo, where a capture only reaches the agent once pushed. Pour itself gained only the general keys below; the module that motivated them lives on as a preset in `resources/mads_config.toml`. The founding axiom applies unchanged: if logging a thought takes longer than the thought, the thought dies.*

---

## Part 1 — roots & hooks

### 1.1 Per-module `base_path` override

Path resolution used to be `vault.base_path` + `module.path`. An optional per-module override falls back to the vault when absent — zero change for every existing module.

*[Deviation: Part 1 shipped 2026-07-16. Six general keys landed — `base_path`, `[modules.<n>.platform]`, `[modules.<n>.frontmatter]`, `frontmatter_date_format`, `post_write_shell`, `post_write_shell_on_serve` — all optional, all absent-means-today's-behavior. See [[field-types]] for the reference. Deviations from the original draft are annotated inline below.]*

```toml
[modules.inbox]
# Overrides [vault].base_path for THIS module only.
base_path = "/home/user/notes-repo"

# Optional per-OS override, mirroring [vault.platform] — keys = std::env::consts::OS.
[modules.inbox.platform]
windows = "C:\\Users\\user\\notes-repo"
```

**Resolution:** `module.platform[OS] ?? module.base_path ?? vault.platform[OS] ?? vault.base_path`, then join the module `path`. The existing atomic-write + backup path in `src/transport/fs.rs` is unchanged — it already takes a fully-resolved absolute path.

*[Deviation: **`~` is not expanded — absolute paths only.** Pour has no tilde expansion anywhere, and `FsWriter::resolve_path_validated` actively rejects `~`; accepting it here would have silently created a literal `~` directory. A tilde in `base_path` is a config validation error.]*

*[Deviation: **the resolution lives in `src/config.rs`, not `src/paths.rs`.** The shipped shape is `ModuleConfig::root_override() -> Option<&str>` (module platform → module base_path → `None`), with `None` completing the chain by falling back to the vault. The real work was **transport selection**, not path joining: `Transport::connect` stays one app-level instance, and `Transport::for_module` returns a write-time `FsWriter` rooted at the override.]*

*[Deviation: a `base_path` module is **filesystem-only** — the Obsidian Local REST API can only address the vault it serves — and this is reported truthfully rather than silently: both the TUI summary and the `/api/v1/submit` response show `FileSystem` for these captures even when the API is connected. Known limit: a `dynamic_select` `source` on such a module still resolves against the vault.]*

**Why not a top-level `[roots]` table?** Considered (named roots, modules reference `root = "inbox"`). Rejected for now: one override key per module is the smaller diff and reads locally where it's used. Promote to `[roots]` only if a third root appears. *(explicit trade: legibility over premature generality.)*

### 1.2 Worked example: an agent-inbox module

The motivating shape — a `create`-mode module dropping one file per capture into a repo that something else (an agent, a sync job, another person) consumes. Freeform body is the point; everything else optional:

```toml
[modules.inbox]
mode = "create"
base_path = "/home/user/notes-repo"
path = "inbox/%Y%m%d-%H%M%S{{slug}}.md"
display_name = "Toss to inbox"
daily_link = false          # the consumer's inbox, not your daily note
post_write_shell = "git add '{{rel_path}}' && git commit -q -m 'capture: {{slug_or_time}}' -- '{{rel_path}}' && git push -q"

[[modules.inbox.fields]]
name = "kind"
field_type = "static_select"
prompt = "Kind"
options = ["musing", "context", "directive-hint"]
default = "musing"
target = "frontmatter"
allow_create = true

[[modules.inbox.fields]]
name = "title"
field_type = "text"
prompt = "Title (optional)"
target = "frontmatter"
preset_exclude = true

[[modules.inbox.fields]]
name = "body"
field_type = "textarea"
prompt = "The thought"
required = true
target = "body"
preset_exclude = true
```

**Filename:** `{{slug}}` is the sanitized `title`, dash-prefixed when present, empty otherwise — `20260619-143255-peace-vs-effort.md` with a title, `20260619-143255.md` without. (`{{slug_or_time}}` in the commit message falls back to the timestamp when untitled.)

*[Deviation: **the slug was new code, not a reuse.** `sanitize_filename_chars` only swaps Windows-illegal characters — it does not kebab-case. `slug_from_title` instead mirrors a Templater JS slug regex exactly, including its non-obvious dropping of all non-ASCII (`café` → `caf`), so poured and hand-templated filenames stay in parity.]*

*[Deviation: **`{{slug}}` had to be registered as a special token**, alongside `{{date}}`/`{{time}}` — `render_path` strips unknown placeholders, so an unregistered `{{slug}}` would silently render to nothing for every capture, with no error.]*

*[Deviation: **`%S` belongs in the path when `title` is optional.** Untitled captures in the same minute otherwise collide, and FS `create_file` hard-errors on an existing path — the TUI would then discard the text just typed. No general collision-retry was added to `create_file`; the fix is the path template's.]*

### 1.3 `post_write_shell`

An optional per-module shell command run **after** a successful write, on either transport. This is what turns a local file into something *delivered*.

**Interpolation tokens — this list is exhaustive:** `{{base_path}}`, `{{rel_path}}`, `{{abs_path}}`, `{{slug}}`, `{{slug_or_time}}`.

*[Deviation: the draft also proposed `{{field_name}}` — **dropped, not deferred.** Every shipped token is Pour-generated and cannot carry raw user text, which is *why* interpolating into a shell string without quoting is sound; `{{field_name}}` would carry arbitrary typed text into a command line. Unknown tokens are **rejected at config-load time**, not stripped — a silent strip (as `render_path` does for paths) would read as "user text is supported here" while quietly changing the command. Shell-quoting and argv-style exec were both considered and rejected: quoting is too easy to get subtly wrong cross-platform for a one-way door, and argv kills `&&` and the git example. **Argv-style is the escalation path if a hook ever needs to carry an untrusted value** — that is the trigger to revisit, not extending the token list.]*

**Execution:** via the OS shell, working dir = `base_path`, result surfaced in the post-write summary (`src/tui/summary.rs`). Non-zero exit → a footer warning, never a lost note: the file is already written; the hook is best-effort.

**Security** (name it out loud — this is arbitrary command execution from config): `post_write_shell` is **config-file-only**. A LAN client cannot set it: `SubmitRequest` is a typed DTO carrying only field values and capture metadata — a client cannot name a config key at all, so serde drops any attempt at the wire boundary. Pinned by `a_lan_client_cannot_inject_a_hook_through_the_submit_body`. A **second** gate sits on top: `post_write_shell_on_serve`, default `false` — a LAN-submitted capture does not fire even an *already-configured* hook unless the module opts in. Single-user local config owns its own footgun; the network surface does not inherit it.

*[Deviation: **execution is awaited (with a 30s timeout), not fire-and-forget.** "Non-blocking + report the exit code + survive process exit" is not simultaneously satisfiable: reporting requires waiting, and a detached task dies with the runtime on quit — defeating the hook's purpose (a push that never happened) precisely when the user is fastest. The bounded await costs ~1–2s of frozen summary screen after a `git push`; the capture is already on disk by then.]*

*[Deviation: the child gets **null stdin and piped stdout/stderr** — the TUI holds a raw-mode terminal, so inherited streams would corrupt the frame, and an inherited stdin would let a credential prompt hang forever behind a UI that cannot show it.]*

*[Deviation: **`post_create_command` is field-level, not module-level**, and requires `create_template` — the draft's framing of it as `post_write_shell`'s sibling was loose. They are not peers.]*

### 1.4 Output shape

```markdown
---
date: 2026-06-19T14:32
kind: musing
title: peace vs effort
author: sam
cssclasses:
  - poured
tags:
  - inbox
---

the thing I keep circling: peace isn't the absence of effort, it's effort
pointed at the right thing.
```

`tags`/`cssclasses`/`author` here are `[modules.<n>.frontmatter]` **statics** — fixed properties of the module, merged after the captured fields (`date`/`kind`/`title`). See [[field-types]] for the full rules.

*[Deviation: the draft demanded **byte-for-byte parity** with a hand-authored editor template. Retired as a non-goal: the corpus it was checked against contradicted itself, and downstream consumers (humans, LLM agents) are not strict parsers. What matters and what shipped: **block** sequences for arrays, per-module datetime via `frontmatter_date_format`, and **`title:` omitted when untitled**. Editor-side affordances (metadata callouts, sign-offs) stay in editor templates; pour does not emit them.]*

*[Deviation: statics are emitted in **alphabetical order** after the captured fields, not config order. Deterministic ordering beat config-order fidelity: the table deserializes into a `BTreeMap`, since preserving document order would have required a hand-written `Deserialize` impl for a purely cosmetic gain.]*

---

## Part 2 — File-picker expansion (potential)

The inbox pattern motivates, but does not require, a second capture shape: **capture an existing note.** You already wrote the thing — in your vault, in a scratch file — and you want to hand it to a module without copy-paste. That needs pour to *read a path the user picks* rather than only text they type.

### 2.1 New field type: `file_select`

A field whose value is a filesystem path chosen from a TUI browser (pour already ships **ratatui + crossterm** — a new widget, not a new dependency).

```toml
[[modules.inbox-file.fields]]
name = "source"
field_type = "file_select"
prompt = "Pick a note"
pick_root = "/home/user/vault"     # browser starts here; defaults to vault.base_path
extensions = ["md"]                # filter; omit for all files
target = "body"                    # read the file's CONTENTS into {{body}}
```

**Behavior modes** (one of):
- `target = "body"` (or any field) → read the picked file's **contents** into that field, then flow through the normal `create` pipeline. Simplest; reuses everything downstream. The capture is a *copy* with fresh frontmatter.
- `mode = "import"` on the module → **copy the file wholesale** to the resolved `path` (optionally prepend module frontmatter), no field templating. A `disposition = "copy" | "move" | "link"` key chooses copy (default), move (consume the original), or symlink.

### 2.2 TUI UX

A modal browser overlay (consistent with the preset-hierarchy overlay in [[pour-preset-hierarchy]]): `↑/↓` move, `→`/`Enter` descend or select, `←` ascend, `Esc` cancel. Extension filter applied to files; dirs always shown. Returns the absolute path as the field value; `{{source}}` and a derived `{{source_name}}` become available to `path`/`post_write_shell`.

### 2.3 Why "potential"

Part 2 adds a stateful TUI widget + filesystem traversal + an `import` write path — real surface, real test load. Part 1 shipped the daily-driver (typed captures) with small, generally-useful primitives. The picker earns its keep only if "capture an *existing* file" turns out to be a frequent move; usage decides.

---

## Scope & phasing

- [x] **v1 — typed capture into an external root.** Per-module `base_path` (§1.1) + the inbox module shape (§1.2). *[Shipped 2026-07-16, together with v1.1.]*
- [x] **v1.1 — `post_write_shell`** (§1.3). The friction-killer: capture → auto-commit+push. Includes the serve-path gates. *[Shipped 2026-07-16.]*
- [ ] **v2 — file picker** (Part 2): `file_select` field + browser widget; then `import` mode + `disposition`.

## Resolved questions

- ✅ **Push cadence.** Per-capture `git push` shipped — simplest, and a commit-per-thought log is legible. Batching (`--flush`) is out of scope; revisit if noisy.
- ✅ **Untitled filename collisions.** `%S` in the path template, preset-side only — no change to the shared `create_file` write path (§1.2).
- ✅ **`base_path` outside any vault** → filesystem-only, reported truthfully, not silently (§1.1).

## Follow-ups

- `deny_unknown_fields` / real config version gating — an older binary **silently ignores** `post_write_shell`. `config_version` was bumped `0.3.0` → `0.4.0` in both resource files, but this is **documentary only and gates nothing**: `CURRENT_CONFIG_VERSION` is `"1.0.0"` and `validate_config_version` checks **major** only. Reconcile the drift.
- Document that `resources/mads_config.toml` is a **seed, not a mirror** — `~/.pour/config.toml` symlinks to the stow package, so `allow_create` writes land only in the live file and the repo copy drifts. Structural, not accidental.
- *Preset upkeep (mads' vault, outside this repo):* keep the inbox preset's editor-side Templater in slug/stamp parity with §1.2, and hand-add preset changes to the live stowed config — the seed never propagates itself.
