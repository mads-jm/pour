use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::{App, FormState};
use crate::config::{FieldConfig, FieldType};

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
    items.push(ListItem::new(Line::from(vec![
        Span::raw(""),
    ])));
    items.push(ListItem::new(Line::from(vec![
        Span::styled(
            format!("{submit_indicator} [ pour ]"),
            submit_style,
        ),
    ])));

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);

    // Place the terminal block cursor for text/textarea/number fields
    if !submit_active {
        if let Some(field) = fields.get(form_state.active_field) {
            let is_text_input = matches!(
                field.field_type,
                FieldType::Text | FieldType::Number
            );
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
    }

    // If active field is a select type AND the dropdown is open, render the options popup below
    if form_state.dropdown_open {
        if let Some(field) = fields.get(form_state.active_field)
            && matches!(
                field.field_type,
                FieldType::StaticSelect | FieldType::DynamicSelect
            )
        {
            render_select_options(frame, area, field, form_state);
        }
    }

    // If active field is a textarea AND the editor is open, render the text editor overlay
    if form_state.textarea_open {
        if let Some(field) = fields.get(form_state.active_field)
            && field.field_type == FieldType::Textarea
        {
            render_textarea_editor(frame, area, field, form_state);
        }
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

    let lines: Vec<Line> = if value.is_empty() {
        vec![Line::from("")]
    } else {
        value.lines().map(|l| Line::from(l.to_string())).collect()
    };

    // Position below the active field row, fill available space
    let y_offset = (form_state.active_field as u16 + 1).min(area.height.saturating_sub(1));
    let editor_area = Rect {
        x: area.x + 4,
        y: area.y + y_offset,
        width: area.width.saturating_sub(8).min(60),
        height: area.height.saturating_sub(y_offset).min(10).max(4),
    };

    if editor_area.height < 3 {
        return;
    }

    let editor = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", field.prompt))
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(Clear, editor_area);
    frame.render_widget(editor, editor_area);

    // Place cursor inside the editor overlay
    // Find the line and column from the flat cursor_position
    let mut remaining = form_state.cursor_position;
    let mut cursor_line: u16 = 0;
    let mut cursor_col: u16 = 0;
    for line in value.split('\n') {
        if remaining <= line.len() {
            cursor_col = remaining as u16;
            break;
        }
        remaining -= line.len() + 1; // +1 for the newline
        cursor_line += 1;
    }

    // +1 for border
    let cx = editor_area.x + 1 + cursor_col;
    let cy = editor_area.y + 1 + cursor_line;
    if cx < editor_area.x + editor_area.width - 1 && cy < editor_area.y + editor_area.height - 1 {
        frame.set_cursor_position(Position::new(cx, cy));
    }
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
            form_state.active_field = (form_state.active_field + 1) % navigable_count;
            form_state.cursor_position = current_value_len(form_state, module);
            FormAction::None
        }

        // Shift+Tab: always move backward one field, close overlays
        KeyCode::BackTab => {
            form_state.dropdown_open = false;
            form_state.textarea_open = false;
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
                    form_state.cursor_position = move_cursor_vertically(value, form_state.cursor_position, -1);
                }
            } else {
                form_state.dropdown_open = false;
                form_state.textarea_open = false;
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
                    form_state.cursor_position = move_cursor_vertically(value, form_state.cursor_position, 1);
                }
            } else {
                form_state.dropdown_open = false;
                form_state.textarea_open = false;
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
            if on_submit_button || is_select || (is_textarea && !form_state.textarea_open) {
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
            }
            FormAction::None
        }

        KeyCode::Backspace => {
            if on_submit_button || is_select || (is_textarea && !form_state.textarea_open) {
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
            }
            FormAction::None
        }

        KeyCode::Left => {
            if form_state.cursor_position > 0 {
                form_state.cursor_position -= 1;
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
