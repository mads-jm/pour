---
tags:
  - guide
  - tui
  - config
date created: Tuesday, March 31st 2026, 10:04:48 pm
date modified: Thursday, April 2nd 2026, 8:17:08 am
---

# Guide: Adding a New Field Type to the TUI

When extending the configuration schema, touch these layers in order so the new field stays coherent across [[pour-design-spec]] and the running app:

1. __Config Layer (`src/config.rs`)__: Add the new variant to the `FieldType` enum. Update `validate()` if the field requires specific properties, like `static_select` requiring `options`.
2. __State Layer (`src/app.rs`)__: Update `App::init_form()` to handle any default state population for the new field, and `App::validate_form()` if it requires custom validation logic before submission.
3. __Presentation Layer (`src/tui/form.rs`)__: Add the render logic to `render()`. Determine if it renders inline like text or requires a popup overlay like selects.
4. __Input Handling (`src/tui/form.rs`)__: Update the `KeyEvent` matcher to handle how the user interacts with this specific field when it is active.
5. __Output Layer (`src/output/mod.rs`)__: Ensure the field's data structure is properly routed to either `frontmatter` or `body` targets during submission.

For the current TUI shape and event routing, see [[System-Architecture-Overview]] and [[sprint-5-tui-report]].


