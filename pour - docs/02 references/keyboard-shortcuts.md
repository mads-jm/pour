---
tags:
  - reference
  - keybindings
  - tui
date created: Monday, April 21st 2026, 12:00:00 am
date modified: Monday, April 21st 2026, 12:00:00 am
---

# Keyboard Shortcuts

Complete hotkey reference for all Pour TUI screens. Source: `src/tui/dashboard.rs`, `src/tui/form.rs`, `src/tui/configure.rs`.

---

## Dashboard

The main screen reached by running `pour` with no arguments.

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate module list |
| `Enter` | Launch selected module |
| `e` | Open module settings configurator for selected module |
| `v` | Open vault settings configurator |
| `n` | Add a new module |
| `r` | Refresh transport (re-probe API connection) |
| `o` | Open vault in Obsidian (fires `obsidian://` URL) |
| `q` | Quit |
| `?` | Toggle help overlay |
| `Ctrl+Up` | Move selected module up in dashboard order |
| `Ctrl+Down` | Move selected module down in dashboard order |

---

## Form (module entry screen)

Reached by launching a module from the dashboard or via `pour <module>`.

### General navigation

| Key | Action |
|-----|--------|
| `Up` | Move to previous visible field |
| `Down` / `Tab` | Move to next visible field |
| `Shift+Tab` | Move to previous visible field |
| `Enter` | Confirm field / open overlay / submit (on submit row) |
| `Esc` | Close active overlay; from submit row exits to dashboard |

### Text fields

| Key | Action |
|-----|--------|
| Printable characters | Append to input |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor |
| `Home` | Cursor to start of line |
| `End` | Cursor to end of line |

### Textarea fields (editor overlay)

| Key | Action |
|-----|--------|
| `Enter` | Open overlay editor (when overlay closed) |
| `Esc` | Close overlay editor |
| `Left` / `Right` | Cycle callout type (when overlay is closed and `callout` is configured) |
| `t` | Edit callout title inline (when focused on textarea row, overlay closed) |

### Select fields (dropdown overlay)

| Key | Action |
|-----|--------|
| `Enter` | Toggle dropdown open/closed; confirm selection when open |
| `Up` / `Down` | Cycle options while open |
| `Left` / `Right` | Cycle options inline when dropdown is closed |
| `Esc` | Close dropdown without confirming |
| Printable characters | Filter options (`dynamic_select` with `allow_create = true`) |
| `Backspace` | Trim search buffer (`dynamic_select` with `allow_create = true`) |

### Composite array fields (table overlay)

| Key | Action |
|-----|--------|
| `Enter` | Open table editor overlay |
| Arrow keys | Navigate cells |
| `Tab` | Advance to next cell |
| `Enter` (in table) | Add new row |
| `Delete` | Delete current row |
| `Esc` | Close table overlay |

### Preset row

| Key | Action |
|-----|--------|
| `Left` / `Right` | Cycle through saved presets (and `<none>`) |
| `Ctrl+Left` | Reorder selected preset backward |
| `Ctrl+Right` | Reorder selected preset forward |
| `s` (on preset row) | Save current form values as preset (name prompt appears) |
| `Ctrl+S` (any non-editing context) | Save current form values as preset |
| `d` (on preset row, real preset selected) | Delete selected preset (y/n confirmation) |

### Delete confirmation dialog

| Key | Action |
|-----|--------|
| `y` / `Y` | Confirm delete |
| `n` / `N` / `Esc` | Cancel |

---

## Configure (module and vault settings)

Reached via `e` (module settings) or `v` (vault settings) from the dashboard.

### Settings list navigation

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate settings rows |
| `Enter` | Edit selected setting (text/identifier starts editing; path opens vault browser; select cycles options) |
| `e` | Start freetext editing on any field (including Path and Identifier) |
| `s` | Save settings and return to dashboard (not available in New Module mode) |
| `d` | Delete the entire module (ModuleSettings only; y/n confirmation) |
| `?` | Open placeholder help overlay (Path fields only) |
| `Esc` | Cancel / return to previous screen |

### New Module mode

| Key | Action |
|-----|--------|
| `Ctrl+S` | Save the new module definition |
| `Esc` | Cancel and discard the new module |

### Freetext edit mode (active when editing a field value)

| Key | Action |
|-----|--------|
| Printable characters | Append to edit buffer |
| `Backspace` | Delete character before cursor |
| `Left` / `Right` | Move cursor |
| `Up` / `Down` | Navigate lines (list editor for options arrays) |
| `Enter` | Confirm edit |
| `Esc` | Cancel edit, restore original value |
| `Ctrl+S` | Save settings while in edit mode |

### Field list (sub-screen within module settings)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate fields |
| `Ctrl+Up` | Reorder field upward |
| `Ctrl+Down` | Reorder field downward |
| `n` | Add a new field |
| `d` | Delete selected field (y/n confirmation) |
| `Enter` | Open field editor for selected field |
| `Esc` | Return to module settings |

### Sub-field list (within a composite_array field editor)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate sub-fields |
| `Ctrl+Up` | Reorder sub-field upward |
| `Ctrl+Down` | Reorder sub-field downward |
| `n` | Add a new sub-field |
| `d` | Delete selected sub-field |
| `Enter` | Open sub-field editor |
| `Esc` | Return to field editor |

---

## Browse (vault directory browser)

Opened from path fields inside the configurator.

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate directory entries |
| `Enter` | Select entry (descend into directory or confirm file) |
| `Backspace` | Go up one directory level |
| `Esc` | Cancel and return to configurator |

---

## Preset name overlay

Appears when saving a preset.

| Key | Action |
|-----|--------|
| Printable characters | Type preset name |
| `Backspace` | Delete character |
| `Enter` | Confirm name and save preset |
| `Esc` | Cancel |

---

## Help overlay

Opened via `?` from the dashboard or path-field configurator.

| Key | Action |
|-----|--------|
| `?` / `Esc` | Close help overlay |
