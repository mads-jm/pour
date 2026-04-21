---
tags:
  - story
  - vision
  - exploration
aliases:
  - pour without obsidian
  - non-obsidian pour
date created: Tuesday, April 8th 2026, 12:00:00 am
date modified: Tuesday, April 8th 2026, 12:00:00 am
---

# Pour Without Obsidian: A Story for the Curious Outsider

## "Why not just use a notebook?"

Fair question. A paper notebook is 'zero-friction' capture — pen hits page, done. No boot time, no config files, no terminal. For a lot of things, paper wins.

But paper can't do what Pour exists to do:

- **Structure at the point of capture.** When you write "Ethiopian, 18g, 1:15, 28s, tasted like blueberries" in a notebook, that's a sentence. When Pour writes it, it's five typed fields in YAML frontmatter — queryable, sortable, graphable. You get the structure *while* you're capturing, not after a transcription step you'll never do.
- **Accumulation across time.** The twentieth coffee entry in a notebook is just page twenty. The twentieth entry in Pour is a dataset. You can answer "what's my average ratio for Ethiopian beans?" without re-reading anything. Paper accumulates pages; structured files accumulate knowledge.
- **Templates eliminate decisions.** A blank page asks "what should I write?" Pour asks "what was the dose?" The form *is* the prompt. You don't decide what to capture — you already decided that when you wrote the config. Now you just fill in blanks.
- **It lives where you already are.** If you're a developer, your terminal is open. `pour coffee` is three keystrokes away from whatever you were doing. A notebook is in a drawer, or a bag, or wherever you last left it.

None of this makes paper bad. Pour isn't replacing a journal — it's replacing the structured logging you *would* do if it weren't so tedious. The coffee ratios you'd track in a spreadsheet if you could be bothered to open one. The concert setlists you'd save if there were a way to do it in 20 seconds.

Paper captures thoughts. Pour captures data shaped like thoughts.

## "What does it actually do?"

Imagine you're standing in your kitchen. You just pulled a great shot of espresso — 18g in, 36g out, 28 seconds, the crema was perfect. You want to remember this. Not in a spreadsheet. Not in a note-taking app you'll forget to open. You want to type one command and be done.

```
pour coffee
```

A form appears in your terminal. You fill in the fields — bean, dose, ratio, method, tasting notes. Hit submit. A clean Markdown file appears in a folder on your disk with all your data structured in YAML frontmatter. Done. Back to your morning.

That's Pour. A fast, keyboard-driven terminal form that writes structured Markdown files to a folder.

## "So where does Obsidian come in?"

Obsidian is a note-taking app that reads a folder of Markdown files. That's its entire data model — a directory of `.md` files with YAML frontmatter at the top. There's no opinionated database, no proprietary format, no cloud lock-in.

Pour was built by an Obsidian user, for Obsidian users. The manifesto talks about "pouring into your vault" — but what it's *actually* doing under the hood is:

1. Writing `.md` files with YAML frontmatter to a directory
2. Optionally appending text under a heading in an existing `.md` file
3. Optionally talking to a local REST API that Obsidian exposes via a plugin

That's it. Items 1 and 2 are just filesystem operations on plain text files. Item 3 is a convenience — faster writes, richer directory listing — but Pour already falls back to the filesystem when the API isn't available.

**The files Pour generates are not Obsidian files. They're Markdown files. Obsidian just happens to be very good at reading them.**

## "Could I use Pour without Obsidian?"

Yes. Today, with caveats.

### What works right now

- **Filesystem transport**: Pour's fallback mode writes directly to disk. No Obsidian needed. Just set `base_path` to any directory, leave out `api_key` and `api_port`, and Pour writes plain Markdown files there.
- **Create mode**: Modules like `pour coffee` generate standalone `.md` files with YAML frontmatter. Any tool that reads Markdown + YAML can consume these — Hugo, Jekyll, Logseq, Dendron, a custom script, or just `cat`.
- **Append mode**: Modules like `pour me` append under a heading in an existing file. This is standard Markdown manipulation — nothing Obsidian-specific about inserting text below `### Journal`.
- **Dynamic selects from disk**: Pour scans a directory for `.md` files to populate dropdown options. This is just `readdir` — it doesn't need Obsidian.
- **Presets and cache**: Stored in `~/.pour/` (config/presets) and `~/.pour/cache/` (ephemeral state), completely independent of any note-taking app.

