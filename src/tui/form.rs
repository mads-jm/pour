use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::{App, FormState};
use crate::config::{FieldConfig, FieldType, SubFieldType};

/// Render the form view for the currently selected module.
pub fn render(app: &App, frame: &mut Frame) {
    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k,
        None => return,
    };
    let module = match app.config.modules.get(module_key) {
        Some(m) => m,
        None => return,
    };
    let form_state = match &app.form_state {
        Some(fs) => fs,
        None => return,
    };

    let area = frame.area();

    // Layout: title bar, field list, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(1),    // fields
            Constraint::Length(3), // footer / validation
        ])
        .split(area);

    // Title: "pour <key> — Display Name" to reinforce the CLI command
    let display_name = module
        .display_name
        .as_deref()
        .unwrap_or(module_key.as_str());
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" ▽ pour {module_key}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" — {display_name} "),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Fields
    render_fields(frame, chunks[1], &module.fields, form_state);

    // Footer: validation errors or key hints
    let footer_content = if !form_state.validation_errors.is_empty() {
        let error_text = form_state.validation_errors.join("; ");
        Line::from(Span::styled(
            format!(" Error: {error_text}"),
            Style::default().fg(Color::Red),
        ))
    } else {
        Line::from(vec![
            Span::styled(" ↑↓", Style::default().fg(Color::Yellow)),
            Span::raw("/"),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" interact  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" clear/back"),
        ])
    };
    let footer = Paragraph::new(footer_content).block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);
}

