---
date created: Monday, April 27th 2026, 6:16:25 am
date modified: Wednesday, April 29th 2026, 5:31:49 pm
---
# Spec: Append Target Recovery (missing Daily notes)

## Context

Append-mode modules (e.g. `pour me`) write under a heading in a date-derived daily note like `Journal/20260426.md`. If that file doesn't exist yet, both transports hard-fail:

- API: `src/transport/api.rs:146-201` — GET on the missing file returns 404 and the error propagates.
- FS: `src/transport/fs.rs:114-116` — explicit `if !full_path.exists()` bails immediately.

The user fills the entire form, hits submit, and loses their input to a "Write failed" line in the summary screen. The startup `Config::check_paths()` at `src/config.rs:1661-1711` already warns about missing append targets, but it skips any path containing `{{` or `%` (line 1667) — i.e. exactly the daily-note pattern that causes this bug.

Goal: (1) make the missing-target state visible on the dashboard before the user starts composing, and (2) preserve in-flight input by offering to create the note at submit time. Both fixes share one new transport capability — creating an empty note — backed by Obsidian REST API's PUT endpoint (`pour - docs/02 references/obsidian-local-rest-api.md` lines 92-95) for API mode and `std::fs::write` for FS mode.

## Approach

Three pieces, all sharing one new transport method:

### 1. Transport: `create_note` Method

Add to the `Transport` enum / trait at `src/transport/mod.rs`:

- API impl (`src/transport/api.rs`): `PUT /vault/{path}` with header `If-None-Match: *` and `Content-Type: text/markdown`. Body = the configured `append_under_header` followed by a blank line, so the next append finds its heading. 412 means the file appeared between check and create — treat as success.
- FS impl (`src/transport/fs.rs`): create parent dirs via `fs::create_dir_all`, then `fs::write` the same minimal content. Use `OpenOptions::new().write(true).create_new(true)` to avoid a TOCTOU clobber.

### 2. Dashboard `⚠ missing` Indicator

In `src/tui/dashboard.rs:207-222`, after the existing `count_span`, append a status span for any append-mode module whose resolved target file is missing.

