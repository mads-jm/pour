/// Small overlays: callout_title + preset_save (render + key).
/// These are too small to justify individual files (~100 LOC each).
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{
    CalloutTitleEdit, FormState, PresetDialogFocus, PresetDialogTarget, PresetSaveDialog,
};
use crate::tui::form::FormAction;

// ── Callout title overlay — render ───────────────────────────────────────────

pub(in crate::tui::form) fn render_callout_title(
    frame: &mut Frame,
    area: Rect,
    edit: &CalloutTitleEdit,
) {
    if area.height < 7 || area.width < 30 {
        return;
    }

    let modal_width = (area.width * 3 / 5)
        .max(40)
        .min(area.width.saturating_sub(4));
    let modal_height = 5u16;
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);
    let title = format!(" Callout Title \u{2014} {} ", edit.field_name);
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

    let label_style = Style::default().fg(Color::Cyan);
    let placeholder_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);

    let text_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let (text, text_style) = if edit.buffer.is_empty() {
        (
            "<title \u{2014} blank to clear>".to_string(),
            placeholder_style,
        )
    } else {
        (edit.buffer.clone(), value_style)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Title: ", label_style),
            Span::styled(text, text_style),
        ])),
        text_area,
    );

    let hint_area = Rect::new(inner.x, inner.y + 2, inner.width, 1);
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" save  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" cancel"),
    ]));
    frame.render_widget(hint, hint_area);

    let cx = inner.x + 7 + edit.cursor as u16;
    if cx < modal_area.x + modal_area.width - 1 {
        frame.set_cursor_position(Position::new(cx, inner.y));
    }
}

// ── Callout title overlay — key ───────────────────────────────────────────────

pub(in crate::tui::form) fn handle_callout_title_key(
    form_state: &mut FormState,
    key: crossterm::event::KeyEvent,
) -> FormAction {
    use crossterm::event::KeyCode;

    let edit = match &mut form_state.callout_title_edit {
        Some(e) => e,
        None => return FormAction::None,
    };

    match key.code {
        KeyCode::Esc => {
            form_state.callout_title_edit = None;
        }
        KeyCode::Enter => {
            let field_name = edit.field_name.clone();
            let trimmed = edit.buffer.trim().to_string();
            if trimmed.is_empty() {
                form_state.callout_titles.remove(&field_name);
            } else {
                form_state.callout_titles.insert(field_name, trimmed);
            }
            form_state.callout_title_edit = None;
        }
        KeyCode::Backspace if edit.cursor > 0 => {
            let byte_pos = edit
                .buffer
                .char_indices()
                .nth(edit.cursor - 1)
                .map(|(i, _)| i)
                .unwrap_or(0);
            edit.buffer.remove(byte_pos);
            edit.cursor -= 1;
        }
        KeyCode::Left if edit.cursor > 0 => {
            edit.cursor -= 1;
        }
        KeyCode::Right if edit.cursor < edit.buffer.chars().count() => {
            edit.cursor += 1;
        }
        KeyCode::Home => {
            edit.cursor = 0;
        }
        KeyCode::End => {
            edit.cursor = edit.buffer.chars().count();
        }
        KeyCode::Char(c) if edit.buffer.chars().count() < 120 => {
            let byte_pos = edit
                .buffer
                .char_indices()
                .nth(edit.cursor)
                .map(|(i, _)| i)
                .unwrap_or(edit.buffer.len());
            edit.buffer.insert(byte_pos, c);
            edit.cursor += 1;
        }
        _ => {}
    }
    FormAction::None
}

// ── Preset save overlay — render ──────────────────────────────────────────────

pub(in crate::tui::form) fn render_preset_save(
    frame: &mut Frame,
    area: Rect,
    overlay: &PresetSaveDialog,
) {
    if area.height < 10 || area.width < 30 {
        return;
    }

    let modal_width = (area.width * 3 / 5)
        .max(40)
        .min(area.width.saturating_sub(4));
    let modal_height = 7u16;
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Save Preset ")
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, modal_area);

    let inner = Rect::new(
        modal_area.x + 1,
        modal_area.y + 1,
        modal_area.width.saturating_sub(2),
        modal_area.height.saturating_sub(2),
    );

    let label_style = Style::default().fg(Color::Cyan);
    let placeholder_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);

    let name_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let (name_text, name_style) = if overlay.name_buffer.is_empty() {
        ("<name>".to_string(), placeholder_style)
    } else {
        (overlay.name_buffer.clone(), value_style)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Name: ", label_style),
            Span::styled(name_text, name_style),
        ])),
        name_area,
    );

    let desc_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    let (desc_text, desc_style) = if overlay.description_buffer.is_empty() {
        (
            "<description \u{2014} optional>".to_string(),
            placeholder_style,
        )
    } else {
        (overlay.description_buffer.clone(), value_style)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Desc: ", label_style),
            Span::styled(desc_text, desc_style),
        ])),
        desc_area,
    );

    let hint_area = Rect::new(inner.x, inner.y + 3, inner.width, 1);
    let hint = if overlay.awaiting_overwrite_confirm {
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("Overwrite \"{}\"? ", overlay.name_buffer.trim()),
                Style::default().fg(Color::Red),
            ),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" confirm  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" switch  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ]))
    };
    frame.render_widget(hint, hint_area);

    let (cursor_row, cursor_col) = match overlay.focus {
        PresetDialogFocus::Name => (inner.y, overlay.cursor_position),
        PresetDialogFocus::Description => (inner.y + 1, overlay.description_cursor),
    };
    let cx = inner.x + 6 + cursor_col as u16;
    if cx < modal_area.x + modal_area.width - 1 {
        frame.set_cursor_position(Position::new(cx, cursor_row));
    }
}