/// Render the vertical list of form fields plus a submit button row.
fn render_fields(frame: &mut Frame, area: Rect, fields: &[FieldConfig], form_state: &FormState) {
    let submit_active = form_state.active_field == fields.len();

    let mut items: Vec<ListItem> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let is_active = i == form_state.active_field;
            let value = form_state
                .field_values
                .get(&field.name)
                .map(|s| s.as_str())
                .unwrap_or("");

            let prompt_style = if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let value_display = match &field.field_type {
                FieldType::StaticSelect | FieldType::DynamicSelect => {
                    let label = if value.is_empty() {
                        "<select>".to_string()
                    } else {
                        value.to_string()
                    };
                    // Show open/closed chevron when the field is active
                    if is_active {
                        if form_state.dropdown_open {
                            format!("{label} [^]")
                        } else {
                            format!("{label} [v]")
                        }
                    } else {
                        label
                    }
                }
                FieldType::Textarea => {
                    let label = if value.is_empty() {
                        "<enter text>".to_string()
                    } else {
                        let line_count = value.lines().count();
                        let first_line = value.lines().next().unwrap_or("");
                        if line_count > 1 {
                            format!("{first_line} [{line_count} lines]")
                        } else {
                            first_line.to_string()
                        }
                    };
                    if is_active {
                        if form_state.textarea_open {
                            format!("{label} [^]")
                        } else {
                            format!("{label} [v]")
                        }
                    } else {
                        label
                    }
                }
                FieldType::CompositeArray => {
                    let rows = form_state
                        .composite_values
                        .get(&field.name)
                        .map(|r| r.len())
                        .unwrap_or(0);
                    let label = if rows == 0 {
                        "add rows".to_string()
                    } else {
                        format!("{rows} row{}", if rows == 1 { "" } else { "s" })
                    };
                    if is_active {
                        if form_state.composite_open {
                            format!("{label} [^]")
                        } else {
                            format!("{label} [v]")
                        }
                    } else {
                        label
                    }
                }
                _ => {
                    if value.is_empty() {
                        if is_active {
                            " ".to_string() // space so the cursor has something to land on
                        } else {
                            "<empty>".to_string()
                        }
                    } else {
                        value.to_string()
                    }
                }
            };

            let required_marker = if field.required.unwrap_or(false) {
                "*"
            } else {
                " "
            };

            let indicator = if is_active { "▸" } else { " " };

            let line = Line::from(vec![
                Span::styled(format!("{indicator} "), prompt_style),
                Span::styled(
                    format!("{}{}: ", field.prompt, required_marker),
                    prompt_style,
                ),
                Span::styled(
                    value_display,
                    if is_active {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    // Submit button row
    let submit_style = if submit_active {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let submit_indicator = if submit_active { "▸" } else { " " };
    items.push(ListItem::new(Line::from(vec![Span::raw("")])));
    items.push(ListItem::new(Line::from(vec![Span::styled(
        format!("{submit_indicator} [ pour ]"),
        submit_style,
    )])));

    let item_count = items.len();
    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);
    super::render_overflow_hints(frame, area, item_count, 0);

    // Place the terminal block cursor for text/textarea/number fields
    if !submit_active && let Some(field) = fields.get(form_state.active_field) {
        let is_text_input = matches!(field.field_type, FieldType::Text | FieldType::Number);
        if is_text_input {
            // prefix: "> " (2) + prompt + required_marker (1) + ": " (2)
            let prefix_len = 2 + field.prompt.len() + 1 + 2;
            let cursor_x = area.x + prefix_len as u16 + form_state.cursor_position as u16;
            let cursor_y = area.y + form_state.active_field as u16;
            if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
                frame.set_cursor_position(Position::new(cursor_x, cursor_y));
            }
        }
    }

    // If active field is a select type AND the dropdown is open, render the options popup below
    if form_state.dropdown_open
        && let Some(field) = fields.get(form_state.active_field)
        && matches!(
            field.field_type,
            FieldType::StaticSelect | FieldType::DynamicSelect
        )
    {
        render_select_options(frame, area, field, form_state);
    }

    // If active field is a textarea AND the editor is open, render the text editor overlay
    if form_state.textarea_open
        && let Some(field) = fields.get(form_state.active_field)
        && field.field_type == FieldType::Textarea
    {
        render_textarea_editor(frame, area, field, form_state);
    }

    // If active field is a composite_array AND the overlay is open, render the table editor
    if form_state.composite_open
        && let Some(field) = fields.get(form_state.active_field)
        && field.field_type == FieldType::CompositeArray
    {
        render_composite_editor(frame, area, field, form_state);
    }
}

/// Render a scrollable options list for select fields.
fn render_select_options(
    frame: &mut Frame,
    area: Rect,
    field: &FieldConfig,
    form_state: &FormState,
) {
    let options = match form_state.field_options.get(&field.name) {
        Some(opts) if !opts.is_empty() => opts,
        _ => return,
    };

    let current_value = form_state
        .field_values
        .get(&field.name)
        .map(|s| s.as_str())
        .unwrap_or("");

    // Position the options list below the active field row
    let y_offset = (form_state.active_field as u16).min(area.height.saturating_sub(1));
    let options_area = Rect {
        x: area.x + 4,
        y: area.y + y_offset + 1,
        width: area.width.saturating_sub(8).min(40),
        height: (options.len() as u16 + 2).min(area.height.saturating_sub(y_offset + 1)),
    };

    if options_area.height < 3 {
        return;
    }

    let items: Vec<ListItem> = options
        .iter()
        .map(|opt| {
            let is_selected = opt == current_value;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let marker = if is_selected { "▸ " } else { "  " };
            ListItem::new(Line::from(Span::styled(format!("{marker}{opt}"), style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Options ")
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(Clear, options_area);
    frame.render_widget(list, options_area);
    // Inner area excludes borders
    let inner = Rect {
        x: options_area.x + 1,
        y: options_area.y + 1,
        width: options_area.width.saturating_sub(2),
        height: options_area.height.saturating_sub(2),
    };
    let selected_idx = options.iter().position(|o| o == current_value).unwrap_or(0);
    let scroll = selected_idx.saturating_sub(inner.height as usize - 1);
    super::render_overflow_hints(frame, inner, options.len(), scroll);
}

/// Render a bordered text editor overlay for textarea fields.
fn render_textarea_editor(
    frame: &mut Frame,
    area: Rect,
    field: &FieldConfig,
    form_state: &FormState,
) {
    let value = form_state
        .field_values
        .get(&field.name)
        .map(|s| s.as_str())
        .unwrap_or("");

    // Position below the active field row, fill available space
    let y_offset = (form_state.active_field as u16 + 1).min(area.height.saturating_sub(1));
    let editor_area = Rect {
        x: area.x + 4,
        y: area.y + y_offset,
        width: area.width.saturating_sub(8).min(60),
        height: area.height.saturating_sub(y_offset).clamp(4, 10),
    };

    if editor_area.height < 3 {
        return;
    }

    // Find the line and column from the flat cursor_position
    let mut remaining = form_state.cursor_position;
    let mut cursor_line: u16 = 0;
    let mut cursor_col: usize = 0;
    for line in value.split('\n') {
        if remaining <= line.len() {
            cursor_col = remaining;
            break;
        }
        remaining -= line.len() + 1; // +1 for the newline
        cursor_line += 1;
    }

    // Horizontal scroll: inner editor width minus borders
    let avail = editor_area.width.saturating_sub(2) as usize;
    let scroll = form_state.textarea_scroll_offset;

    // Render all lines with the same horizontal scroll offset applied
    let raw_lines: Vec<&str> = if value.is_empty() {
        vec![""]
    } else {
        value.split('\n').collect()
    };

    let lines: Vec<Line> = raw_lines
        .iter()
        .map(|l| {
            let char_count = l.chars().count();
            let left_clipped = scroll > 0 && char_count > 0;
            let right_clipped = char_count > scroll + avail;
            let content_take = avail.saturating_sub(left_clipped as usize + right_clipped as usize);
            let slice: String = l.chars().skip(scroll).take(content_take).collect();

            let mut spans: Vec<Span> = Vec::new();
            if left_clipped {
                spans.push(Span::styled("◂", Style::default().fg(Color::DarkGray)));
            }
            spans.push(Span::raw(slice));
            if right_clipped {
                spans.push(Span::styled("▸", Style::default().fg(Color::DarkGray)));
            }
            Line::from(spans)
        })
        .collect();

    let editor = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", field.prompt))
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(Clear, editor_area);
    frame.render_widget(editor, editor_area);

    // Place cursor: +1 for border, cursor_col adjusted by scroll, +1 if left indicator shown
    let left_indicator: u16 = if scroll > 0 { 1 } else { 0 };
    let viewport_col = cursor_col.saturating_sub(scroll) as u16;
    let cx = editor_area.x + 1 + left_indicator + viewport_col;
    let cy = editor_area.y + 1 + cursor_line;
    if cx < editor_area.x + editor_area.width - 1 && cy < editor_area.y + editor_area.height - 1 {
        frame.set_cursor_position(Position::new(cx, cy));
    }
}

/// Render a bordered table editor overlay for composite_array fields.
fn render_composite_editor(
    frame: &mut Frame,
    area: Rect,
    field: &FieldConfig,
    form_state: &FormState,
) {
    let sub_fields = match &field.sub_fields {
        Some(subs) if !subs.is_empty() => subs,
        _ => return,
    };

    let rows = form_state
        .composite_values
        .get(&field.name)
        .cloned()
        .unwrap_or_default();

    // Position below the active field row, fill available space
    let y_offset = (form_state.active_field as u16 + 1).min(area.height.saturating_sub(1));
    let editor_area = Rect {
        x: area.x + 2,
        y: area.y + y_offset,
        width: area.width.saturating_sub(4).min(70),
        height: area.height.saturating_sub(y_offset).clamp(5, 14),
    };

    if editor_area.height < 4 {
        return;
    }

    // Build lines: header row, then data rows
    let col_count = sub_fields.len();

    // Calculate column widths: max of header and cell widths, with minimum 6
    let mut widths: Vec<usize> = sub_fields.iter().map(|s| s.prompt.len().max(6)).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len().max(1));
            }
        }
    }

    // Clamp total width to fit editor area (inner width = editor_area.width - 2 for borders)
    let inner_width = editor_area.width.saturating_sub(2) as usize;
    let total: usize = widths.iter().sum::<usize>() + (col_count * 3) + 1; // " | " separators
    if total > inner_width && inner_width > col_count * 4 {
        let scale = inner_width as f64 / total as f64;
        for w in &mut widths {
            *w = (*w as f64 * scale).max(3.0) as usize;
        }
    }

    let mut lines: Vec<Line> = Vec::new();

    // Header line
    let mut header_spans = Vec::new();
    for (i, sub) in sub_fields.iter().enumerate() {
        let w = widths.get(i).copied().unwrap_or(6);
        if i > 0 {
            header_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }
        header_spans.push(Span::styled(
            format!("{:width$}", sub.prompt, width = w),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(Line::from(header_spans));

    // Separator line
    let sep: String = widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let dashes = "─".repeat(*w);
            if i > 0 {
                format!("─┼─{dashes}")
            } else {
                dashes
            }
        })
        .collect();
    lines.push(Line::from(Span::styled(
        sep,
        Style::default().fg(Color::DarkGray),
    )));

    // Data rows
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            " (empty — press Enter to add a row)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (row_idx, row) in rows.iter().enumerate() {
            let is_active_row = row_idx == form_state.composite_row;
            let mut row_spans = Vec::new();

            for (col_idx, _sub) in sub_fields.iter().enumerate() {
                let w = widths.get(col_idx).copied().unwrap_or(6);
                let cell = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                let is_active_cell = is_active_row && col_idx == form_state.composite_col;

                if col_idx > 0 {
                    row_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
                }

                let style = if is_active_cell {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::DarkGray)
                } else if is_active_row {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                };

                let display = if cell.is_empty() && is_active_cell {
                    "_".to_string()
                } else {
                    format!("{:width$}", cell, width = w)
                };

                row_spans.push(Span::styled(display, style));
            }

            // Row indicator
            let indicator = if is_active_row { "▸" } else { " " };
            let mut full_spans = vec![Span::styled(
                format!("{indicator} "),
                if is_active_row {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            )];
            full_spans.extend(row_spans);
            lines.push(Line::from(full_spans));
        }
    }

    // Hint line
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" Tab", Style::default().fg(Color::Yellow)),
        Span::raw(" next  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" add row  "),
        Span::styled("Del", Style::default().fg(Color::Yellow)),
        Span::raw(" remove  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" close"),
    ]));

    let editor = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", field.prompt))
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(Clear, editor_area);
    frame.render_widget(editor, editor_area);
}

/// Handle a key event while in Form view.
///
/// Returns a `FormAction` signalling what the wiring layer should do next.
pub fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) -> FormAction {
    use crossterm::event::KeyCode;

    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k.clone(),
        None => return FormAction::None,
    };
    let module = match app.config.modules.get(&module_key) {
        Some(m) => m,
        None => return FormAction::None,
    };
    let real_field_count = module.fields.len();
    // +1 for the virtual submit button at the end
    let navigable_count = real_field_count + 1;

    let form_state = match &mut app.form_state {
        Some(fs) => fs,
        None => return FormAction::None,
    };

    let on_submit_button = form_state.active_field == real_field_count;
    let active_field = module.fields.get(form_state.active_field);
    let is_select = active_field
        .map(|f| {
            matches!(
                f.field_type,
                FieldType::StaticSelect | FieldType::DynamicSelect
            )
        })
        .unwrap_or(false);
    let is_textarea = active_field
        .map(|f| f.field_type == FieldType::Textarea)
        .unwrap_or(false);
    let is_composite = active_field
        .map(|f| f.field_type == FieldType::CompositeArray)
        .unwrap_or(false);

    // Composite overlay has its own key handling
    if is_composite && form_state.composite_open {
        return handle_composite_key(form_state, active_field.unwrap(), key);
    }

    match key.code {
        // Esc (layered):
        //   1. overlay open (dropdown/textarea) → close it
        //   2. current field has content → clear it
        //   3. field already empty → cancel form (back to dashboard)
        KeyCode::Esc => {
            if form_state.dropdown_open {
                form_state.dropdown_open = false;
                FormAction::None
            } else if form_state.textarea_open {
                form_state.textarea_open = false;
                form_state.textarea_scroll_offset = 0;
                FormAction::None
            } else if form_state.composite_open {
                form_state.composite_open = false;
                FormAction::None
            } else if let Some(field) = active_field {
                let value = form_state
                    .field_values
                    .entry(field.name.clone())
                    .or_default();
                if !value.is_empty() {
                    value.clear();
                    form_state.cursor_position = 0;
                    FormAction::None
                } else {
                    FormAction::Cancel
                }
            } else {
                FormAction::Cancel
            }
        }

        // Tab: always move forward one field, close overlays
        KeyCode::Tab => {
            form_state.dropdown_open = false;
            form_state.textarea_open = false;
            form_state.textarea_scroll_offset = 0;
            form_state.composite_open = false;
            form_state.active_field = (form_state.active_field + 1) % navigable_count;
            form_state.cursor_position = current_value_len(form_state, module);
            FormAction::None
        }

        // Shift+Tab: always move backward one field, close overlays
        KeyCode::BackTab => {
            form_state.dropdown_open = false;
            form_state.textarea_open = false;
            form_state.textarea_scroll_offset = 0;
            form_state.composite_open = false;
            form_state.active_field = if form_state.active_field == 0 {
                navigable_count - 1
            } else {
                form_state.active_field - 1
            };
            form_state.cursor_position = current_value_len(form_state, module);
            FormAction::None
        }

        // Up: cycle options when dropdown is open; navigate to previous field otherwise
        KeyCode::Up => {
            if is_select && form_state.dropdown_open {
                if let Some(field) = active_field {
                    cycle_select(form_state, &field.name, -1);
                }
            } else if is_textarea && form_state.textarea_open {
                // Move cursor up one line inside the editor
                if let Some(field) = active_field {
                    let value = form_state
                        .field_values
                        .get(&field.name)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    form_state.cursor_position =
                        move_cursor_vertically(value, form_state.cursor_position, -1);
                }
            } else {
                form_state.dropdown_open = false;
                form_state.textarea_open = false;
                form_state.textarea_scroll_offset = 0;
                form_state.composite_open = false;
                form_state.active_field = if form_state.active_field == 0 {
                    navigable_count - 1
                } else {
                    form_state.active_field - 1
                };
                form_state.cursor_position = current_value_len(form_state, module);
            }
            FormAction::None
        }

        // Down: cycle options when dropdown is open; navigate to next field otherwise
        KeyCode::Down => {
            if is_select && form_state.dropdown_open {
                if let Some(field) = active_field {
                    cycle_select(form_state, &field.name, 1);
                }
            } else if is_textarea && form_state.textarea_open {
                // Move cursor down one line inside the editor
                if let Some(field) = active_field {
                    let value = form_state
                        .field_values
                        .get(&field.name)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    form_state.cursor_position =
                        move_cursor_vertically(value, form_state.cursor_position, 1);
                }
            } else {
                form_state.dropdown_open = false;
                form_state.textarea_open = false;
                form_state.textarea_scroll_offset = 0;
                form_state.composite_open = false;
                form_state.active_field = (form_state.active_field + 1) % navigable_count;
                form_state.cursor_position = current_value_len(form_state, module);
            }
            FormAction::None
        }

        // Enter:
        //   - submit button: submit the form
        //   - select field: toggle dropdown open/closed
        //   - textarea closed: toggle editor open
        //   - textarea open: insert a newline at cursor
        //   - text/number: advance to next field
        KeyCode::Enter => {
            if on_submit_button {
                FormAction::Submit
            } else if is_select {
                form_state.dropdown_open = !form_state.dropdown_open;
                FormAction::None
            } else if is_composite {
                form_state.composite_open = true;
                form_state.composite_row = 0;
                form_state.composite_col = 0;
                FormAction::None
            } else if is_textarea {
                if form_state.textarea_open {
                    // Insert newline inside the editor
                    if let Some(field) = active_field {
                        let value = form_state
                            .field_values
                            .entry(field.name.clone())
                            .or_default();
                        let pos = form_state.cursor_position.min(value.len());
                        value.insert(pos, '\n');
                        form_state.cursor_position = pos + 1;
                        // After a newline, cursor_col resets to 0 on the new line
                        form_state.textarea_scroll_offset = 0;
                    }
                } else {
                    // Open the editor overlay
                    form_state.textarea_open = true;
                    form_state.cursor_position = current_value_len(form_state, module);
                }
                FormAction::None
            } else {
                // text / number fields: advance to next field (like Tab)
                form_state.active_field = (form_state.active_field + 1) % navigable_count;
                form_state.cursor_position = current_value_len(form_state, module);
                FormAction::None
            }
        }

        KeyCode::Char(c) => {
            if on_submit_button
                || is_select
                || is_composite
                || (is_textarea && !form_state.textarea_open)
            {
                return FormAction::None;
            }
            if let Some(field) = active_field {
                // For number fields, only allow digits, decimal point, and leading minus
                if field.field_type == FieldType::Number
                    && !c.is_ascii_digit()
                    && c != '.'
                    && c != '-'
                {
                    return FormAction::None;
                }

                let value = form_state
                    .field_values
                    .entry(field.name.clone())
                    .or_default();
                let pos = form_state.cursor_position.min(value.len());
                value.insert(pos, c);
                form_state.cursor_position = pos + 1;

                if is_textarea && form_state.textarea_open {
                    let value_snap = form_state
                        .field_values
                        .get(&field.name)
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
                    let avail = term_cols.saturating_sub(8).min(60).saturating_sub(2);
                    sync_textarea_scroll(form_state, &value_snap, avail);
                }
            }
            FormAction::None
        }

        KeyCode::Backspace => {
            if on_submit_button
                || is_select
                || is_composite
                || (is_textarea && !form_state.textarea_open)
            {
                return FormAction::None;
            }
            if let Some(field) = active_field {
                let value = form_state
                    .field_values
                    .entry(field.name.clone())
                    .or_default();
                if form_state.cursor_position > 0 && !value.is_empty() {
                    let pos = form_state.cursor_position.min(value.len());
                    value.remove(pos - 1);
                    form_state.cursor_position = pos - 1;
                }

                if is_textarea && form_state.textarea_open {
                    let value_snap = form_state
                        .field_values
                        .get(&field.name)
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
                    let avail = term_cols.saturating_sub(8).min(60).saturating_sub(2);
                    sync_textarea_scroll(form_state, &value_snap, avail);
                }
            }
            FormAction::None
        }

        KeyCode::Left => {
            if form_state.cursor_position > 0 {
                form_state.cursor_position -= 1;
            }
            if is_textarea && form_state.textarea_open {
                if let Some(field) = active_field {
                    let value_snap = form_state
                        .field_values
                        .get(&field.name)
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
                    let avail = term_cols.saturating_sub(8).min(60).saturating_sub(2);
                    sync_textarea_scroll(form_state, &value_snap, avail);
                }
            }
            FormAction::None
        }

        KeyCode::Right => {
            if let Some(field) = active_field {
                let len = form_state
                    .field_values
                    .get(&field.name)
                    .map(|v| v.len())
                    .unwrap_or(0);
                if form_state.cursor_position < len {
                    form_state.cursor_position += 1;
                }
                if is_textarea && form_state.textarea_open {
                    let value_snap = form_state
                        .field_values
                        .get(&field.name)
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
                    let avail = term_cols.saturating_sub(8).min(60).saturating_sub(2);
                    sync_textarea_scroll(form_state, &value_snap, avail);
                }
            }
            FormAction::None
        }

        _ => FormAction::None,
    }
}

