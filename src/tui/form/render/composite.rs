use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{FieldPresetPickerState, FormState};
use crate::config::FieldConfig;

/// Render a bordered table editor overlay for composite_array fields.
pub(super) fn render_composite_editor(
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
    let mut widths: Vec<usize> = sub_fields
        .iter()
        .map(|s| UnicodeWidthStr::width(s.prompt.as_str()).max(6))
        .collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(UnicodeWidthStr::width(cell.as_str()).max(1));
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

    // Status / preset subtitle: shows the last applied preset name (if any) or
    // a transient status message (e.g. "preset shape adjusted").
    if let Some(status) = &form_state.composite_status {
        lines.push(Line::from(Span::styled(
            format!(" {status}"),
            Style::default().fg(Color::DarkGray),
        )));
    } else if let Some(name) = form_state.last_applied_field_preset.get(&field.name) {
        lines.push(Line::from(Span::styled(
            format!(" preset: {name}"),
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Hint line
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" Tab", Style::default().fg(Color::Yellow)),
        Span::raw(" next  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" add  "),
        Span::styled("Del", Style::default().fg(Color::Yellow)),
        Span::raw(" remove  "),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(" save  "),
        Span::styled("l", Style::default().fg(Color::Yellow)),
        Span::raw(" load  "),
        Span::styled("p", Style::default().fg(Color::Yellow)),
        Span::raw(" cycle  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" close"),
    ]));

    // Extra row to fit the optional subtitle without clipping the hint.
    let extra: u16 = if form_state.composite_status.is_some()
        || form_state
            .last_applied_field_preset
            .contains_key(&field.name)
    {
        1
    } else {
        0
    };
    let editor_area = Rect {
        height: editor_area.height.saturating_add(extra).min(
            area.height
                .saturating_sub(editor_area.y.saturating_sub(area.y)),
        ),
        ..editor_area
    };

    let editor = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", field.prompt))
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(Clear, editor_area);
    frame.render_widget(editor, editor_area);
}

/// Render the centered modal overlay listing saved per-field presets for a
/// composite_array field. Shown on top of the composite editor when the user
/// presses `l`. Mirrors the styling of the module-level preset save dialog.
pub(super) fn render_field_preset_picker(
    frame: &mut Frame,
    area: Rect,
    picker: &FieldPresetPickerState,
) {
    if area.height < 10 || area.width < 30 {
        return;
    }

    let modal_width = (area.width * 3 / 5)
        .max(44)
        .min(area.width.saturating_sub(4));
    let row_count = picker.names.len().max(1) as u16;
    let modal_height = (row_count + 5).min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Load Preset — {} ", picker.field_name))
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, modal_area);

    let inner = Rect::new(
        modal_area.x + 1,
        modal_area.y + 1,
        modal_area.width.saturating_sub(2),
        modal_area.height.saturating_sub(2),
    );

    let mut lines: Vec<Line> = Vec::new();
    if picker.names.is_empty() {
        lines.push(Line::from(Span::styled(
            " (no saved presets — press s in the editor to save one)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, name) in picker.names.iter().enumerate() {
            let selected = i == picker.selected;
            let marker = if selected { "▸ " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let mut spans = vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(name.clone(), style),
            ];
            if let Some(Some(desc)) = picker.descriptions.get(i) {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("— {desc}"),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(spans));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" load  "),
        Span::styled("Ctrl+D", Style::default().fg(Color::Yellow)),
        Span::raw(" delete  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" cancel"),
    ]));

    frame.render_widget(Paragraph::new(lines), inner);
}
