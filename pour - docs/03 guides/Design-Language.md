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
date modified: Friday, April 3rd 2026, 4:11:43 am
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
▽ pour   [local]
```

The front door. One branded mark, module list below as a clean launcher, transport badge as system state.

Empty state: `no modules configured. add modules to config.toml.`

### Form Header

```ts
▽ pour coffee — Brew Log
```

The strongest expression of the product. The `▽` anchors the command lockup. Display name follows in dark gray as a subtitle. Submit reads `[ submit ]` — lowercase, tool-voiced.

### Summary

```ts
▽ saved                    (success — green bold)
! error                    (failure — red bold)
```

The brand mark signals completed flow. Dropping it on error is intentional — the funnel failed, the pattern breaks, the red styling does the rest.

Body labels are terse and lowercase:

```ts
  path: 02-Logbook/2026-04-01-brew.md
  transport: api
```

### Configure Header

```ts
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