/// Actions that the form handler can signal to the wiring layer.
#[derive(Debug, PartialEq, Eq)]
pub enum FormAction {
    None,
    Submit,
    Cancel,
}

/// Cycle the selected value in a select field by the given delta (-1 or +1).
fn cycle_select(form_state: &mut FormState, field_name: &str, delta: i32) {
    let options = match form_state.field_options.get(field_name) {
        Some(opts) if !opts.is_empty() => opts,
        _ => return,
    };

    let current = form_state
        .field_values
        .get(field_name)
        .cloned()
        .unwrap_or_default();

    let current_idx = options.iter().position(|o| *o == current);
    let new_idx = match current_idx {
        Some(idx) => {
            let len = options.len() as i32;
            ((idx as i32 + delta).rem_euclid(len)) as usize
        }
        None => 0,
    };

    if let Some(new_value) = options.get(new_idx) {
        form_state
            .field_values
            .insert(field_name.to_string(), new_value.clone());
    }
}

/// Get the length of the current field's value for cursor positioning.
fn current_value_len(form_state: &FormState, module: &crate::config::ModuleConfig) -> usize {
    module
        .fields
        .get(form_state.active_field)
        .and_then(|f| form_state.field_values.get(&f.name))
        .map(|v| v.len())
        .unwrap_or(0)
}

