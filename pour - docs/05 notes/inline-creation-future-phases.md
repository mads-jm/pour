---
date created: Friday, April 3rd 2026, 2:21:46 am
date modified: Wednesday, April 29th 2026, 5:31:52 pm
---

# Inline Creation — Future Phases

## Shipped

### Template-Driven Note Scaffolding (Phase 2)

`[templates.<name>]` config sections define reusable note templates with `path` (supporting `{{name}}` and strftime), and `fields` (restricted to text, number, static_select). Referenced by `create_template` on dynamic_select fields. Validation enforces template existence, `{{name}}` in path, no `..` traversal, unique field names, and reserved name (`date`, `name`) rejection.

__Files__: `src/config.rs`, `src/autocreate.rs`, `tests/config.rs`, `tests/autocreate.rs`

### Sub-Form Overlay for Template Creation (Phase 2)

When `allow_create + create_template` is set and the user enters a novel value, a centered modal sub-form overlay appears with the template's fields. Supports text input with char-index cursor, number validation, static_select dropdowns. Submit creates the note with full frontmatter, Esc cancels. Graceful degradation to bare stub if terminal too small.

__Files__: `src/tui/form.rs`, `src/app.rs`

### Post-Creation Command Hook (Phase 2)

`post_create_command` fires an Obsidian command via REST API `/commands/{commandId}/` after template-driven note creation. Enables coordination with Templater and other plugins. Silently skipped on filesystem transport.

__Files__: `src/transport/api.rs`, `src/transport/mod.rs`, `src/main.rs`

## Still Deferred

- Nested templates / recursive sub-forms
- Dynamic data sources in template fields (only static_select supported, not dynamic_select)
- Template inheritance or composition
- TUI configure screen support for editing `create_template` / `post_create_command` fields
- Dataview DQL query sources for template field options

