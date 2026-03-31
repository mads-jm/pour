use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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

    // Title
    let display_name = module
        .display_name
        .as_deref()
        .unwrap_or(module_key.as_str());
    let title = Paragraph::new(Line::from(vec![Span::styled(
        format!(" {display_name} "),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]))
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
            Span::styled(" Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" next  "),
            Span::styled("Shift+Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" prev  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" submit  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])
    };
    let footer = Paragraph::new(footer_content).block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);
}

/// Render the vertical list of form fields.
fn render_fields(frame: &mut Frame, area: Rect, fields: &[FieldConfig], form_state: &FormState) {
    let items: Vec<ListItem> = fields
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
                    if value.is_empty() {
                        "<select>".to_string()
                    } else {
                        value.to_string()
                    }
                }
                FieldType::Textarea => {
                    if value.is_empty() {
                        "<enter text>".to_string()
                    } else {
                        // Show first line with ellipsis if multiline
                        let first_line = value.lines().next().unwrap_or("");
                        if value.contains('\n') {
                            format!("{first_line}...")
                        } else {
                            first_line.to_string()
                        }
                    }
                }
                _ => {
                    if is_active {
                        // Show cursor indicator
                        format!("{value}_")
                    } else if value.is_empty() {
                        "<empty>".to_string()
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

            let indicator = if is_active { ">" } else { " " };

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

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);

    // If active field is a select type, render the options popup below
    if let Some(field) = fields.get(form_state.active_field)
        && matches!(
            field.field_type,
            FieldType::StaticSelect | FieldType::DynamicSelect
        )
    {
        render_select_options(frame, area, field, form_state);
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
            let marker = if is_selected { "> " } else { "  " };
            ListItem::new(Line::from(Span::styled(format!("{marker}{opt}"), style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Options ")
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(list, options_area);
}

/// Handle a key event while in Form view.
///
/// Returns `true` if the key was consumed, `false` otherwise.
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
    let field_count = module.fields.len();

    let form_state = match &mut app.form_state {
        Some(fs) => fs,
        None => return FormAction::None,
    };

    let active_field = module.fields.get(form_state.active_field);
    let is_select = active_field
        .map(|f| {
            matches!(
                f.field_type,
                FieldType::StaticSelect | FieldType::DynamicSelect
            )
        })
        .unwrap_or(false);

    match key.code {
        KeyCode::Esc => FormAction::Cancel,

        KeyCode::Tab => {
            if field_count > 0 {
                form_state.active_field = (form_state.active_field + 1) % field_count;
                form_state.cursor_position = current_value_len(form_state, module);
            }
            FormAction::None
        }

        KeyCode::BackTab => {
            if field_count > 0 {
                form_state.active_field = if form_state.active_field == 0 {
                    field_count - 1
                } else {
                    form_state.active_field - 1
                };
                form_state.cursor_position = current_value_len(form_state, module);
            }
            FormAction::None
        }

        KeyCode::Enter => {
            if is_select {
                // For selects, Enter confirms the current selection — cycle to next field
                if field_count > 0 {
                    form_state.active_field = (form_state.active_field + 1) % field_count;
                    form_state.cursor_position = current_value_len(form_state, module);
                }
                FormAction::None
            } else {
                FormAction::Submit
            }
        }

        KeyCode::Up if is_select => {
            if let Some(field) = active_field {
                cycle_select(form_state, &field.name, -1);
            }
            FormAction::None
        }

        KeyCode::Down if is_select => {
            if let Some(field) = active_field {
                cycle_select(form_state, &field.name, 1);
            }
            FormAction::None
        }

        KeyCode::Char(c) => {
            if let Some(field) = active_field {
                // For number fields, only allow digits and decimal point
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
