---
tags:
  - reference
  - rust
  - terminal
aliases:
  - crossterm
date created: Tuesday, March 31st 2026, 12:14:38 am
date modified: Thursday, April 2nd 2026, 8:17:06 am
---

# Crossterm - Terminal Manipulation Reference

> __Source:__ <https://docs.rs/crossterm/latest/crossterm/>
> __Crate:__ `crossterm`

## Role in Pour

Crossterm is the backend for [[ratatui]]. It handles raw terminal I/O, event capture, and screen management.

## Terminal Setup/Teardown

```rust
use crossterm::{
    execute,
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

// Setup
enable_raw_mode()?;
execute!(std::io::stdout(), EnterAlternateScreen)?;

// Teardown
disable_raw_mode()?;
execute!(std::io::stdout(), LeaveAlternateScreen)?;
```

Note: `ratatui::init()` and `ratatui::restore()` handle this automatically.

## Event Handling

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

// Blocking read
if let Event::Key(key) = event::read()? {
    match key.code {
        KeyCode::Char('q') => { /* quit */ }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => { /* ctrl+c */ }
        KeyCode::Enter => { /* submit */ }
        KeyCode::Tab => { /* next field */ }
        KeyCode::BackTab => { /* prev field */ }
        KeyCode::Up => { /* navigate up */ }
        KeyCode::Down => { /* navigate down */ }
        KeyCode::Backspace => { /* delete char */ }
        KeyCode::Char(c) => { /* type character */ }
        KeyCode::Esc => { /* cancel */ }
        _ => {}
    }
}

// Non-blocking poll
if event::poll(std::time::Duration::from_millis(100))? {
    if let Event::Key(key) = event::read()? {
        // handle key
    }
}
```

## Key Types

| Type | Description |
|------|-------------|
| `Event` | Top-level event enum: `Key`, `Mouse`, `Resize`, `Paste`, `FocusGained/Lost` |
| `KeyEvent` | Contains `code: KeyCode`, `modifiers: KeyModifiers`, `kind: KeyEventKind` |
| `KeyCode` | Enum: `Char(char)`, `Enter`, `Esc`, `Tab`, `BackTab`, `Backspace`, `Up/Down/Left/Right`, `F(u8)`, etc. |
| `KeyModifiers` | Bitflags: `SHIFT`, `CONTROL`, `ALT`, `SUPER`, `NONE` |
| `KeyEventKind` | `Press`, `Repeat`, `Release` |

## Execution Macros

```rust
// Immediate flush (for single commands)
execute!(stdout, command1, command2)?;

// Queued (for batch rendering -- ratatui uses this internally)
queue!(stdout, command1, command2)?;
stdout.flush()?;
```