const TEXTAREA_SCROLL_MARGIN: usize = 2;

/// Recompute `textarea_scroll_offset` so the cursor column stays in the
/// horizontal viewport of the textarea editor.
///
/// `value` is the full textarea string; `cursor_pos` is the flat byte offset.
/// `avail_width` is the inner editor width (editor_area.width - 2 borders).
fn sync_textarea_scroll(form_state: &mut FormState, value: &str, avail_width: u16) {
    if avail_width == 0 {
        return;
    }
    let avail = avail_width as usize;

    // Compute cursor_col on the active line
    let mut remaining = form_state.cursor_position;
    let mut cursor_col: usize = 0;
    for line in value.split('\n') {
        if remaining <= line.len() {
            cursor_col = remaining;
            break;
        }
        remaining -= line.len() + 1;
    }

    let scroll = form_state.textarea_scroll_offset;

    // Scroll right: cursor near/past right edge
    let right_edge = scroll + avail.saturating_sub(TEXTAREA_SCROLL_MARGIN + 1);
    if cursor_col >= right_edge {
        form_state.textarea_scroll_offset =
            cursor_col.saturating_sub(avail.saturating_sub(TEXTAREA_SCROLL_MARGIN + 1));
    }

    // Scroll left: cursor near/before left edge
    if cursor_col < scroll + TEXTAREA_SCROLL_MARGIN && scroll > 0 {
        form_state.textarea_scroll_offset = cursor_col.saturating_sub(TEXTAREA_SCROLL_MARGIN);
    }

    if form_state.textarea_scroll_offset > cursor_col {
        form_state.textarea_scroll_offset = 0;
    }
}