// ── Preset save overlay — key ─────────────────────────────────────────────────

/// Handle key events while the preset save overlay is open.
///
/// All keys are consumed. Enter saves, Esc cancels, text keys edit the name buffer.
pub(in crate::tui::form) fn handle_preset_save_key(
    form_state: &mut FormState,
    module_key: &str,
    module: &crate::config::ModuleConfig,
    key: crossterm::event::KeyEvent,
) -> FormAction {
    use crossterm::event::KeyCode;

    let overlay = match &mut form_state.preset_overlay {
        Some(o) => o,
        None => return FormAction::None,
    };

    let max_len = |focus: PresetDialogFocus| match focus {
        PresetDialogFocus::Name => 50,
        PresetDialogFocus::Description => 120,
    };

    match key.code {
        KeyCode::Esc => {
            form_state.preset_overlay = None;
            FormAction::None
        }
        KeyCode::Enter => {
            let name = overlay.name_buffer.trim().to_string();
            if name.is_empty() {
                return FormAction::None;
            }
            let description = {
                let trimmed = overlay.description_buffer.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            };
            if matches!(&overlay.target, PresetDialogTarget::Module) {
                let is_collision = form_state.preset_names.contains(&name);
                let editing_same = form_state
                    .selected_preset_name
                    .as_deref()
                    .map(|n| n == name)
                    .unwrap_or(false);
                if is_collision && !editing_same && !overlay.awaiting_overwrite_confirm {
                    overlay.awaiting_overwrite_confirm = true;
                    return FormAction::None;
                }
                overlay.awaiting_overwrite_confirm = false;
            }
            match &overlay.target {
                PresetDialogTarget::CompositeField { field_name } => {
                    let field_name = field_name.clone();
                    let rows: Vec<Vec<String>> = form_state
                        .composite_values
                        .get(&field_name)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|r| r.iter().any(|c| !c.is_empty()))
                        .collect();
                    if rows.is_empty() {
                        form_state.preset_overlay = None;
                        return FormAction::None;
                    }
                    form_state.preset_overlay = None;
                    let _ = module_key;
                    FormAction::SaveFieldPreset {
                        field_name,
                        name,
                        description,
                        rows,
                    }
                }
                PresetDialogTarget::Module => {
                    let visible_indices = crate::visibility::visible_field_indices(
                        &module.fields,
                        &form_state.field_values,
                    );
                    let mut values = std::collections::HashMap::new();
                    for &ci in &visible_indices {
                        let field = &module.fields[ci];
                        if field.preset_exclude == Some(true) {
                            continue;
                        }
                        if field.field_type == crate::config::FieldType::CompositeArray {
                            continue;
                        }
                        let val = form_state
                            .field_values
                            .get(&field.name)
                            .cloned()
                            .unwrap_or_default();
                        if !val.is_empty() {
                            values.insert(field.name.clone(), val);
                        }
                    }
                    let _ = module_key;
                    form_state.preset_overlay = None;
                    FormAction::SavePreset {
                        name,
                        description,
                        values,
                    }
                }
            }
        }
        KeyCode::Tab | KeyCode::Up | KeyCode::Down | KeyCode::BackTab => {
            overlay.focus = match overlay.focus {
                PresetDialogFocus::Name => PresetDialogFocus::Description,
                PresetDialogFocus::Description => PresetDialogFocus::Name,
            };
            FormAction::None
        }
        KeyCode::Backspace => {
            let is_name_focus = matches!(overlay.focus, PresetDialogFocus::Name);
            let (buffer, cursor) = match overlay.focus {
                PresetDialogFocus::Name => (&mut overlay.name_buffer, &mut overlay.cursor_position),
                PresetDialogFocus::Description => (
                    &mut overlay.description_buffer,
                    &mut overlay.description_cursor,
                ),
            };
            if *cursor > 0 {
                let byte_pos = buffer
                    .char_indices()
                    .nth(*cursor - 1)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                buffer.remove(byte_pos);
                *cursor -= 1;
            }
            if is_name_focus {
                overlay.name_was_user_edited = true;
                overlay.awaiting_overwrite_confirm = false;
            }
            FormAction::None
        }
        KeyCode::Left => {
            let cursor = match overlay.focus {
                PresetDialogFocus::Name => &mut overlay.cursor_position,
                PresetDialogFocus::Description => &mut overlay.description_cursor,
            };
            if *cursor > 0 {
                *cursor -= 1;
            }
            FormAction::None
        }
        KeyCode::Right => {
            let (buffer, cursor) = match overlay.focus {
                PresetDialogFocus::Name => (&overlay.name_buffer, &mut overlay.cursor_position),
                PresetDialogFocus::Description => {
                    (&overlay.description_buffer, &mut overlay.description_cursor)
                }
            };
            let char_count = buffer.chars().count();
            if *cursor < char_count {
                *cursor += 1;
            }
            FormAction::None
        }
        KeyCode::Char(c) => {
            let limit = max_len(overlay.focus);
            let (buffer, cursor) = match overlay.focus {
                PresetDialogFocus::Name => (&mut overlay.name_buffer, &mut overlay.cursor_position),
                PresetDialogFocus::Description => (
                    &mut overlay.description_buffer,
                    &mut overlay.description_cursor,
                ),
            };
            if buffer.chars().count() < limit {
                let byte_pos = buffer
                    .char_indices()
                    .nth(*cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(buffer.len());
                buffer.insert(byte_pos, c);
                *cursor += 1;
                overlay.awaiting_overwrite_confirm = false;
                if matches!(overlay.focus, PresetDialogFocus::Name) {
                    overlay.name_was_user_edited = true;
                }
            }
            FormAction::None
        }
        _ => FormAction::None,
    }
}
