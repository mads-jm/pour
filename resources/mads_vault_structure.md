# Vault Structure — mads_config.toml

This documents the Obsidian vault layout that `mads_config.toml` expects.
Use it as a reference when setting up a similar vault, or adapt the paths
in the config to match your own structure.

Though as any obsidian user would encourage.. adopt your own strategies for folder structure. I use tags aggressively in conjunction with large 'bucket' style folders, but get fine grained for stuff like this where deep nesting is valuable. 

## Layout

The vault follows a PARA-style numbering scheme.

```
main/                                  ← vault root (base_path)
├── 02 - Areas/
│   └── 204 - Cooking/
│       └── Coffee/
│           ├── Beans/                 ← dynamic_select source for bean field
│           │   └── YesPlz - Homestar.md
│           ├── Brewers/               ← dynamic_select sources (subfolder per category)
│           │   ├── Pour Over/
│           │   │   ├── Chemex.md
│           │   │   └── V60.md
│           │   ├── Espresso/
│           │   │   └── Flair 58.md
│           │   └── Immersion/
│           │       └── AeroPress.md
│           └── Grinders/              ← dynamic_select source for grinder field
│               ├── DF64.md
│               └── K-Ultra.md
├── 05 - Fleeting/                     ← create-mode output for `pour note`
│   └── 20260405-my-idea.md
├── 06 - Periodic/
│   └── 00 - Daily/                    ← append target for `pour me` and `pour todo`
│       ├── 20260404.md
│       └── 20260405.md
└── ...
```

## How Pour Uses Each Folder

### Daily notes — `06 - Periodic/00 - Daily/`

**Modules**: `me` (append), `todo` (append)

Pour appends under headings in existing daily notes. The notes must already
exist with the target headings (daily note plugins like Templater or
Periodic Notes handle creation).

- `pour me` appends under `## [[Journaling|Journal]]`
- `pour todo` appends under `### Tasks`

Path pattern: `%Y%m%d.md` → `20260405.md`

### Fleeting notes — `05 - Fleeting/`

**Module**: `note` (create)

Pour creates a new note per entry. No pre-existing folder structure needed
beyond the directory itself.

Path pattern: `%Y%m%d-{{title}}.md` → `20260405-my-idea.md`

### Coffee — `02 - Areas/204 - Cooking/Coffee/`

**Module**: `coffee` (create)

Pour creates a brew-log note per entry. The path interpolates the bean name
and timestamp for uniqueness.

Path pattern: `{{bean}}@{{time}}-%Y%m%d.md` → `YesPlz - Homestar@14-30-20260405.md`

#### Beans/ — dynamic_select source

Each `.md` file becomes an option in the bean dropdown. Notes are created
inline via the `bean` template when the user types a novel value.

Bean frontmatter (written by Pour's `bean` template):

```yaml
---
date: 2026-04-05
roaster: YesPlz
origin: Blend
process: Washed
roast_level: Light
bag_weight_g: 250
---
```

#### Brewers/ — dynamic_select sources (per category)

Subfolder per brew method category. The `show_when` on each brewer field
points its `source` at the matching subfolder:

| brew_method | source path               |
|-------------|---------------------------|
| Pour Over   | `Coffee/Brewers/Pour Over`  |
| Espresso    | `Coffee/Brewers/Espresso`   |
| Immersion   | `Coffee/Brewers/Immersion`  |

New brewers are created inline via the `brewer` template, which uses a
`{{category}}` field to route the note into the correct subfolder.

#### Grinders/ — dynamic_select source

Each `.md` file becomes an option in the grinder dropdown. Grinder notes
include a Dataview query that surfaces all brews using that grinder:

```markdown
## Brew Log

\```dataview
TABLE brew_method AS "Method", grind_setting AS "Grind", dose_g AS "Dose", rating AS "Rating", date AS "Date"
FROM "02 - Areas/204 - Cooking/Coffee"
WHERE grinder = link("DF64")
SORT date DESC
\```
```

## Coffee Brew Output

A completed `pour coffee` entry for a Pour Over produces a note like:

```yaml
---
date: 2026-04-05
icon: ☕
brew_method: Pour Over
brewer: "[[V60]]"
bean: "[[YesPlz - Homestar]]"
grinder: "[[DF64]]"
grind_setting: 28
dose_g: 20
yield_g: 320
time_s: 210
water_temp_c: 96
recipe:
  - stage: Bloom
    weight_g: 60
    duration_s: 30
  - stage: First Pour
    weight_g: 200
    duration_s: 45
  - stage: Second Pour
    weight_g: 320
    duration_s: 45
  - stage: Draw Down
    duration_s: 60
rating: 4
---

> [!quote]
> Bright citrus, clean finish. Lighter body than yesterday's 94°C brew.
```

An Espresso entry would instead include `machine`, `drink_type`,
`shot_style`, and `pressure_profile` fields, while omitting the pour-over
specific fields — controlled by `show_when` rules on `brew_method`.

## Recreating This Structure

1. Create the folder tree shown above in your vault
2. Add at least one `.md` file in each `dynamic_select` source folder
   so the dropdowns have content on first run
3. Copy `mads_config.toml` to `~/.config/pour/config.toml`
4. Update `base_path` to your vault's absolute path
5. Adjust the `02 - Areas/204 - Cooking/Coffee/` prefix if your vault
   uses a different folder scheme
6. Run `pour coffee` to test