/// Handle key events inside the composite array overlay.
///
/// Uses local row/col indices to avoid split-borrow issues with `FormState`.
fn handle_composite_key(
    form_state: &mut FormState,
    field: &FieldConfig,
    key: crossterm::event::KeyEvent,
) -> FormAction {
    use crossterm::event::KeyCode;

    let sub_fields = match &field.sub_fields {
        Some(subs) if !subs.is_empty() => subs,
        _ => return FormAction::None,
    };
    let col_count = sub_fields.len();
    let field_name = field.name.clone();

    // Snapshot navigation state to avoid borrow issues
    let row = form_state.composite_row;
    let col = form_state.composite_col;

    match key.code {
        KeyCode::Esc => {
            form_state.composite_open = false;
        }

        KeyCode::Enter => {
            let rows = form_state.composite_values.entry(field_name).or_default();
            let new_row = vec![String::new(); col_count];
            if rows.is_empty() {
                rows.push(new_row);
                form_state.composite_row = 0;
            } else {
                let insert_at = (row + 1).min(rows.len());
                rows.insert(insert_at, new_row);
                form_state.composite_row = insert_at;
            }
            form_state.composite_col = 0;
            form_state.cursor_position = 0;
        }

        KeyCode::Delete => {
            let rows = form_state.composite_values.entry(field_name).or_default();
            if !rows.is_empty() {
                let idx = row.min(rows.len() - 1);
                rows.remove(idx);
                if rows.is_empty() {
                    form_state.composite_row = 0;
                } else {
                    form_state.composite_row = row.min(rows.len() - 1);
                }
                form_state.cursor_position = 0;
            }
        }

        KeyCode::Tab => {
            let rows = form_state.composite_values.get(&field_name);
            let row_count = rows.map(|r| r.len()).unwrap_or(0);
            if row_count == 0 {
                return FormAction::None;
            }
            let mut new_col = col + 1;
            let mut new_row = row;
            if new_col >= col_count {
                new_col = 0;
                new_row = (row + 1).min(row_count - 1);
            }
            form_state.composite_col = new_col;
            form_state.composite_row = new_row;
            form_state.cursor_position = composite_cell_len(form_state, &field_name);
        }

        KeyCode::BackTab => {
            let rows = form_state.composite_values.get(&field_name);
            if rows.map(|r| r.len()).unwrap_or(0) == 0 {
                return FormAction::None;
            }
            if col == 0 {
                if row > 0 {
                    form_state.composite_row = row - 1;
                    form_state.composite_col = col_count - 1;
                }
            } else {
                form_state.composite_col = col - 1;
            }
            form_state.cursor_position = composite_cell_len(form_state, &field_name);
        }

        KeyCode::Up => {
            let row_count = form_state
                .composite_values
                .get(&field_name)
                .map(|r| r.len())
                .unwrap_or(0);
            if row_count > 0 && row > 0 {
                form_state.composite_row = row - 1;
            }
            form_state.cursor_position = composite_cell_len(form_state, &field_name);
        }

        KeyCode::Down => {
            let row_count = form_state
                .composite_values
                .get(&field_name)
                .map(|r| r.len())
                .unwrap_or(0);
            if row_count > 0 && row < row_count - 1 {
                form_state.composite_row = row + 1;
            }
            form_state.cursor_position = composite_cell_len(form_state, &field_name);
        }

        KeyCode::Left => {
            if let Some(sub) = sub_fields.get(col) {
                if sub.field_type == SubFieldType::StaticSelect {
                    cycle_composite_select_in(form_state, &field_name, sub, -1);
                } else if form_state.cursor_position > 0 {
                    form_state.cursor_position -= 1;
                }
            }
        }

        KeyCode::Right => {
            if let Some(sub) = sub_fields.get(col) {
                if sub.field_type == SubFieldType::StaticSelect {
                    cycle_composite_select_in(form_state, &field_name, sub, 1);
                } else {
                    let len = composite_cell_len(form_state, &field_name);
                    if form_state.cursor_position < len {
                        form_state.cursor_position += 1;
                    }
                }
            }
        }

        KeyCode::Char(' ') => {
            if let Some(sub) = sub_fields.get(col)
                && sub.field_type == SubFieldType::StaticSelect
            {
                cycle_composite_select_in(form_state, &field_name, sub, 1);
                return FormAction::None;
            }
            insert_composite_char_in(form_state, &field_name, sub_fields, ' ');
        }

        KeyCode::Char(c) => {
            insert_composite_char_in(form_state, &field_name, sub_fields, c);
        }

        KeyCode::Backspace => {
            let r = form_state.composite_row;
            let c = form_state.composite_col;
            if let Some(rows) = form_state.composite_values.get_mut(&field_name)
                && let Some(row) = rows.get_mut(r)
                && let Some(cell) = row.get_mut(c)
                && form_state.cursor_position > 0
                && !cell.is_empty()
            {
                let pos = form_state.cursor_position.min(cell.len());
                cell.remove(pos - 1);
                form_state.cursor_position = pos - 1;
            }
        }

        _ => {}
    }

    FormAction::None
}

