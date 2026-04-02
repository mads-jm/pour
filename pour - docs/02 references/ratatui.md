---
tags:
  - reference
  - rust
  - tui
aliases:
  - ratatui
date created: Tuesday, March 31st 2026, 12:14:35 am
date modified: Thursday, April 2nd 2026, 9:18:45 am
---

# Ratatui - TUI Framework Reference

> __Source:__ <https://docs.rs/ratatui/latest/ratatui/>
> __Crate:__ `ratatui` (with [[crossterm]] backend)

## Architecture

Ratatui uses an __immediate rendering model__ -- the entire frame is redrawn each cycle. No widget state is retained between frames.

### Application Lifecycle

```rust
fn main() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            // render widgets here
            frame.render_widget(my_widget, frame.area());
        })?;

        // handle events
        if let Event::Key(key) = crossterm::event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                _ => {}
            }
        }
    }
    Ok(())
}
```

## Core Types

| Type | Module | Purpose |
|------|--------|---------|
| `Terminal` | `ratatui` | Manages drawing operations |
| `Frame` | `ratatui` | Drawable area passed to render closures |
| `Rect` | `ratatui::layout` | Rectangular area (x, y, width, height) |
| `Layout` | `ratatui::layout` | Splits areas into sub-regions |
| `Constraint` | `ratatui::layout` | Sizing rules for layout |
| `Widget` | `ratatui::widgets` | Trait for renderable components |

## Layout System

```rust
use ratatui::layout::{Layout, Constraint, Direction};

// Vertical stack
let chunks = Layout::vertical([
    Constraint::Length(3),    // fixed 3 rows
    Constraint::Min(0),       // fill remaining
    Constraint::Length(1),    // fixed 1 row
]).split(frame.area());

// Horizontal split
let cols = Layout::horizontal([
    Constraint::Percentage(50),
    Constraint::Percentage(50),
]).split(chunks[1]);
```

__Constraint variants:__ `Length(u16)`, `Min(u16)`, `Max(u16)`, `Percentage(u16)`, `Fill(u16)`, `Ratio(u32, u32)`

## Key Widgets

### Block (borders/titles)

```rust
let block = Block::default()
    .title("My Block")
    .borders(Borders::ALL)
    .border_style(Style::default().fg(Color::Cyan));
```

### Paragraph (text display)

```rust
let text = Paragraph::new("Hello world")
    .block(Block::default().borders(Borders::ALL))
    .style(Style::default().fg(Color::White))
    .wrap(Wrap { trim: true });
```

### List (selectable items)

```rust
let items = vec![
    ListItem::new("Item 1"),
    ListItem::new("Item 2"),
    ListItem::new("Item 3"),
];
let list = List::new(items)
    .block(Block::default().title("Menu").borders(Borders::ALL))
    .highlight_style(Style::default().add_modifier(Modifier::BOLD))
    .highlight_symbol(">> ");

// Requires StatefulWidget rendering:
frame.render_stateful_widget(list, area, &mut list_state);
```

### Table

```rust
let rows = vec![
    Row::new(vec!["Cell1", "Cell2"]),
    Row::new(vec!["Cell3", "Cell4"]),
];
let table = Table::new(rows, [Constraint::Length(10), Constraint::Length(10)])
    .header(Row::new(vec!["Col1", "Col2"]).style(Style::default().bold()))
    .block(Block::default().borders(Borders::ALL));
```

### Tabs

```rust
let tabs = Tabs::new(vec!["Tab1", "Tab2", "Tab3"])
    .select(current_tab)
    .highlight_style(Style::default().fg(Color::Yellow));
```

## Text & Styling

Text hierarchy: `Text` > `Line` > `Span`

```rust
use ratatui::text::{Text, Line, Span};
use ratatui::style::{Style, Color, Modifier, Stylize};

// Fluent API via Stylize trait
let span = "bold red".red().bold();

// Composed line
let line = Line::from(vec![
    Span::raw("Normal "),
    Span::styled("highlighted", Style::default().fg(Color::Yellow).bold()),
]);

// Multi-line text
let text = Text::from(vec![
    Line::from("Line 1"),
    Line::from("Line 2"),
]);
```

## Stateful Widgets

Some widgets (List, Table) have associated state types:

```rust
let mut list_state = ListState::default();
list_state.select(Some(0)); // select first item

// In render:
frame.render_stateful_widget(list, area, &mut list_state);

// Navigation:
list_state.select_next();
list_state.select_previous();
```

## Form-Building Patterns

Ratatui doesn't have built-in form widgets. Common patterns:

1. __App struct__ holds form state (current field, field values, cursor positions)
2. __Event handler__ routes input to the active field
3. __Render function__ draws each field with appropriate styling (active vs inactive)
4. Third-party crates: `tui-textarea`, `tui-input` for text input widgets







