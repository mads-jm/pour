---
tags:
  - reference
  - keybindings
  - tui
date created: Saturday, April 25th 2026, 4:19:07 pm
date modified: Saturday, October 3rd 2026, 6:47:20 am
---

# Keyboard Shortcuts

Complete hotkey reference for all Pour TUI screens. Source: `src/tui/dashboard.rs`, `src/tui/form/` (`key/`, `overlays/`), `src/tui/configure/key/`, `src/tui/summary.rs`, `src/tui/loop_.rs`.

---

## Global

| Key | Action |
|-----|--------|
| `Ctrl+C` | Quit from any screen |

Pasted text arrives as typed characters, so pasting into a field works like typing it.

---

## Dashboard

The main screen reached by running `pour` with no arguments.

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate module list (wraps) |
| `Enter` | Launch selected module |
| `e` | Open module settings configurator for selected module |
| `v` | Open vault settings configurator |
| `n` | Add a new module |
| `r` | Refresh transport (re-probe API connection). Only shown in the footer when not on the API, but works either way |
| `o` | Open vault in Obsidian (fires `obsidian://` URL) |
| `s` | Serve the PWA. The TUI suspends and the server runs in the terminal on port 8421; `Ctrl+C` stops it and returns to the dashboard |
| `q` | Quit |
| `?` | Toggle help overlay |
| `Ctrl+Up` | Move selected module up in dashboard order |
| `Ctrl+Down` | Move selected module down in dashboard order |

### Path Warnings Overlay

Shown at startup when a module path has problems, or after `s` fails to bind the port. It takes every key until dismissed.

| Key | Action |
|-----|--------|
| `Enter` | Dismiss and continue to the dashboard |
| `e` | Dismiss and open the configurator for the module named in the first warning |

---

## Form (Module Entry Screen)

Reached by launching a module from the dashboard or via `pour <module>`. The form has a preset row at the top, the visible fields, and a submit row at the bottom.

### General Navigation

| Key | Action |
|-----|--------|
| `Up` | Move to previous row (wraps from preset row to submit row) |
| `Down` / `Tab` | Move to next row (wraps) |
| `Shift+Tab` | Move to previous row |
| `Enter` | Text, number, toggle and counter fields: advance to next row. Select, textarea and composite fields: open their overlay. Submit row: submit |
| `Esc` | Steps back one level: clears a select search, else closes an open dropdown, textarea editor or table, else clears the field's value, else returns to the dashboard. On the preset row or submit row it returns to the dashboard |
| `Ctrl+R` | Collapse or expand the priors panel. Any module can show it: without a `[modules.<name>.priors]` block, pour uses a zero-config default. View only; never changes values |

Moving between rows with `Up`, `Down`, `Tab` or `Shift+Tab` closes any open dropdown, textarea editor or table.

### Text Fields

| Key | Action |
|-----|--------|
| Printable characters | Insert at cursor (`number` fields accept only digits, `.` and `-`) |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor. A value wider than its row scrolls sideways to keep the cursor in view, with `◂`/`▸` marking hidden text |

### Toggle Fields

| Key | Action |
|-----|--------|
| `Space` | Flip between `true` and `false` (footer hint: `space flip`) |
| `Enter` | Advance to next row |

Other characters are ignored on a toggle.

### Counter Fields

| Key | Action |
|-----|--------|
| `0`-`9`, `.`, `-` | Type an amount to add to the stored value (footer hint: `0-9 add`) |
| `=` | Start the value with `=` to set the stored value instead of adding (footer hint: `= set`) |
| `Backspace` / `Left` / `Right` | Edit as a text field |

### Textarea Fields (Editor Overlay)

| Key | Action |
|-----|--------|
| `Enter` | Open overlay editor (when overlay closed); insert a newline (when open) |
| Printable characters | Insert at cursor (only while the editor is open) |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor (editor open). Long lines scroll sideways to keep the cursor in view |
| `Up` / `Down` | Move cursor between lines (editor open). The editor scrolls to keep the cursor's line in view, with `▲`/`▼` on its border when lines are hidden |
| `Esc` | Close overlay editor |
| `Left` / `Right` | Cycle callout type (when overlay is closed and a callout is active) |
| `t` / `T` | Edit callout title (focused on textarea row with an active callout, overlay closed) |

