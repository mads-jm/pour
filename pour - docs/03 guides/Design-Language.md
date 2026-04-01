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
date modified: Tuesday, April 1st 2026, 12:00:00 am
---

# Pour Design Language

## Brand Mark

Primary: `▽`
Fallback: `v`

`▽` reads as a funnel, dripper, intake cone. It covers coffee, music, and thought capture without collapsing into one domain. It survives any terminal font. Nerd Font glyphs are acceptable as optional enhancements but never as the primary mark.

## Voice

The TUI should feel **sharp**, **ritualized**, **command-forward**, and **quietly technical**.

It should not feel cozy, corporate, menu-first, or playful. The product is about immediate capture under flow. The interface is a precision tool you trust at 1am, not a friendly dashboard.

### Copy rules

Text reads like a tool, not an app.

```
▽ pour coffee        not    Welcome to Pour
▽ saved              not    Your entry was successfully written
transport: api       not    Transport Mode: API
```

Lowercase. Terse. Imperative. Almost command-line dry.

## Implemented Surfaces

### Dashboard header

```
▽ pour   [local]
```

The front door. One branded mark, module list below as a clean launcher, transport badge as system state.

Empty state: `no modules configured. add modules to config.toml.`

### Form header

```
▽ pour coffee — Brew Log
```

The strongest expression of the product. The `▽` anchors the command lockup. Display name follows in dark gray as a subtitle. Submit reads `[ submit ]` — lowercase, tool-voiced.

### Summary

```
▽ saved                    (success — green bold)
! error                    (failure — red bold)
```

The brand mark signals completed flow. Dropping it on error is intentional — the funnel failed, the pattern breaks, the red styling does the rest.

Body labels are terse and lowercase:

```
  path: 02-Logbook/2026-04-01-brew.md
  transport: api
```

### Configure header

```
▽ configure coffee   [modified]
```

Intentionally more utilitarian than the form. Module key only — display name omitted. This is maintenance mode, not the main ritual. Browser overlay title: `browse: {path}`.

## Color

| Role | Color | Usage |
|---|---|---|
| Brand / active | Cyan | Headers, active field labels, selected items |
| Success | Green | `▽ saved`, submit button |
| Interaction | Yellow | Key hints, transport badge, [modified] tag, browser borders |
| Failure | Red | `! error`, validation messages |
| Scaffolding | Dark Gray | Inactive labels, subtitles, kind hints |
| Content | White | Active field values, body text |
| Inactive | Gray | Inactive field values |

No centralized theme file. All styling is inline via ratatui's `Style` builder.

## Rules

1. **One `▽` per screen.** It belongs in the header. Not on list items, footers, or popups.
2. **Angular geometry.** Brackets, dividers, compact tags. No rounded emoji-style symbols.
3. **Contrast is functional.** Cyan for active, dark gray for scaffolding, yellow for hints. Never decorative.
4. **No coffee tropes.** No mug icons, steam motifs, beans, or cafe signage.
5. **No terminal cosplay.** No ASCII art banners, heavy box drawing, or CRT gimmicks.
6. **Headers establish context in one line.** Brand, module, state.
7. **Footers are operational.** Key hints only. Not a branding surface.

## SVG Guidance

If the mark becomes an SVG for web or docs:

- Outline cone
- One centered drop or point of flow
- No mug handle, no steam, no dense detail

Same geometry as `▽`, same restraint.