### What's Obsidian-flavored

A few features use Obsidian conventions that aren't universal Markdown:

| Feature | What it does | Obsidian-specific? |
|---|---|---|
| `wikilink = true` | Wraps values in `[[double brackets]]` | Yes — `[[links]]` are an Obsidian/wiki convention, not standard Markdown |
| `post_create_command` | Fires an Obsidian plugin command via REST API | Yes — requires Obsidian + REST API plugin |
| `execute_command()` | Sends commands to Obsidian's command palette | Yes — REST API only |
| YAML frontmatter | Structured metadata at top of file | No — widely supported (Hugo, Jekyll, Logseq, Zola, etc.) |
| `[[callout]]` syntax | `> [!type]` blockquotes | Obsidian-flavored, but increasingly adopted elsewhere |

The first three are opt-in. If you don't set `wikilink`, `post_create_command`, or `api_key`, none of that code runs. What you're left with is a terminal form engine that writes structured Markdown to a folder.

### What you'd pair it with instead

Without Obsidian as the "viewer," you'd want *something* to make sense of the files Pour generates:

- **Logseq** — reads the same folder-of-markdown structure, supports YAML frontmatter, understands `[[wikilinks]]`
- **Dendron** (VS Code) — same model, hierarchical note naming
- **A static site generator** (Hugo, Zola, Eleventy) — YAML frontmatter is their native data format; Pour becomes a structured content entry tool
- **Dataview-style queries** via scripts — Pour's strict frontmatter makes files trivially parseable with `yq`, `python-frontmatter`, or a 10-line script
- **Plain `ls` + `grep`** — the filenames are timestamped and descriptive; `grep -r "origin: Ethiopia" Coffee/` just works
- **A future `pour query` command** — Pour already knows the schema; a read-back mode is a natural extension

## "What would it take to make Pour truly vault-agnostic?"

Not much. The architecture is already 90% there.

### Already decoupled
- The config system (`config.toml`) has no Obsidian-specific keys except the optional API connection
- The transport layer abstracts API vs filesystem behind a single interface
- Output generation (frontmatter + body) produces standard Markdown + YAML
- The TUI, form engine, presets, visibility system, and field validation are all vault-agnostic

### The remaining 10%

1. **Wikilinks as an output format option, not an assumption**: Today `wikilink = true` emits `[[brackets]]`. A more general approach: `link_format = "wikilink" | "markdown" | "plain"` — emitting `[[x]]`, `[x](x.md)`, or bare `x` depending on the target ecosystem.

2. **`post_create_command` generalized to hooks**: Instead of firing Obsidian REST API commands specifically, expose a general `post_create_hook` that runs a shell command. Obsidian users configure it to hit the API; others run a script, a webhook, or nothing.

3. **Language in config and docs**: The word "vault" is everywhere — `vault.base_path`, `vault_path`, "vault connection status." Renaming to something neutral (`workspace`, `root`, `target`) would signal that Pour doesn't require Obsidian. This is cosmetic but meaningful for first impressions.

4. **README and onboarding**: The current README opens with "logs structured data into an Obsidian vault." A non-Obsidian user bounces immediately. Leading with "writes structured Markdown to a folder" and noting Obsidian as one (excellent) consumer would widen the aperture.

## The honest answer

Pour *is* Obsidian-centric in its story and language. It is *not* Obsidian-centric in its architecture. The filesystem transport, the YAML frontmatter, the Markdown output — these are all standard. An Obsidian user gets the best experience today (REST API speed, wikilink graph edges, plugin commands), but a non-Obsidian user can run `pour` right now and get clean, portable, structured Markdown files in any directory they choose.

The question isn't "can Pour work without Obsidian?" — it already does. The question is whether the project wants to *tell that story* or keep it as a quiet capability.
The latter is our reality.. making sure our tool is usable by as many people as it makes sense to support. But rest assured.. I won't be shutting up about Obsidian, let alone markdown.