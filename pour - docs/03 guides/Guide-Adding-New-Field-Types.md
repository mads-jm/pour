---
tags:
  - guide
  - tui
  - config
date created: Tuesday, March 31st 2026, 10:04:48 pm
date modified: Saturday, October 3rd 2026, 6:47:17 am
---

# Guide: Adding a New Field Type to the TUI

When extending the configuration schema, touch these layers in order so the new field stays coherent across [[pour-design-spec]] and the running app:

1. __Config Layer (`src/config.rs`)__: Add the new variant to the `FieldType` enum. Update `validate()` if the field requires specific properties, like `static_select` requiring `options`, or if a write mode can't hold it (`update` modules reject `composite_array` and body-targeted fields). A few config writers turn the type back into a string for `toml_edit` with exhaustive matches, so the compiler points you at each one.
2. __State Layer (`src/app.rs`)__: Update `App::init_form()` to handle any default state population for the new field, and `App::validate_form()` if it requires custom validation logic before submission.
3. __Presentation Layer (`src/tui/form/render/`)__: Add a match arm to `render_fields()` in `fields.rs`. Decide whether it renders inline like text or needs an overlay like the select list (`render_select_options`) or the textarea popout (`render_textarea_editor`). The footer key hint for the focused field lives in `render/mod.rs`.
4. __Input Handling (`src/tui/form/key/`)__: `dispatch()` in `key/mod.rs` routes each key by the active field's type. Character filtering for typed input is in `key/text.rs` (`handle_char`), and select behavior is in `key/select.rs`.
5. __Output Layer (`src/output/`)__: `partition_fields()` in `output/mod.rs` routes the value to `frontmatter` or `body` through `FieldConfig::effective_target()`. Value formatting is in `output/frontmatter.rs`, and `update`-mode writes are in `output/update.rs`.
6. __Configure Editor__: Add the type's string to the type cycler in `build_field_settings()` (`src/tui/configure/init.rs`) and to the string-to-enum match in `src/config_updates.rs`. The compiler doesn't check these string lists, and the parse falls back to `text`, so skipping them makes auto-save quietly rewrite the new type as `text`.
7. __Server and PWA__: Add the wire name to `field_type_wire()` in `src/server/dto/mapping.rs`, value checks to `src/server/handlers/submit/validate.rs`, and a widget to `buildFieldInput()` in `web/app.js`. Without one, the PWA renders the field as a plain text input.
8. __One-shot (`src/oneshot.rs`)__: Only if the type should be writable as `pour <module> <field> [value]`. Today that covers `toggle` and `counter` on `update` modules.

Tests go in `tests/`, mirroring `src/` (see `CONTRIBUTING.md`). Document the type, its keys, and its validation rules in [[field-types]].

For the current TUI shape and event routing, see [[System-Architecture-Overview]].
