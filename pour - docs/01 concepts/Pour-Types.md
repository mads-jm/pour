---
tags:
  - concept
  - modes
  - habits
aliases:
  - pour types
  - capture shapes
date created: Thursday, August 6th 2026
date modified: Thursday, August 6th 2026
---

# Pour Types

There are three kinds of pour, one per write mode. Which kind a signal gets is decided by what the signal *is*, not by what is easiest to configure.

| Pour | Mode | Shape on disk | Examples |
|---|---|---|---|
| Entry | `append` | lines under a heading in a note that already exists | `pour me`, `pour todo`, `pour log` |
| Event | `create` | a new file with its own frontmatter | `pour coffee`, `pour note` |
| Ambient state | `update` | a frontmatter property on a periodic note, changed in place | `pour habit` |

## Entry

Something happened in the flow of the day and belongs in the day's note. A timestamp and some text under a heading. The note is the container; the entry has no identity of its own and nothing will ever link to it. When the API is down, pour writes an [[Atomic-Note-Fallback|atomic timestamped note]] instead of editing the daily note blind.

## Event

Something with enough nuance to deserve its own file. A coffee has a bean, a ratio, a taste. The frontmatter is the record and the filename is the identity, so later notes can link to it and queries can find it. This is the pour the [[the_pour_manifesto|manifesto]] was written about.

## Ambient state

The background hum of a day. Partaken or not. Ounces so far. It has no story beyond its value, so it is not worth a note and not worth a line. It lives as a property on the periodic note, the template owns the key and its default, and pour only ever mutates it.

This is the only pour that edits bytes the user already owns. Every other write adds a file or appends to a section. That is why `update` carries guard rails the other two do not need: stat before read, a single-line edit that never re-emits YAML, and an atomic replace. [[field-types]] has the rules; [[pour-habit-capture]] has the reasoning.

## The creed

> [!quote] Ambient state gets a property. Novel experience gets a note. Never both for the same signal.

Two corollaries that follow from it.

Derive, never mirror. If a signal can be counted from event notes (coffees today, from coffee notes), it is not ambient state. Storing it as a property too creates a second source of truth, and the two drift within a week.

The field name is the frontmatter key. Pour does not map names — `field.name` is what `output/update.rs` writes, with no indirection. The first live run of the habit module wrote a stray `water:` key next to the template's then-`water_oz:` for exactly this reason. Match the template, or change the template. That one resolved the second way: the daily template now tracks `water:`, the module's field is `water`, and `pour habit water 16` is the command.

## Choosing

Ask in this order.

1. Will anything ever link to it, or will you want to find *this one* later? Event.
2. Does it belong in the story of the day, as a thing that happened at a time? Entry.
3. Is it a fact about the day itself, with no story? Ambient state.

If a signal seems to want two of these, it is usually an event, and the ambient-looking part should be derived from the event notes.

See also: [[pour-design-spec]] §3.5 for the mode mechanics, [[the_habit_story]] for where the third pour came from.
