---

kanban-plugin: board
date created: Tuesday, April 7th 2026, 2:52:17 am
date modified: Monday, May 4th 2026, 11:17:46 pm

tags:
  - feature
  - cq
  - question
---

# Inbox

- [ ] Select dropdown and composite editor open one row too high below a preset description or callout textarea — `render_select_options` and `render_composite_editor` still use the field index as the screen row. An `allow_create` select covers its own search field.
- [ ] Long-form editor cycle: sticky viewport on both axes — the textarea popout and single-line rows recompute scroll from the cursor each frame, so Up/Left slide the text under a still cursor instead of moving the cursor first.
- [ ] Configure screen counts chars as cells — `tui/configure/render.rs` cursor placement uses `cursor_position - scroll_offset` as a column, so CJK and emoji in a setting value drift. Reuse the form's per-glyph cell count.
- [ ] Preset save/delete only on preset/submit screen
- [ ] Review text entry across mads.modules (textarea/popover for notes)
- [ ] Handle missing file in append mode (template fallback)
- [ ] Callout type configurable per module/textarea
- [ ] Composite dynamic select (submenus)
- [ ] User-configurable Obsidian commands
- [ ] Multi-path resolution / fallbacks
- [ ] Complex tuple array field — `composite_array` evolution
- [ ] Configuration validation UI
- [ ] Vault configuration sync
- [ ] Icon validation (length, grapheme, control chars)
- [ ] TUI test coverage for icon rendering
- [ ] `secrets.toml` file permissions on Unix
- [ ] `App.deferred_stderr` not drained on panic — autocreate diagnostics may be lost
- [ ] `render_append_template` hardcodes `%Y-%m-%d` for `{{date}}` (only `path` rendering honors `date_format`)
- [ ] `required` + `show_when` — clear value on hide or preserve for toggle-back?
- [ ] Config migration — aspirational or planned? Annotate docs accordingly.
- [ ] `semver` crate vs hand-rolled — ~30KB, eliminates edge cases.
- [ ] Field vs module icon asymmetry — field icons cosmetic only, intentional?
- [ ] Icon ordering in frontmatter — position 0 deliberate for Dataview sorting?

# Scoping

- [ ] Priors / review panel **L2** — `overlap`/`window` match modes, select/tag `mode` summaries, `me`/`note` recent-N + `#tag` overlap, PWA panel + `POST /api/v1/priors/{module}`. Spec: [[pour-review-priors]] §10 (L2).

# Ready

# In Progress

# In Review

# Done

- [x] Form value-side cursor uses byte offset; CJK/emoji in field *values* can drift (prompt-side fixed in v0.2.1). Textarea popout and single-line rows now place the cursor by display width.
- [x] Text area double newline in blockquote output
- [x] Priors / review panel **L1** (coffee, TUI) — config-declared read-back of best prior captures at capture time; shared frontmatter reader + wikilink stripper foundation. Spec: [[pour-review-priors]] (shipped). Story: [[priors_at_the_pour]]. ADR: [[ADR-007-Frontmatter-Reader]].

%% kanban:settings

```
{"kanban-plugin":"board","list-collapse":[]}
```

%%