/// Get the length of the current composite cell value.
fn composite_cell_len(form_state: &FormState, field_name: &str) -> usize {
    form_state
        .composite_values
        .get(field_name)
        .and_then(|rows| rows.get(form_state.composite_row))
        .and_then(|row| row.get(form_state.composite_col))
        .map(|v| v.len())
        .unwrap_or(0)
}

/// Insert a character into the active composite cell.
fn insert_composite_char_in(
    form_state: &mut FormState,
    field_name: &str,
    sub_fields: &[crate::config::SubFieldConfig],
    c: char,
) {
    if let Some(sub) = sub_fields.get(form_state.composite_col) {
        if sub.field_type == SubFieldType::StaticSelect {
            return;
        }
        if sub.field_type == SubFieldType::Number && !c.is_ascii_digit() && c != '.' && c != '-' {
            return;
        }
    }

    let r = form_state.composite_row;
    let col = form_state.composite_col;
    if let Some(rows) = form_state.composite_values.get_mut(field_name)
        && let Some(row) = rows.get_mut(r)
        && let Some(cell) = row.get_mut(col)
    {
        let pos = form_state.cursor_position.min(cell.len());
        cell.insert(pos, c);
        form_state.cursor_position = pos + 1;
    }
}

/// Cycle through options for a static_select sub-field in a composite row.
fn cycle_composite_select_in(
    form_state: &mut FormState,
    field_name: &str,
    sub: &crate::config::SubFieldConfig,
    delta: i32,
) {
    let options = match &sub.options {
        Some(opts) if !opts.is_empty() => opts,
        _ => return,
    };

    let r = form_state.composite_row;
    let c = form_state.composite_col;
    if let Some(rows) = form_state.composite_values.get_mut(field_name)
        && let Some(row) = rows.get_mut(r)
        && let Some(cell) = row.get_mut(c)
    {
        let current_idx = options.iter().position(|o| o == cell);
        let new_idx = match current_idx {
            Some(idx) => {
                let len = options.len() as i32;
                ((idx as i32 + delta).rem_euclid(len)) as usize
            }
            None => 0,
        };
        if let Some(new_value) = options.get(new_idx) {
            *cell = new_value.clone();
        }
    }
}

/// Move a flat cursor position up or down by one line within multiline text.
fn move_cursor_vertically(text: &str, cursor: usize, delta: i32) -> usize {
    // Find which line and column the cursor is on
    let mut line_start = 0;
    let mut current_line = 0;
    let mut col = cursor;
    for (i, line) in text.split('\n').enumerate() {
        if cursor <= line_start + line.len() {
            current_line = i;
            col = cursor - line_start;
            break;
        }
        line_start += line.len() + 1;
    }

    let target_line = (current_line as i32 + delta).max(0) as usize;

    // Walk to the target line and clamp column
    let mut pos = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i == target_line {
            return pos + col.min(line.len());
        }
        pos += line.len() + 1;
    }
    // Past end — clamp to end of text
    text.len()
}
