---
tags:
  - spec
  - presets
  - tui
date created: Monday, April 27th 2026, 11:23:20 pm
date modified: Wednesday, April 29th 2026, 5:31:45 pm
---

# Pour Preset Hierarchy — Spec

## Motivation

The TUI preset selector was a single-line Left/Right cycler. With 9+ presets from brewer × bean × intent combinations, linear cycling became unusable. The fix: a hierarchical drilldown overlay that mirrors the user's mental model (`brewer → bean → intent`), declared per module in config.

## Config Schema

Add `preset_axes` to any module in `config.toml`:

```toml
[modules.coffee]
mode = "create"
path = "Coffee/log.md"
preset_axes = ["method", "bean"]
```

`preset_axes` is a `Vec<String>` of field names, in drilldown order. Empty or absent → no picker; the legacy Left/Right cycler stays.

__Validation rules__ (lazy, checked at form open): an axis name is invalid if:
- The field does not exist in the module
- The field type is `composite_array`
- The field has `preset_exclude = true`

Invalid axes produce a footer warning but do not prevent the form from opening; the cycler falls back.

## Data Model

`src/data/preset_tree.rs` exposes three pure functions:

```rust
pub fn build(presets: &[PresetEntry], axes: &[String]) -> PresetTree;
pub fn validate_axes(axes: &[String], fields: &[FieldConfig]) -> Result<(), Vec<AxisError>>;
pub fn suggest_preset_name(values: &HashMap<String,String>, axes: &[String]) -> String;
```

__`PresetTree`__ has two lists at root: `roots` (axis-drilled branches, sorted alphabetically) and `ungrouped` (presets missing any axis value, shown at the bottom of root).

__`TreeNode`__ is either:
- `Branch { axis_value, children, count }` — a grouping node; count aggregates all leaves below
- `Leaf { preset_name, description }` — a selectable preset

__Rules:__
- Branches are sorted alphabetically at every level.
- Leaves preserve the saved order from `presets.json`.
- A preset whose axis value is absent or empty is placed in `ungrouped`.
- A single child at a level is still shown (no auto-drill).

## UX — Drilldown Picker

__Open__: bare `p` on the preset row, or `Ctrl+P` from any non-editing context (no picker fires when `preset_axes` is empty — falls through to cycler).

__Navigation inside the picker__:

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate items in the current level |
| `Enter` | Drill into a branch, or apply a leaf preset |
| `Backspace` / `←` | Pop back one level (or close at root) |
| `Esc` | Cancel — no preset applied |

__Header__: breadcrumb showing the drilled path + next axis label, e.g. `V60 ▸ bean`.

__Footer hint__: `↑↓ nav  Enter select  Bksp/← back  Esc cancel`

__Scroll__: viewport tracks selected item; a `selected/total (pct%)` indicator appears when the list exceeds the visible area.

## Identity Migration

`FormState.selected_preset: usize` (positional) is replaced by `selected_preset_name: Option<String>` (name-keyed). This makes the selection survive preset reorder operations without needing to recompute an index.

All code paths that previously compared `selected_preset == 0` / `selected_preset > 0` now check `selected_preset_name.is_none()` / `.is_some()`.

## Inline Row Display

When `preset_axes` is non-empty:
- The `◂ name ▸` chevrons are __not shown__ — `p` replaces cycling.
- The footer shows `p picker` instead of `←→ cycle`.
- `Left`/`Right` bare keys on the preset row are no-ops (Ctrl+Left/Right reorder still works).

When `preset_axes` is empty:
- Chevrons and cycler behaviour are unchanged.

## Save Dialog Auto-Suggest (Phase C)

When opening the save dialog for a __new__ preset (no preset currently selected), the name field is pre-filled with:

```
<axis0_value> · <axis1_value> · ...
```

Empty axis values are skipped — no trailing separator. The middle-dot separator is U+00B7 surrounded by spaces.

When __editing an existing preset__, the name field is pre-filled with the existing name (unchanged behaviour).

The user can edit the suggested name freely. Any keystroke in the name field sets `name_was_user_edited = true`.

## Overwrite Confirm

If the user enters a name that collides with an existing preset (and is not editing that same preset), the first Enter shows:

```
Overwrite "<name>"?  Enter confirm  Esc cancel
```

A second Enter proceeds. Any keystroke clears the confirm state.

## Edge Cases

- __Ungrouped__: presets with a missing or empty axis value appear under a flat "Ungrouped" entry at the bottom of the root list.
- __Single child__: a branch with a single child still displays the level — no auto-drill.
- __Numeric axis values__: `dose: "18"` renders as `18 ▸ bean`. Allowed; no coercion.
- __Axis value containing ` · `__: would produce an ambiguous auto-suggested name. Not sanitized; documented here as known-but-allowed.
- __Validation warning__: a typo in `preset_axes` (e.g. `"beann"`) surfaces as a footer warning. The form still opens; the cycler is active as fallback.

## Known Limitations

- __Picker scroll viewport__: the viewport is hardcoded to 20 rows in the key handler (`Down` key clamps at `viewport_offset + 20`). On terminals shorter than ~25 rows the lower portion of long preset lists becomes keyboard-unreachable. Tracked for follow-up; the fix is to compute window height from the terminal area at key-handling time rather than using a constant.

## PWA Forward-Compatibility

The `preset_axes` config key is per-module in `config.toml` and is included in `GET /api/v1/config` responses when the PWA roll-forward is implemented. The data shape (a flat list of `PresetEntry` with their `values` map) is forward-compatible with a mobile drilldown — the same config drives both surfaces. PWA drilldown is a separate plan; this spec gates it.
