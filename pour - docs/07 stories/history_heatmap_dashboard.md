---
tags:
  - story
  - spec
  - history
  - tui
aliases:
  - history heatmap
  - heatmap dashboard
  - history dashboard
date created: Tuesday, April 21st 2026, 10:15:48 pm
date modified: Wednesday, April 29th 2026, 5:31:51 pm
---

# History Heatmap Dashboard

## The Vibe

You open `pour` and the dashboard already tells you about *now* — last pour, today count, streak. But it can't show you *shape*. Did you pour more in March or February? Was last Wednesday a dead zone or a flood? You can feel the rhythm of your captures, but you can't see it.

A GitHub contribution graph solves this exact problem — it turns a year of discrete events into a single glanceable shape. Dense green = active. Gray = quiet. The shape tells a story without reading a single number.

Pour should have this. Not because it's pretty (it is), but because the heatmap *is* the behavioral feedback loop the manifesto talks about — seeing your capture rhythm makes you want to sustain it.

## Screen: `pour history` or `Ctrl+H` from the Dashboard

A new TUI screen, not a replacement for the dashboard. The dashboard stays fast and ambient. The heatmap is the deep view you pull up when you want to reflect.

### Layout

```
 History                                        Apr 2026
 ─────────────────────────────────────────────────────────
     W1    W2    W3    W4    W5   ...   W48   W49   W50
 Mon  ░░    ██    ░░    ▓▓    ██         ░░    ██    ▓▓
 Tue  ██    ██    ░░    ░░    ▓▓         ██    ░░    ██
 Wed  ▓▓    ░░    ██    ██    ░░         ▓▓    ██    ░░
 Thu  ░░    ▓▓    ▓▓    ██    ██         ░░    ▓▓    ██
 Fri  ██    ██    ░░    ▓▓    ░░         ██    ██    ░░
 Sat  ░░    ░░    ░░    ░░    ░░         ░░    ░░    ░░
 Sun  ░░    ░░    ░░    ░░    ░░         ░░    ░░    ░░

 ░ 0   ▒ 1-2   ▓ 3-5   █ 6+           Total: 1,247

 ─────────────────────────────────────────────────────────
 Selected: Wed Mar 11 — 4 pours (coffee x3, me x1)
 ─────────────────────────────────────────────────────────
 ← → navigate weeks   ↑ ↓ navigate days   q back
 T trim before cursor   Esc cancel
```

### Grid Rendering

- __Rows__: Mon–Sun (7 rows).
- __Columns__: Weeks. Default viewport shows ~50 weeks (fits in 80-col terminal at 1 char per cell + spacing). Scroll horizontally for older history.
- __Cells__: Single Unicode block character. Intensity mapped to daily capture count:
  - `░` (light shade) — 0 captures
  - `▒` (medium shade) — 1–2
  - `▓` (dark shade) — 3–5
  - `█` (full block) — 6+
- __Color__: Monochrome by default. Optional per-module color tinting in a future pass.
- __Right-aligned__: Most recent week at the right edge (like GitHub). Scroll left for older history.

### Cell Thresholds

The 4-tier thresholds (0, 1–2, 3–5, 6+) should be adaptive based on the user's actual capture volume. Compute the 25th/50th/75th percentile of non-zero daily counts from the loaded history, use those as the tier boundaries. Fall back to the hardcoded defaults above if the history has fewer than 14 non-zero days (not enough data for meaningful percentiles).

### Navigation

| Key | Action |
|-----|--------|
| `Left` / `Right` | Move cursor one week |
| `Up` / `Down` | Move cursor one day (Mon-Sun) |
| `Home` | Jump to oldest entry |
| `End` | Jump to today |
| `q` / `Esc` | Return to dashboard |

### Detail Pane

Below the grid, a 1–2 line detail pane shows the selected day:

```
Wed Mar 11 — 4 pours (coffee x3, me x1)
```

If the cursor is on a day with 0 captures: `Wed Mar 11 — no captures`.

### Data Source

