# Dedicated Terminal Homepage

## Premise

If `pour` is going to live in its own dedicated terminal, the homepage should stop behaving like a simple launcher and start behaving like an ambient capture surface.

The question is not just "what module do I open?" but:

- am I ready to capture right now?
- what have I logged recently?
- what part of life am I neglecting?

That shift supports the low-friction pillar more directly. A pinned terminal should make capture feel present, available, and already integrated into the day before a key is ever pressed.

## What The Homepage Should Optimize For

- immediate readiness
- visible recent activity
- gentle behavioral pressure
- one-key capture paths
- low visual noise

The dashboard should not lead with a menu. It should lead with state.

## Ambient Stats Worth Showing

Ambient stats are useful when they reinforce rhythm, not when they act like vanity metrics.

Good homepage stats:

- `last pour`: `18h ago`, `yesterday`, `4d ago`
- `today`: total captures today
- `this week`: total captures this week
- `streak`: days with at least one capture
- `by module`: `coffee 1`, `me 2`, `music 0`
- `transport`: `api` or `fs`
- `ready`: config valid, vault reachable, write path healthy

Most useful:

- `last pour`
- `today`
- `this week`
- per-module counts

The strongest metric is usually recency. `last pour: yesterday` creates more productive tension than a generic lifetime total ever will.

## Recommended Homepage Structure

```text
▽ pour
last pour: yesterday   today: 2   week: 9   streak: 4d   [api]

[c] coffee   [m] me   [u] music   [/] all modules

recent
> coffee   07:42
  me       12:11
  music    yesterday

gaps
  music    none this week
```

This layout does four things:

- establishes brand and readiness immediately
- makes capture shortcuts visible
- shows recent rhythm without opening history views
- highlights neglected categories without turning the UI into nagware

## Design Principles For A Dedicated Terminal

### 1. The homepage should be ambient first, navigational second

If `pour` is sitting open all day, it should still be useful when idle. The first screen should carry meaning even when the user is not interacting with it.

### 2. One-key capture matters more than menu navigation

A dedicated terminal invites muscle memory.

Prefer:

- `c` for coffee
- `m` for me
- `u` for music
- `/` or `Enter` for the full module list

The dashboard should reduce the number of keystrokes between seeing the terminal and beginning capture.

### 3. Show absence, not just activity

It is useful to know what is missing.

Examples:

- `music: none this week`
- `coffee: last logged 3d ago`
- `me: 0 today`

This turns the dashboard into a lightweight behavioral mirror. It encourages life integration without becoming preachy.

### 4. Keep the screen calm

A dedicated terminal should be comfortable to leave open for hours or days.

Avoid:

- rotating tips
- busy widgets
- decorative animations
- constant redraw noise
- dense tables

It should feel stable, sparse, and factual.

### 5. Surface operational trust

The terminal should quietly communicate whether capture is safe right now.

Useful status indicators:

- transport mode
- vault/API availability
- config validity
- draft or pending write state

That preserves confidence in the "capture instantly" promise.

## Revisiting The Low-Friction Pillar

Low friction is not only about making form entry fast. It is also about keeping the tool mentally present.

If the homepage shows useful ambient state, then `pour` becomes easier to integrate into life because:

- it is always visibly ready
- it reminds the user of capture rhythm
- it reduces recall burden
- it makes starting a capture feel immediate

The dashboard should not ask, first, "what do you want to do?"

It should say:

- you are ready
- here is your recent rhythm
- press one key

That is a better expression of "Pour is not a workspace. It is a reflex."

## Highest-Value Changes

If this is implemented incrementally, the highest-value additions are:

1. Add a top-line ambient status row with `last pour`, `today`, `week`, and transport state.
2. Add one-key shortcuts for the most-used modules directly on the homepage.
3. Add a small recent activity list.
4. Add one "gap" indicator showing what has not been logged recently.

That is enough to turn the homepage from a launcher into a living capture surface.
