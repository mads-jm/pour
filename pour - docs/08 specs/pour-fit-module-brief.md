---
tags:
  - spec
  - brief
  - module
date created: Friday, April 4th 2026
status: draft
---

# `pour fit` — Workout Module Design Brief

## Intent

A `pour fit` module for logging workout sessions to the Obsidian vault. Reuses the category-dependent pattern established by the coffee module: a top-level split selector gates conditional fields via `show_when`, with `composite_array` for structured set logging.

## Module Shape

| Key | Value |
|-----|-------|
| Name | `fit` |
| Display name | Workout |
| Mode | `create` — one note per session |
| Path | `02 - Areas/202 - Health/00 - Fitness/%Y%m%d-{{split}}.md` |

## Fields

### Category

- **`split`** — `static_select`: Push / Pull / Legs / Upper / Lower / Cardio / Custom
  - Controls conditional fields below via `show_when`

### Conditional per Split

- **`exercises_push`**, **`exercises_pull`**, **`exercises_legs`**, etc. — `dynamic_select` per split
  - Source folders: `Health/00 - Fitness/Exercises/Push/`, `.../Pull/`, `.../Legs/`, etc.
  - `allow_create = true` with an exercise template for adding new exercises inline
  - Each folder contains `.md` note stubs (e.g., `Bench Press.md`, `Deadlift.md`)

- **Cardio-specific**: `distance_km` (number), `pace` (text), `heart_rate_avg` (number)
  - `show_when = { field = "split", equals = "Cardio" }`

### Universal

- **`sets`** — `composite_array` for set logging
  - Sub-fields: `exercise` (text), `weight_kg` (number), `reps` (number), `rpe` (number)
  - Appears for all strength splits (Push/Pull/Legs/Upper/Lower)
  - Could use `show_when = { field = "split", one_of = ["Push", "Pull", "Legs", "Upper", "Lower"] }`

- **`duration_min`** — `number`: Total session duration
- **`bodyweight_kg`** — `number`: Optional daily weigh-in
- **`notes`** — `textarea` → body: How the session felt, energy level, injuries

### Frontmatter Targets

All fields target frontmatter except `notes` (body) and `sets` (frontmatter as array of objects).

## Vault Structure Needed

```
02 - Areas/202 - Health/00 - Fitness/
├── Exercises/
│   ├── Push/
│   │   ├── Bench Press.md
│   │   ├── Overhead Press.md
│   │   └── ...
│   ├── Pull/
│   │   ├── Deadlift.md
│   │   ├── Barbell Row.md
│   │   └── ...
│   ├── Legs/
│   │   ├── Squat.md
│   │   ├── Romanian Deadlift.md
│   │   └── ...
│   └── Cardio/
│       ├── Running.md
│       └── Cycling.md
├── Exercises.md          (existing — index note)
└── Push Pull Legs.md     (existing — reference note)
```

## Template: Exercise

```toml
[templates.exercise]
path = "02 - Areas/202 - Health/00 - Fitness/Exercises/{{category}}/{{name}}.md"

[[templates.exercise.fields]]
name = "category"
field_type = "static_select"
prompt = "Category"
options = ["Push", "Pull", "Legs", "Cardio"]

[[templates.exercise.fields]]
name = "muscle_group"
field_type = "text"
prompt = "Primary muscle group"
```

## Open Questions

1. **Per-exercise vs per-session tracking**: The `sets` composite_array logs sets inline. An alternative is a separate `pour set` append module that adds rows to the session note — more granular but more friction per set.

2. **Exercise selection within the form**: Currently `composite_array` sub-fields can't be `dynamic_select`. The exercise field inside `sets` would be plain text unless we extend sub-field types. Alternatively, a top-level `exercise` dynamic_select could pre-fill.

3. **Progressive overload tracking**: Showing previous session's numbers for the same exercise requires reading from vault history — not currently supported. Could be a future `history_hint` feature on fields.

4. **Rest timer**: Out of scope for Pour (a capture tool, not a training timer). Could note rest periods as a composite sub-field if desired.

5. **Upper/Lower vs PPL**: The split options include both PPL and Upper/Lower to support different program structures. The exercise folders might need to be organized differently (some exercises span categories). Consider whether `Custom` split should show all exercises.

6. **Warm-up sets**: Should warm-up sets be distinguished from working sets? Could add a `set_type` sub-field (`Warm-up` / `Working` / `Drop`).

## References

- Existing vault: `02 - Areas/202 - Health/00 - Fitness/` has `Exercises.md`, `Push Pull Legs.md`
- Coffee module pattern: `resources/mads_config.toml` `[modules.coffee]` — category → equipment → method-specific → universal
- `show_when` spec: `pour - docs/02 references/field-types.md`
- `composite_array` spec: same reference doc
