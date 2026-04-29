---
tags:
  - story
  - vision
  - manifesto
aliases:
  - the pour manifesto
  - manifesto
date created: Tuesday, March 31st 2026, 12:24:04 am
date modified: Wednesday, April 29th 2026, 5:31:43 pm
---

# The Pour Manifesto: Why We Build

## The Problem: The Tragedy of the Uncaptured Thought

[[obsidian-local-rest-api|Obsidian]] is a beautiful, sprawling garden for our minds. It is the perfect place to wander, connect ideas, and synthesize knowledge.

But going to it takes too long.

When you are deep in a flow state, dialing in the perfect espresso shot, or standing in the crowd at a festival, your brain operates in seconds, not minutes. If the process of logging a thought takes longer than the thought itself, the thought dies.

Switching apps, navigating to the right folder, opening the daily note, and formatting the frontmatter by hand—that isn't just friction. It is a barrier to entry. It makes the act of remembering feel like administrative work. And because of that friction, we stop writing. We lose the nuances of our days.

## The Vision: Writing More About What Matters

^vision

We don't log data just to have data.

We log coffee ([[pour-design-spec|`pour coffee`]]) because we are chasing the perfect extraction, and we want to remember how that Ethiopian light roast tasted at a 1:15 ratio.

We log music (`pour music`) because the energy of a live set and the people we shared it with are memories worth keeping.

We log ourselves ([[pour-design-spec|`pour me`]]) because our passing thoughts, midnight epiphanies, and daily anxieties deserve a place to rest outside our own heads.

We want to write more about the things that bring us joy. To do that, the tool must get out of the way entirely.

## The Ethos of Pour

^ethos

> [!quote] Pour is not a workspace. It is a reflex.

It is a capture tool that meets you where you already are — terminal at the desk, [[ADR-005-PWA-Companion|PWA]] in your pocket — designed to be as fluid and instantaneous as the gesture that invokes it.

- __Velocity is a Feature:__ The time between having a thought and executing the capture must be near zero. `pour` lives where you already are — your terminal at the desk, your phone in the wild — and is just the tool you need at the moment you need it. No cold-start app launch, no folder to find, no schema to format by hand. The phone surface costs more friction than the terminal; that cost is worth paying when the alternative is forgetting the thought entirely. See [[Capture-Reflex]].
- __Capture First, Synthesize Later:__ `pour` separates the act of recording from the act of organizing. You pour the raw data into the vault flawlessly formatted. You can open [[obsidian-local-rest-api|Obsidian]] on Sunday to make sense of it all.
- __Plaintext is Forever:__ We reject proprietary databases. A memory should not be locked behind a subscription or a specific app version. Everything `pour` generates is strict, portable Markdown and YAML — see [[field-types]] for the full vocabulary. It belongs to you.
- __Fluidity:__ The name is the instruction. You don't "execute a script" or "insert a database row." You *pour* a V60. You *pour* your thoughts. Type the verb at the desk, drag the gesture in your pocket — the act is the same. It is a continuous, natural motion.

## The User Story

^user-story

> As a developer and enthusiast, I want a frictionless capture surface — terminal when I'm at the desk, [[ADR-005-PWA-Companion|PWA]] when I'm not — to log highly structured data (like coffee recipes and concert memories) directly into my [[obsidian-local-rest-api|Obsidian]] vault, so that I can capture the details of the things I love without breaking my workflow.

See also: [[pour-design-spec|the design spec]] for *how* this is built, [[v1.0.0-Release|v1.0.0]] for the freeze criteria, and [[pour_without_obsidian]] for the architecture's portability beyond Obsidian.











