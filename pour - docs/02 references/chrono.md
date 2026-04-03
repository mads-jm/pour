---
tags:
  - reference
  - rust
  - datetime
aliases:
  - chrono
date created: Tuesday, March 31st 2026, 12:14:43 am
date modified: Friday, April 3rd 2026, 4:11:45 am
---

# Chrono - Date/Time Reference

> __Source:__ <https://docs.rs/chrono/latest/chrono/>
> __Crate:__ `chrono`

## Role in Pour

Used for generating timestamps in file names (e.g., `2026-03-30_1205_V60.md`) and daily note paths (e.g., `2026-03-30.md`).

## Getting Current Time

```rust
use chrono::Local;

let now = Local::now();
```

## Formatting with Strftime

```rust
// Daily note path: "2026-03-30.md"
let date_str = now.format("%Y-%m-%d").to_string();

// Coffee log filename: "2026-03-30_1205_Brew.md"
let timestamp = now.format("%Y-%m-%d_%H%M").to_string();

// Full timestamp for entries: "14:32"
let time_str = now.format("%H:%M").to_string();
```

__Common format specifiers:__

| Specifier | Meaning | Example |
|-----------|---------|---------|
| `%Y` | 4-digit year | 2026 |
| `%m` | Month (01-12) | 03 |
| `%d` | Day (01-31) | 30 |
| `%H` | Hour 24h (00-23) | 14 |
| `%M` | Minute (00-59) | 05 |
| `%S` | Second (00-59) | 32 |
| `%A` | Weekday name | Monday |
| `%B` | Month name | March |

## Parsing Dates

```rust
use chrono::NaiveDate;

let date = NaiveDate::parse_from_str("2026-03-30", "%Y-%m-%d")?;
```

## Key Types

| Type | Use |
|------|-----|
| `DateTime<Local>` | Current local date+time |
| `NaiveDate` | Date without timezone |
| `NaiveDateTime` | Date+time without timezone |
| `Local` | System timezone |