Resolution helper (new, in `src/output/template.rs`): `resolve_static_path(template, date_format, now) -> Option<String>` — runs strftime expansion + `{{date}}`/`{{time}}` substitution, then returns `Some(path)` only if no `{{…}}` placeholders remain. Daily-note paths like `Journal/%Y%m%d.md` resolve cleanly; field-dependent paths like `Coffee/{{bean}}.md` return `None` and get no indicator (we can't know the target until form-fill).

Existence check: `app.config.vault.base_path.join(resolved).exists()`. Cheap enough to do per-render — one `metadata` syscall per append-mode module. FS check is authoritative for both transports since the vault sits on disk in both modes.

Render style: yellow `⚠ missing` span, matching the existing "gaps" yellow at `dashboard.rs:285`.

When the highlighted module has the indicator, append `· c create` to the footer hint at `dashboard.rs:299-318`.

### 3. Confirm-on-submit + Dashboard Hotkey

__Typed error.__ `src/output/mod.rs:99-149` `write_append` currently bubbles a string error. Introduce a `WriteError::TargetMissing { vault_path }` variant (or pre-check existence inside `write_append` before calling the transport, to keep the typed error in one place). The caller needs to distinguish "missing target" from generic IO/HTTP failures.

__Confirm overlay.__ When the form-submit handler in `src/main.rs:635` (`handle_submit`) sees `TargetMissing`, push a new app screen/overlay state — same dismissable-overlay pattern already used for startup warnings (`src/main.rs:225-227`, `src/tui/dashboard.rs:323-325`, `src/tui/dashboard.rs:397-439`). Overlay text:

```
target Journal/20260426.md is missing.

[Y] create and submit
[N] cancel (keep input)
```

On `Y`: call `transport.create_note(path, heading)`, then retry `write_append`. On `N`: dismiss overlay, return to form with all field values preserved (form state is already in `App`, so this is just a screen transition, not a re-init).

__Dashboard hotkey `c`.__ In the dashboard input handler (alongside the existing module navigation in `src/main.rs`), if the highlighted module is append-mode and `resolve_static_path(…)` returns a missing path, `c` calls `transport.create_note(…)` for that module. No overlay — just create and let the indicator disappear on next render. Surfaces tactile feedback by briefly flashing the module name green for one tick (optional polish — drop if it complicates the patch).

## Files to Modify

- `src/transport/mod.rs` — add `create_note(&self, vault_path: &str, header: &str) -> Result<()>` to the `Transport` enum dispatch.
- `src/transport/api.rs` — `ApiClient::create_note` via PUT + `If-None-Match: *`.
- `src/transport/fs.rs` — `FsWriter::create_note` via `create_dir_all` + `OpenOptions::create_new`.
- `src/output/template.rs` — new `resolve_static_path` helper that returns `Option<String>`. Reuses the existing strftime + token logic in `render_path` (lines 25-71).
- `src/output/mod.rs` — `write_append` returns typed `TargetMissing` when path doesn't exist before calling transport (or wrap the transport error if it does occur).
- `src/tui/dashboard.rs` — missing-target span in module list; conditional footer hint.
- `src/main.rs` — `c` hotkey on dashboard; `TargetMissing` overlay in submit handler; create-and-retry path.

## Reusable Helpers (don't reinvent)

- `template::render_path` at `src/output/template.rs:25-71` — strftime + token substitution. New `resolve_static_path` should call into the same logic.
- `Config::check_paths` at `src/config.rs:1661-1711` — pattern for "skip paths with `{{` or `%`". The new dashboard check is the dynamic-aware sibling, not a replacement.
- Dismissable-overlay rendering at `src/tui/dashboard.rs:397-439` — pattern for the "create note?" confirm.
- Module-list iteration at `src/tui/dashboard.rs:169-224` — just append a span to the existing `Line::from(vec![…])`.

## Out of Scope

- Templated daily notes (frontmatter, custom sections from Obsidian's daily-note template). v1 creates a minimal file with just the heading. If users want richer templates, they create the note in Obsidian first.
- Create-mode modules whose parent directory is missing (the user only mentioned append). Existing `check_paths` already warns at startup; revisit if it becomes a real pain point.
- Auto-create-on-submit without confirmation. The confirm step is intentional — silent file creation in the user's vault is too surprising for a personal note system.

## Verification

1. `cargo build && cargo test` — covers the new template helper and any new transport-level tests.
2. Manual, FS transport, missing file:
   - Configure `pour me` with `mode = "append"`, `path = "Journal/%Y%m%d.md"`.
   - Delete today's `Journal/20260426.md` from the vault.
   - Run `pour` → dashboard shows `⚠ missing` next to `[me]` and footer shows `c create`.
   - Press `c` → file appears with just the configured `## Log` heading.
   - Press `c` again on the now-existing module → no-op (file already exists; FS `create_new` errors gracefully or we treat AlreadyExists as success).
3. Manual, FS transport, in-flight input recovery:
   - Delete today's daily note.
   - Run `pour me`, fill the form, submit.
   - Confirm overlay appears with the resolved path.
   - Press `N` → return to form with all input intact.
   - Press submit again, confirm overlay re-appears, press `Y` → file is created and append succeeds; entry visible in dashboard "recent" on return.
4. Manual, API transport: same flow with Obsidian Local REST API running. Verify PUT creates the note (check via Obsidian) and append-under-heading lands in the right spot.
5. Field-templated path (`Coffee/{{bean}}.md`): confirm dashboard shows NO indicator (path can't be resolved without form input). Submitting still hits the existing error path — that's a separate UX problem and not in scope here.

## Docs to Update on Implementation (per CLAUDE.md)

- `pour - docs/04 architecture/System-Architecture-Overview.md` — note the new `create_note` transport method.
- `pour - docs/09 milestones/v0.2.0-Foundation.md` — if "missing daily note" is listed as a known limitation, mark resolved.
- `pour - docs/08 specs/pour-design-spec.md` — annotate the append-mode behavior with the new confirm-on-missing flow if the spec described the old hard-fail.
- `pour - docs/00 index/` — link this spec from the relevant index file.
- `README.md` — only if it describes append behavior; otherwise skip.