Reads from the in-memory `Vec<HistoryEntry>` (already loaded at startup from `history.jsonl`). No additional I/O. The heatmap is a pure view over data that already exists.

For the grid, precompute a `HashMap<NaiveDate, usize>` (date -> count) and a `HashMap<NaiveDate, HashMap<String, usize>>` (date -> module -> count) once on screen entry.

---

## Feature: Atomic History Trim

### The Problem

History is append-only and preserved forever by default. But "forever" is a policy, not a constraint. Users should be able to reclaim space or discard old data they no longer care about — but it must be intentional, explicit, and safe.

### UX: Trim Before Cursor

From the heatmap screen, the user navigates to a date and presses `T`. This initiates a trim of all entries *strictly before* that date.

__Confirmation flow__ (this is destructive — no undo):

```
 ┌─────────────────────────────────────────────┐
 │  Trim history before Wed Mar 11, 2026?      │
 │                                              │
 │  This will permanently delete 847 entries    │
 │  spanning 2025-01-15 to 2026-03-10.         │
 │                                              │
 │  This cannot be undone.                      │
 │                                              │
 │  Type "trim" to confirm:  ░░░░░░            │
 │                                              │
 │  Esc to cancel                               │
 └─────────────────────────────────────────────┘
```

Typing the word "trim" (not just pressing Enter) is the safety gate. This matches the destructive-action UX pattern of tools like `gh repo delete`.

### Implementation: Atomic Trim

The trim operation must be atomic — no partial state on crash or error.

__Steps:__

1. __Filter__: Partition `Vec<HistoryEntry>` into `keep` (>= trim date) and `discard` (< trim date).
2. __Write new file__: Write `keep` entries to `history.jsonl.tmp` as JSONL.
3. __Sync__: `file.sync_all()` — ensure data is on disk.
4. __Replace__: `atomic_replace("history.jsonl.tmp", "history.jsonl")` — same utility used by summary cache writes.
5. __Recompute summary__: `compute_summary(&keep)` and write `history-summary.json`.
6. __Update in-memory state__: Replace `self.entries` with `keep`, update `self.summary`.

If any step fails, the original `history.jsonl` is untouched. The `.tmp` file is the only artifact and can be cleaned up on next load.

__Why not append-only for trims?__ A trim is inherently a rewrite — you're removing data from the middle/beginning of the log. The atomic tmp+rename pattern is the right tool here. Trims are rare (user-initiated, confirmed), so the one-time cost of a full rewrite is fine.

### CLI Alternative

`pour trim --before 2026-03-11` with the same confirmation prompt (type "trim"). This gives scriptable access without the TUI.

### History Method

```rust
impl History {
    /// Remove all entries before `cutoff_date` (exclusive).
    /// Atomic: writes a new JSONL file, syncs, then replaces the original.
    pub fn trim_before(&mut self, cutoff: NaiveDate) -> Result<TrimResult>;
}

pub struct TrimResult {
    pub removed: usize,
    pub remaining: usize,
}
```

---

## What This Doesn't Include (Yet)

- __Per-module heatmap filtering__ — e.g., show only coffee captures. Natural extension, but adds complexity to the grid rendering. Defer.
- __Export from heatmap__ — selecting a date range and exporting to CSV/JSON. The JSONL file is already the export format; `jq` and friends handle this.
- __Undo trim__ — intentionally absent. The confirmation flow is the safety net. If users want backup, they can `cp history.jsonl history.jsonl.bak` before trimming. Pour is not a backup tool.
- __Color themes / module tinting__ — the monochrome shade approach works in any terminal. Color is a nice-to-have for later.

## Open Questions

1. __Keybinding__: `Ctrl+H` from the dashboard, or a dedicated `pour history` subcommand, or both? Leaning both — `Ctrl+H` for when you're already in the TUI, subcommand for direct access.
2. __Adaptive thresholds__: The percentile-based approach means two users see different intensity for the same count. Is this desirable (personalized) or confusing (inconsistent)?
3. __Month labels__: GitHub shows month labels above the grid. Worth the vertical space, or just show the current cursor's month in the header?