### Callout Title Editor

| Key | Action |
|-----|--------|
| Printable characters | Insert at cursor (up to 120 characters) |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor |
| `Home` / `End` | Cursor to start or end |
| `Enter` | Confirm title (an empty title clears it) |
| `Esc` | Cancel |

### Select Fields (Dropdown Overlay)

| Key | Action |
|-----|--------|
| `Enter` | Toggle dropdown open/closed |
| `Up` / `Down` | Cycle options while open. The highlighted option becomes the value right away |
| `Left` / `Right` | Cycle options inline when dropdown is closed |
| `Esc` | Close dropdown |
| Printable characters | Filter options and open the dropdown (`static_select` or `dynamic_select` with `allow_create = true`) |
| `Backspace` | Trim search buffer (`allow_create = true`) |
| `Enter` (with search text) | Pick the highlighted match. With no match, use the typed text as a new value: `static_select` appends it to the field's `options` in `config.toml`, and a field with `create_template` opens the sub-form overlay instead (`allow_create = true`) |

### Sub-form Overlay (create_template)

Opens when `Enter` creates a new value on a select field that has `create_template`.

| Key | Action |
|-----|--------|
| `Down` / `Tab` | Next row (wraps) |
| `Up` / `Shift+Tab` | Previous row (wraps) |
| Printable characters | Type into the field (`number` accepts only digits, `.` and `-`) |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor; on a `static_select` field, cycle options |
| `Enter` | Advance to next row; on the submit row, create the note |
| `Esc` | Cancel |

### Composite Array Fields (Table Overlay)

| Key | Action |
|-----|--------|
| `Enter` | Open table editor overlay |
| Printable characters | Type into the current cell (`number` columns accept only digits, `.` and `-`; `s`, `l` and `p` are taken by the preset keys below) |
| `Backspace` | Delete character before cursor in the cell |
| `Up` / `Down` | Move between rows |
| `Left` / `Right` | Move cursor in the cell; on a `static_select` column, cycle options |
| `Space` | On a `static_select` column, cycle to the next option |
| `Tab` | Advance to next cell (wraps to the next row) |
| `Shift+Tab` | Go back one cell |
| `Enter` (in table) | Insert a new row below the current one |
| `Delete` | Delete current row |
| `s` | Save current rows as a per-field preset |
| `l` | Open the per-field preset picker |
| `p` | Quick-cycle to the next saved per-field preset |
| `Esc` | Close table overlay |

### Per-field Preset Picker (Composite Overlay)

Appears when `l` is pressed inside a composite_array editor with at least one saved preset.

| Key | Action |
|-----|--------|
| `Up` / `Down` | Move selection (wraps) |
| `Enter` | Apply selected preset (replaces existing rows) |
| `Ctrl+D` | Delete selected preset |
| `Esc` | Cancel and close picker |

### Preset Row

| Key | Action |
|-----|--------|
| `Left` / `Right` | Cycle through saved presets (and `<none>`). Only when the module has no `preset_axes` |
| `Ctrl+Left` | Reorder selected preset backward |
| `Ctrl+Right` | Reorder selected preset forward |
| `s` (on preset row or submit row) | Save current form values as preset (name prompt appears) |
| `Ctrl+S` (any field, unless a textarea editor or table is open) | Save current form values as preset |
| `d` (on preset row, real preset selected) | Delete selected preset (y/n confirmation) |
| `p` (on preset row) / `Ctrl+P` | Open the preset picker. Only when the module has `preset_axes` |

### Preset Picker (`preset_axes`)

Presets grouped by the module's `preset_axes`, one level per axis.

| Key | Action |
|-----|--------|
| `Up` / `Down` | Move selection |
| `Enter` | Open a group, or apply the selected preset |
| `Backspace` / `Left` | Go up one level; at the top level, close |
| `Esc` | Close picker |

### Delete Confirmation Dialog

| Key | Action |
|-----|--------|
| `y` / `Y` | Confirm delete |
| `n` / `N` / `Esc` | Cancel |

---

