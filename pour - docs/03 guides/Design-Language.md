---
tags:
  - note
  - branding
  - tui
  - design
aliases:
  - pour design language
  - pour icon direction
date created: Tuesday, March 31st 2026, 11:20:00 pm
date modified: Friday, October 2nd 2026, 11:30:00 pm
---

# Pour Design Language

## Brand Mark

Primary: `▽`
Fallback: `v`

`▽` reads as a funnel, dripper, intake cone. It covers coffee, music, and thought capture without collapsing into one domain. It survives any terminal font. Nerd Font glyphs are acceptable as optional enhancements but never as the primary mark.

## Voice

The TUI should feel __sharp__, __ritualized__, __command-forward__, and __quietly technical__.

It should not feel cozy, corporate, menu-first, or playful. The product is about immediate capture under flow. The interface is a precision tool you trust at 1am, not a friendly dashboard.

### Copy Rules

Text reads like a tool, not an app.

```ts
▽ pour coffee        not    Welcome to Pour
▽ saved              not    Your entry was successfully written
transport: api       not    Transport Mode: API
```

Lowercase. Terse. Imperative. Almost command-line dry.

## Implemented Surfaces

### Dashboard Header

```ts
▽ pour  ›  main                 14:32                 v1.1.0  [API]
```

The front door. One branded mark, module list below as a clean launcher, transport badge as system state. The vault's folder name sits after the mark, a clock in the middle, the version and badge on the right. The badge reads `[API]` in green or `[File System]` in yellow.

Empty state: `no modules configured. add modules to config.toml.`

### Form Header

```ts
▽ pour coffee — Brew Log
```

The strongest expression of the product. The `▽` anchors the command lockup. Display name follows in dark gray as a subtitle. Submit reads `[ pour ]` — lowercase, tool-voiced.

The footer names the keys the focused field answers to. Most fields show `Enter interact`. A toggle shows `space flip`, and a counter shows `0-9 add` and `= set`. A hint that names a dead key is worse than no hint.

Text that doesn't fit stays in its row. A long single-line value scrolls sideways, with a dark gray `◂` or `▸` at the edge that hides text. The textarea popout uses the same marks for long lines and puts `▲`/`▼` on its border when lines are hidden above or below. While the popout is open, the field row shows only `[^]`, so the value is never drawn twice.

### Priors Panel

```ts
┌ bean · method · rating desc ─┐
│dose_g  time_s                │
│18  165                       │
│repeat: 18 · 165              │
│3 of 12 captures              │
└──────────────────────────────┘
```

Read-only reference, so it stays quiet. Dark gray border and column names, white rows, and dark gray rows for captures that lack the `rank_by` value. The `repeat:` line is cyan. The title names the fields that matched, then the ranking when every row has a value for it. Collapsed with `Ctrl+R`, it shrinks to one cyan line: `▸ repeat: 18 · 165 — ^R for rows`. With no match it draws nothing, not an empty box.

### Summary

```ts
▽ saved                    (success — green bold)
! error                    (failure — red bold)
```

The brand mark signals completed flow. Dropping it on error is intentional — the funnel failed, the pattern breaks, the red styling does the rest.

Sound: one short synthesized tone on `▽ saved`, off by default (`[sound] on_save = true`). Nothing else in pour makes a sound. The tone does the green header's job of confirming a finished capture, which is why it sits alongside "not playful" rather than against it. `! error` stays silent.

Body labels are terse and lowercase:

```ts
  path: 02-Logbook/2026-04-01-brew.md
  transport: API
```

The value after `transport:` is the mode's display name, `API` or `File System`, and is not lowercased.

### Configure Header

```ts
▽ configure coffee   [modified]
```

Intentionally more utilitarian than the form. Module key only — display name omitted. This is maintenance mode, not the main ritual. Browser overlay title: `browse: {path}`.

## Color

| Role | Color | Usage |
|---|---|---|
| Brand / active | Cyan | Headers, active field labels, selected items |
| Success | Green | `▽ saved`, submit button, `[API]` badge |
| Interaction | Yellow | Key hints, `[File System]` badge, summary transport value, [modified] tag, browser borders |
| Failure | Red | `! error`, validation messages |
| Scaffolding | Dark Gray | Inactive labels, subtitles, kind hints |
| Content | White | Active field values, body text |
| Inactive | Gray | Inactive field values |

No centralized theme file. All styling is inline via ratatui's `Style` builder.

## Rules

1. __One `▽` per screen.__ It belongs in the header. Not on list items, footers, or popups.
2. __Angular geometry.__ Brackets, dividers, compact tags. No rounded emoji-style symbols.
3. __Contrast is functional.__ Cyan for active, dark gray for scaffolding, yellow for hints. Never decorative.
4. __No coffee tropes.__ No mug icons, steam motifs, beans, or cafe signage.
5. __No terminal cosplay.__ No ASCII art banners, heavy box drawing, or CRT gimmicks.
6. __Headers establish context in one line.__ Brand, module, state.
7. __Footers are operational.__ Key hints only. Not a branding surface.

## SVG Guidance

If the mark becomes an SVG for web or docs:

- Outline cone
- One centered drop or point of flow
- No mug handle, no steam, no dense detail

Same geometry as `▽`, same restraint.



