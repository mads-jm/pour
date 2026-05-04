use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{FormState, SubFormState};
use crate::config::TemplateFieldType;
use crate::tui::form::FormAction;

// ── Render ────────────────────────────────────────────────────────────────────

/// Render a centered modal overlay for template-driven inline note creation.
///
/// Shows the template fields with the same visual style as the main form.
/// A `[ create ]` button at the bottom submits the sub-form.
pub(in crate::tui::form) fn render(
    frame: &mut Frame,
    area: Rect,
    sub_form: &SubFormState,
    template: &crate::config::TemplateConfig,
) {
    if area.height < 10 || area.width < 30 {
        return;
    }

    let field_count = template.fields.len();
    let error_row: u16 = if sub_form.error_message.is_some() {
        1
    } else {
        0
    };
    let modal_height = (field_count as u16 + 5 + error_row).min(area.height.saturating_sub(4));
    let modal_width = (area.width * 3 / 5)
        .max(30)
        .min(area.width.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let title = format!(" New: {} ", sub_form.note_name);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, modal_area);

    let inner = Rect::new(
        modal_area.x + 1,
        modal_area.y + 1,
        modal_area.width.saturating_sub(2),
        modal_area.height.saturating_sub(2),
    );

    let on_submit_button = sub_form.active_field == field_count;
    for (i, tfield) in template.fields.iter().enumerate() {
        let row_y = inner.y + i as u16;
        if row_y >= inner.y + inner.height.saturating_sub(2) {
            break;
        }

        let is_active = i == sub_form.active_field;
        let value = sub_form
            .field_values
            .get(&tfield.name)
            .map(|s| s.as_str())
            .unwrap_or("");

        let label_width = 14u16;
        let label_area = Rect::new(inner.x, row_y, label_width.min(inner.width), 1);
        let label_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let indicator = if is_active { "\u{25B8} " } else { "  " };
        let label_text = format!("{indicator}{}", tfield.prompt);
        let label = Paragraph::new(Line::from(Span::styled(
            if label_text.chars().count() > label_width as usize {
                let truncated: String = label_text.chars().take(label_width as usize - 1).collect();
                format!("{truncated}\u{2026}")
            } else {
                let char_count = label_text.chars().count();
                let padding = label_width as usize - char_count;
                format!("{label_text}{}", " ".repeat(padding))
            },
            label_style,
        )));
        frame.render_widget(label, label_area);

        let value_x = inner.x + label_width;
        let value_width = inner.width.saturating_sub(label_width);
        let value_area = Rect::new(value_x, row_y, value_width, 1);

        let (display_val, value_style) = if tfield.field_type == TemplateFieldType::StaticSelect {
            let extensible = tfield.allow_create.unwrap_or(false);
            let inner_val = if value.is_empty() { "select" } else { value };
            let text = if is_active && !extensible {
                format!("\u{25C2} {inner_val} \u{25B8}")
            } else {
                inner_val.to_string()
            };
            let style = if is_active {
                Style::default().fg(Color::White)
            } else if value.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            (text, style)
        } else {
            let text = if value.is_empty() {
                "\u{2026}".to_string()
            } else {
                value.to_string()
            };
            let style = if is_active {
                Style::default().fg(Color::White)
            } else if value.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            (text, style)
        };
        let val_widget = Paragraph::new(Line::from(Span::styled(display_val, value_style)));
        frame.render_widget(val_widget, value_area);

        let extensible_static = tfield.field_type == TemplateFieldType::StaticSelect
            && tfield.allow_create.unwrap_or(false);
        if is_active && (tfield.field_type != TemplateFieldType::StaticSelect || extensible_static)
        {
            let cx = value_x + sub_form.cursor_position as u16;
            if cx < value_x + value_width {
                frame.set_cursor_position(Position::new(cx, row_y));
            }
        }
    }

    let button_y = inner.y + inner.height.saturating_sub(2 + error_row);
    if button_y > inner.y {
        let button_area = Rect::new(inner.x, button_y, inner.width, 1);
        let button_style = if on_submit_button {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let button = Paragraph::new(Line::from(Span::styled("  [ create ]", button_style)));
        frame.render_widget(button, button_area);
    }

    if let Some(ref err) = sub_form.error_message {
        let error_y = inner.y + inner.height.saturating_sub(1 + error_row);
        if error_y > inner.y {
            let error_area = Rect::new(inner.x, error_y, inner.width, 1);
            let msg = format!(" ! {err}");
            let error_widget = Paragraph::new(Line::from(Span::styled(
                msg,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            frame.render_widget(error_widget, error_area);
        }
    }

    let active_tfield = template.fields.get(sub_form.active_field);
    let on_extensible_static = active_tfield.is_some_and(|tf| {
        tf.field_type == TemplateFieldType::StaticSelect && tf.allow_create.unwrap_or(false)
    });
    let hint_y = inner.y + inner.height.saturating_sub(1);
    if hint_y > inner.y {
        let hint_area = Rect::new(inner.x, hint_y, inner.width, 1);
        let mut spans = vec![
            Span::styled(" \u{2191}\u{2193}", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("\u{2190}\u{2192}", Style::default().fg(Color::Yellow)),
            Span::raw(if on_extensible_static {
                " cycle  "
            } else {
                " select  "
            }),
        ];
        if on_extensible_static {
            spans.push(Span::styled("type", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" add new  "));
        }
        spans.push(Span::styled("Enter", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" submit  "));
        spans.push(Span::styled("Esc", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" cancel"));
        let hint = Paragraph::new(Line::from(spans));
        frame.render_widget(hint, hint_area);
    }
}

// ── Key handler ───────────────────────────────────────────────────────────────

/// Handle key events when the sub-form overlay is open.
///
/// All keys are consumed by the sub-form. Tab/Shift+Tab navigate fields,
/// Enter submits or toggles dropdowns, Esc cancels.
pub(in crate::tui::form) fn handle_key(
    form_state: &mut FormState,
    config: &crate::config::Config,
    key: crossterm::event::KeyEvent,
) -> FormAction {
    use crossterm::event::KeyCode;

    let sub_form = match &mut form_state.sub_form {
        Some(sf) => sf,
        None => return FormAction::None,
    };

    let template = config
        .templates
        .as_ref()
        .and_then(|t| t.get(&sub_form.template_name));
    let template = match template {
        Some(t) => t,
        None => return FormAction::None,
    };

    let field_count = template.fields.len();
    let navigable_count = field_count + 1;
    let on_submit_button = sub_form.active_field == field_count;
    let active_tfield = template.fields.get(sub_form.active_field);
    let is_static_select = active_tfield
        .map(|f| f.field_type == TemplateFieldType::StaticSelect)
        .unwrap_or(false);
    let is_static_select_extensible =
        is_static_select && active_tfield.and_then(|f| f.allow_create).unwrap_or(false);

    let sync_cursor = |sf: &mut SubFormState, tmpl: &crate::config::TemplateConfig| {
        if let Some(tf) = tmpl.fields.get(sf.active_field) {
            sf.cursor_position = sf
                .field_values
                .get(&tf.name)
                .map(|v| v.chars().count())
                .unwrap_or(0);
        } else {
            sf.cursor_position = 0;
        }
    };

    match key.code {
        KeyCode::Esc => {
            form_state.sub_form = None;
            FormAction::None
        }

        KeyCode::Down | KeyCode::Tab => {
            sub_form.active_field = (sub_form.active_field + 1) % navigable_count;
            sync_cursor(sub_form, template);
            FormAction::None
        }

        KeyCode::Up | KeyCode::BackTab => {
            sub_form.active_field = if sub_form.active_field == 0 {
                navigable_count - 1
            } else {
                sub_form.active_field - 1
            };
            sync_cursor(sub_form, template);
            FormAction::None
        }

        KeyCode::Enter => {
            if on_submit_button {
                return FormAction::CreateFromTemplate {
                    field_name: sub_form.parent_field_name.clone(),
                    template_name: sub_form.template_name.clone(),
                    note_name: sub_form.note_name.clone(),
                    field_values: sub_form.field_values.clone(),
                };
            }
            sub_form.active_field = (sub_form.active_field + 1) % navigable_count;
            sync_cursor(sub_form, template);
            FormAction::None
        }

        KeyCode::Left => {
            if is_static_select {
                if let Some(tf) = active_tfield
                    && let Some(opts) = sub_form.field_options.get(&tf.name)
                    && !opts.is_empty()
                {
                    let current = sub_form
                        .field_values
                        .get(&tf.name)
                        .cloned()
                        .unwrap_or_default();
                    let idx = opts.iter().position(|o| o == &current).unwrap_or(0);
                    let new_idx = if idx == 0 { opts.len() - 1 } else { idx - 1 };
                    sub_form
                        .field_values
                        .insert(tf.name.clone(), opts[new_idx].clone());
                }
            } else if sub_form.cursor_position > 0 {
                sub_form.cursor_position -= 1;
            }
            FormAction::None
        }

        KeyCode::Right => {
            if is_static_select {
                if let Some(tf) = active_tfield
                    && let Some(opts) = sub_form.field_options.get(&tf.name)
                    && !opts.is_empty()
                {
                    let current = sub_form
                        .field_values
                        .get(&tf.name)
                        .cloned()
                        .unwrap_or_default();
                    let idx = opts.iter().position(|o| o == &current).unwrap_or(0);
                    let new_idx = (idx + 1) % opts.len();
                    sub_form
                        .field_values
                        .insert(tf.name.clone(), opts[new_idx].clone());
                }
            } else if let Some(tf) = active_tfield {
                let char_count = sub_form
                    .field_values
                    .get(&tf.name)
                    .map(|v| v.chars().count())
                    .unwrap_or(0);
                if sub_form.cursor_position < char_count {
                    sub_form.cursor_position += 1;
                }
            }
            FormAction::None
        }

        KeyCode::Char(c) => {
            if on_submit_button || (is_static_select && !is_static_select_extensible) {
                return FormAction::None;
            }
            if let Some(tf) = active_tfield {
                if tf.field_type == TemplateFieldType::Number
                    && !c.is_ascii_digit()
                    && c != '.'
                    && c != '-'
                {
                    return FormAction::None;
                }
                if is_static_select_extensible {
                    let is_existing_option = sub_form
                        .field_options
                        .get(&tf.name)
                        .map(|opts| {
                            let current = sub_form
                                .field_values
                                .get(&tf.name)
                                .cloned()
                                .unwrap_or_default();
                            !current.is_empty() && opts.iter().any(|o| o == &current)
                        })
                        .unwrap_or(false);
                    if is_existing_option {
                        sub_form.field_values.insert(tf.name.clone(), String::new());
                        sub_form.cursor_position = 0;
                    }
                }
                let value = sub_form.field_values.entry(tf.name.clone()).or_default();
                let byte_pos = value
                    .char_indices()
                    .nth(sub_form.cursor_position)
                    .map(|(i, _)| i)
                    .unwrap_or(value.len());
                value.insert(byte_pos, c);
                sub_form.cursor_position += 1;
            }
            FormAction::None
        }

        KeyCode::Backspace => {
            if on_submit_button || (is_static_select && !is_static_select_extensible) {
                return FormAction::None;
            }
            if let Some(tf) = active_tfield {
                let value = sub_form.field_values.entry(tf.name.clone()).or_default();
                if sub_form.cursor_position > 0 {
                    let byte_pos = value
                        .char_indices()
                        .nth(sub_form.cursor_position - 1)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    value.remove(byte_pos);
                    sub_form.cursor_position -= 1;
                }
            }
            FormAction::None
        }

        _ => FormAction::None,
    }
}