## Summary (After Submit)

| Key | Action |
|-----|--------|
| `Enter` | Back to the dashboard |
| `a` | Start another entry in the same module |
| `o` | Open the written note in Obsidian |
| `q` | Quit |

---

## Configure (Module and Vault Settings)

Reached via `e` (module settings) or `v` (vault settings) from the dashboard.

### Settings List Navigation

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate settings rows |
| `Enter` | Act on selected setting: text and identifier rows start editing, path rows open the vault browser, toggle rows cycle options, list rows open the list editor, quick-select rows open the quick-select overlay, link rows (Fields, Sub-fields) open that sub-screen |
| `e` | Start freetext editing on any field (including Path and Identifier) |
| `s` | Save settings to `config.toml` and stay on the screen (not available in New Module mode) |
| `d` | Delete the entire module (ModuleSettings only; y/n confirmation) |
| `?` | Open placeholder help overlay (Path fields only) |
| `Esc` | Leave without saving: field editor goes back to the field list, sub-field editor to the sub-field list, everything else to the dashboard |

### New Module Mode

| Key | Action |
|-----|--------|
| `Ctrl+S` | Save the new module definition |
| `Esc` | Cancel and discard the new module |

### Freetext Edit Mode (Active when Editing a Field Value)

| Key | Action |
|-----|--------|
| Printable characters | Insert at cursor (Identifier fields accept only `a-z`, `A-Z`, `0-9`, `_` and `-`) |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor |
| `Enter` | Confirm edit |
| `Esc` | Cancel edit, restore original value |
| `?` | Open placeholder help overlay (Path fields only) |

### List Editor (Options Arrays)

One option per line.

| Key | Action |
|-----|--------|
| Printable characters | Insert at cursor |
| `Enter` | New line |
| `Backspace` | Delete character before cursor; at line start, join with the previous line |
| Arrow keys | Move cursor |
| `Ctrl+S` | Keep the edited list |
| `Esc` | Discard changes |

### Quick-select Overlay (Callout Type)

| Key | Action |
|-----|--------|
| Option hotkey | Pick that option |
| `Backspace` | Clear the value |
| `Esc` | Close |

### Confirmation Dialog

| Key | Action |
|-----|--------|
| `y` | Confirm delete |
| `n` / `Esc` | Cancel |

### Field List (Sub-screen within Module Settings)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate fields |
| `Ctrl+Up` | Reorder field upward |
| `Ctrl+Down` | Reorder field downward |
| `n` | Add a new field |
| `d` | Delete selected field (y/n confirmation) |
| `Enter` | Open field editor for selected field (on `< Back`, return to module settings) |
| `Esc` | Return to module settings |

### Sub-field List (Within a composite_array Field Editor)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate sub-fields |
| `Ctrl+Up` | Reorder sub-field upward |
| `Ctrl+Down` | Reorder sub-field downward |
| `n` | Add a new sub-field |
| `d` | Delete selected sub-field (y/n confirmation) |
| `Enter` | Open sub-field editor |
| `Esc` | Return to field editor |

---

## Browse (Vault Directory Browser)

Opened from path fields inside the configurator. It lists directories only.

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate directory entries |
| `Enter` | Descend into the selected directory (`..` goes up) |
| `Tab` | Use the selected directory as the value. For a module `path`, Pour appends `/{date_format}.md` and drops into freetext edit so you can adjust the filename |
| `Backspace` | Go up one directory level |
| `Esc` | Cancel and return to configurator |

---

## Preset name Overlay

Appears when saving a preset. It has a name line and a description line.

| Key | Action |
|-----|--------|
| Printable characters | Type into the focused line (name up to 50 characters, description up to 120) |
| `Backspace` | Delete character |
| `Left` / `Right` | Move cursor |
| `Tab` / `Shift+Tab` / `Up` / `Down` | Switch between name and description |
| `Enter` | Save preset. If the name belongs to a different existing preset, asks first; press `Enter` again to overwrite |
| `Esc` | Cancel |

---

## Help Overlay

Opened via `?` from the dashboard or a path field in the configurator.

| Key | Action |
|-----|--------|
| `?` / `Esc` | Close help overlay |
