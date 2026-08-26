---
tags:
  - story
  - spec
  - review
  - tui
aliases:
  - priors panel
  - the priors panel
  - priors at the pour
  - review at capture
date created: Monday, July 13th 2026, 2:00:00 pm
date modified: Monday, July 13th 2026, 2:00:00 pm
---

# Priors at the Pour

## The Vibe

You're opening a new bag of beans. You've already added it to the vault — the bean note exists, the roaster is `[[Onyx]]`, it's a natural Ethiopian. You type `pour coffee`, pick the bean, pick `V60`. And right there — before you've weighed a single gram — you want to know: *what did I do last time this worked?*

Today the answer is a context switch. Leave the terminal, cold-start Obsidian, navigate to the Coffee folder, build or find a Dataview view, scan it, come back. By the time you're back, the kettle's boiling and the moment's gone. So you don't. You go by feel, pull an okay shot, and never close the loop.

The [[the_pour_manifesto|manifesto]] says *Capture First, Synthesize Later* — review is Obsidian's job, on Sunday. And that's true for **reflection**. But this isn't reflection. This is **decision support at the point of the pour**: the last few data points that make *this* extraction better. Showing your best prior shots while you dial in the next one isn't "synthesize later" — it's the most capture-first thing imaginable. It's velocity toward a *good* outcome, not just a logged one.

> [!quote] Bases is where you go to reflect. Priors is what's already there when you decide.

Obsidian [[pour-design-spec|Bases]] owns the Sunday view — open the vault, filter, admire a table. It cannot sit inside your terminal capture flow at the second you pick the bean. That gap is the whole feature.

## Screen: the Priors panel, inline in `pour coffee`

Not a new screen. A panel that appears *beside the form you're already filling*, the moment there's enough context to match on.

```
 pour coffee
 ┌ New Brew ──────────────┐ ┌ Onyx · V60 · your ⭐4+ ─┐
 │ bean   [Onyx Monarch]  │ │ 15g  1:16  3:00   ⭐5   │
 │ method [V60         ]  │ │ 15g  1:16  2:50   ⭐4   │
 │ dose   [__          ]  │ │ 16g  1:15  3:10   ⭐4   │
 │ ratio  [__          ]  │ │ ──────────────────────  │
 │ time   [__          ]  │ │ repeat: 15g · 1:16 · 2:55│
 │ ...                    │ │ 3 of 12 brews · matched  │
 └────────────────────────┘ │ roaster+method           │
                            └──────────────────────────┘
 ↑↓ field   ^R toggle priors   enter submit
```

Three things make it honest and useful, in this order of importance:

1. **The `repeat:` line** — the dialing target. The typical numbers from the shots that *worked*, not from all shots. This is the payload.
2. **The header** — `Onyx · V60 · your ⭐4+` — states exactly *what it matched on and ranked by*, so you trust the numbers instead of guessing where they came from.
3. **The rows** — texture. The spread behind the target.

You never asked a question. You never wrote a filter. The panel knew what to show because the config already declared it — the same way the form itself is the prompt. **Config declares the query; you never write one.** The instant you'd have to compose a filter, this has become Bases-in-a-terminal and betrayed the exact friction it exists to kill.

## The new-bag problem (the actual story)

Re-read the opening: *a **new bag***. The specific bean has **zero** history. If the panel matched on `bean`, it would be empty at the precise moment you invoked the story. So matching is a **cascade of decreasing specificity** — widen by dropping the most-specific key until something matches:

```
 match_on = ["bean", "roaster", "method"]

   bean history?      → empty (new bag)
     ↓ drop `bean`
   roaster + method   → "Onyx · V60 · your ⭐4+"     ← the sweet spot
     ↓ empty (new roaster too)
   method only        → "any roaster · V60"
     ↓ still nothing
   → "first V60 you're logging — go by feel"
```

The header always names the tier that matched. A panel that silently shows `any roaster` numbers under an `Onyx` heading is worse than no panel.

## Why this is a Pour primitive, not a coffee feature

The whole app is TOML-configurable to any domain, and so is this. The same panel serves:

- **`pour coffee`** — match `roaster` + `method`, rank by `rating desc`, summarize `dose/yield/time` as medians. *"Your best shots for this profile."*
- **`pour lift`** — match `exercise`, rank by `weight_g max`, summarize the top set. *"Your PR and recent working weights."*
- **`pour me`** — no rating, no numbers, no selects: fall back to recent-N of the same module, with `#tag`-in-body overlap when tags are present. *"What was on your mind last few entries."*
- **`pour music`** — match `venue`, summary shows the most-seen artist. *"You've been here before."*

One config vocabulary, four domains. If it can serve **coffee and `me` from the same keys**, it's a primitive. See [[pour-review-priors]] for the full schema, ranking model, match modes, and phasing.

## What this deliberately is not

- **Not a query language.** No filter/sort UI at capture time. Ever. The config is the query; the moment of capture stays low-load.
- **Not a browser.** No standalone "scroll all my coffee" screen in v1 — that's what Bases is for. This is glanceable decision support, not exploration. (`pour review <module>` as a standalone surface is a *possible* later graft on the same engine, not a goal.)
- **Not a replacement for Bases/Dataview.** It reads the same frontmatter Bases reads. Sunday reflection stays in Obsidian; this is Tuesday-morning, kettle's-on decision support.

## Relationship to the rest of Pour

- Shares plumbing with [[pour-lookup-fields]] — frontmatter read, wikilink resolution, the API→FS fallback ([[The-3-Tier-Data-Fallback]]). Should slot *behind* lookup-fields on the roadmap; both are the seedbed of the long-rumored `pour query` ([[pour_without_obsidian]]).
- Sibling to [[history_heatmap_dashboard]] in the "review" family: the heatmap reviews *cadence* (did I pour more in March?); Priors reviews *content* (what worked for this profile?).
- Data path mirrors [[ADR-001-Hybrid-Transport-Layer]]: Obsidian REST `/search/` when the API is up, filesystem scan + frontmatter parse when it's not.

See [[pour-review-priors]] for the spec.
