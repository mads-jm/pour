use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::{App, FormState, SubFormState};
use crate::config::{FieldConfig, FieldType, SubFieldType, TemplateFieldType};
use crate::visibility::visible_field_indices;

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

    // Sub-form overlay renders LAST so it paints over footer and fields
    if let Some(sub_form) = &form_state.sub_form {
        if let Some(templates) = &app.config.templates {
            if let Some(template) = templates.get(&sub_form.template_name) {
                render_sub_form(frame, area, sub_form, template);
            }
        }
    }
}

/// Render the vertical list of form fields plus a submit button row.
fn render_fields(frame: &mut Frame, area: Rect, fields: &[FieldConfig], form_state: &FormState) {
    // Compute which fields are currently visible given the form's current values.
    // `vi` (visible index) is the render position; `ci` (config index) is the field's
    // position in the original `fields` slice.
    let visible_indices = visible_field_indices(fields, &form_state.field_values);

    let submit_active = form_state.active_field == visible_indices.len();

    let mut items: Vec<ListItem> = visible_indices
        .iter()
        .enumerate()
        .map(|(vi, &ci)| {
            let field = &fields[ci];
            let is_active = vi == form_state.active_field;
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

            // Track whether this field is in active search/filter mode.
            let field_search_active = is_active
                && field.field_type == FieldType::DynamicSelect
                && field.allow_create.unwrap_or(false)
                && form_state
                    .search_buffers
                    .get(&field.name)
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);

            let value_display = match &field.field_type {
                FieldType::StaticSelect | FieldType::DynamicSelect => {
                    let display_text = if field_search_active {
                        form_state
                            .search_buffers
                            .get(&field.name)
                            .cloned()
                            .unwrap_or_default()
                    } else if value.is_empty() {
                        "<select>".to_string()
                    } else {
                        value.to_string()
                    };
                    // Show open/closed chevron when the field is active
                    if is_active {
                        if form_state.dropdown_open {
                            format!("{display_text} [^]")
                        } else {
                            format!("{display_text} [v]")
                        }
                    } else {
                        display_text
                    }
                }
                FieldType::Textarea => {
                    let callout_prefix = form_state
                        .callout_overrides
                        .get(&field.name)
                        .map(|c| format!("[!{c}] "))
                        .unwrap_or_default();
                    let label = if value.is_empty() {
                        format!("{callout_prefix}<enter text>")
                    } else {
                        let line_count = value.lines().count();
                        let first_line = value.lines().next().unwrap_or("");
                        if line_count > 1 {
                            format!("{callout_prefix}{first_line} [{line_count} lines]")
                        } else {
                            format!("{callout_prefix}{first_line}")
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

            // Search-mode gets a distinct style so the user knows they're filtering.
            let value_style = if field_search_active {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC)
            } else if is_active {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            let icon_prefix = field.icon.as_deref()
                .map(|i| format!("{i} "))
                .unwrap_or_default();

            let line = Line::from(vec![
                Span::styled(format!("{indicator} "), prompt_style),
                Span::styled(
                    format!("{icon_prefix}{}{}: ", field.prompt, required_marker),
                    prompt_style,
                ),
                Span::styled(value_display, value_style),
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

    // Resolve the active field's config-index from the visible list.
    let active_config_field = visible_indices
        .get(form_state.active_field)
        .and_then(|&ci| fields.get(ci));

    // Place the terminal block cursor for text/textarea/number fields
    if !submit_active
        && let Some(field) = active_config_field
    {
        let is_text_input = matches!(field.field_type, FieldType::Text | FieldType::Number);
        if is_text_input {
            // prefix: "▸ " (2) + prompt + required_marker (1) + ": " (2)
            let prefix_len = 2 + field.prompt.len() + 1 + 2;
            let cursor_x = area.x + prefix_len as u16 + form_state.cursor_position as u16;
            // `active_field` is the visible index, which is the render row.
            let cursor_y = area.y + form_state.active_field as u16;
            if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
                frame.set_cursor_position(Position::new(cursor_x, cursor_y));
            }
        }
    }

    // If active field is a select type AND the dropdown is open, render the options popup below
    if form_state.dropdown_open
        && let Some(field) = active_config_field
        && matches!(
            field.field_type,
            FieldType::StaticSelect | FieldType::DynamicSelect
        )
    {
        let search = if field.field_type == FieldType::DynamicSelect
            && field.allow_create.unwrap_or(false)
        {
            form_state
                .search_buffers
                .get(&field.name)
                .cloned()
                .unwrap_or_default()
        } else {
            String::new()
        };
        render_select_options(frame, area, field, form_state, &search);
    }

    // If active field is a textarea AND the editor is open, render the text editor overlay
    if form_state.textarea_open
        && let Some(field) = active_config_field
        && field.field_type == FieldType::Textarea
    {
        render_textarea_editor(frame, area, field, form_state);
    }

    // If active field is a composite_array AND the overlay is open, render the table editor
    if form_state.composite_open
        && let Some(field) = active_config_field
        && field.field_type == FieldType::CompositeArray
    {
        render_composite_editor(frame, area, field, form_state);
    }
}

/// Render a scrollable options list for select fields.
///
/// `search` is the current search buffer text. When non-empty, only options
/// matching the search (case-insensitive substring) are shown. An empty
/// `search` means show all options (the standard closed-list behaviour).
fn render_select_options(
    frame: &mut Frame,
    area: Rect,
    field: &FieldConfig,
    form_state: &FormState,
    search: &str,
) {
    let all_options = match form_state.field_options.get(&field.name) {
        Some(opts) if !opts.is_empty() => opts,
        _ => return,
    };

    // Apply search filter when the buffer is non-empty.
    let filtered: Vec<&String>;
    let options: &[&String] = if search.is_empty() {
        filtered = all_options.iter().collect();
        &filtered
    } else {
        filtered = all_options
            .iter()
            .filter(|o| o.to_lowercase().contains(&search.to_lowercase()))
            .collect();
        &filtered
    };

    let current_value = form_state
        .field_values
        .get(&field.name)
        .map(|s| s.as_str())
        .unwrap_or("");

    // Position the options list below the active field row.
    let y_offset = (form_state.active_field as u16).min(area.height.saturating_sub(1));

    // When searching and no options match, show a "+ Create" affordance hint.
    if options.is_empty() {
        let hint_area = Rect {
            x: area.x + 4,
            y: area.y + y_offset + 1,
            width: area.width.saturating_sub(8).min(40),
            height: 3,
        };
        if hint_area.y + hint_area.height > area.y + area.height {
            return;
        }
        let create_line = Line::from(vec![
            Span::styled(
                "  + ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Create ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("\"{search}\""),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        let hint = Paragraph::new(create_line).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" New ")
                .border_style(Style::default().fg(Color::Green)),
        );
        frame.render_widget(Clear, hint_area);
        frame.render_widget(hint, hint_area);
        return;
    }

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
            let is_selected = opt.as_str() == current_value;
            let base_style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let marker = if is_selected { "▸ " } else { "  " };

            // When filtering, highlight the matched portion within the option text.
            // We use char-level comparison to find the match position in the original
            // string, avoiding byte-index misalignment from case folding.
            if !search.is_empty() {
                let search_chars: Vec<char> =
                    search.chars().flat_map(|c| c.to_lowercase()).collect();
                let match_pos = opt
                    .char_indices()
                    .enumerate()
                    .find_map(|(_, (byte_idx, _))| {
                        let remaining = &opt[byte_idx..];
                        let mut opt_chars = remaining.chars();
                        let mut matched_bytes = 0usize;
                        for &sc in &search_chars {
                            match opt_chars.next() {
                                Some(oc) if oc.to_lowercase().next() == Some(sc) => {
                                    matched_bytes += oc.len_utf8();
                                }
                                _ => return None,
                            }
                        }
                        Some((byte_idx, matched_bytes))
                    });
                if let Some((match_start, match_len)) = match_pos {
                    let before = &opt[..match_start];
                    let matched = &opt[match_start..match_start + match_len];
                    let after = &opt[match_start + match_len..];
                    let highlight_style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    };
                    let line = Line::from(vec![
                        Span::styled(format!("{marker}{before}"), base_style),
                        Span::styled(matched, highlight_style),
                        Span::styled(after, base_style),
                    ]);
                    return ListItem::new(line);
                }
            }

            ListItem::new(Line::from(Span::styled(
                format!("{marker}{opt}"),
                base_style,
            )))
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
    let selected_idx = options
        .iter()
        .position(|o| o.as_str() == current_value)
        .unwrap_or(0);
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

/// Render a centered modal overlay for template-driven inline note creation.
///
/// Shows the template fields with the same visual style as the main form.
/// A `[ create ]` button at the bottom submits the sub-form.
fn render_sub_form(
    frame: &mut Frame,
    area: Rect,
    sub_form: &SubFormState,
    template: &crate::config::TemplateConfig,
) {
    // Graceful degradation: skip if terminal is too small
    if area.height < 10 || area.width < 30 {
        return;
    }

    // Centered modal: 60% width, height to fit fields + chrome
    let field_count = template.fields.len();
    let error_row: u16 = if sub_form.error_message.is_some() { 1 } else { 0 };
    let modal_height = (field_count as u16 + 5 + error_row).min(area.height.saturating_sub(4)); // fields + title + button + hints + borders + optional error
    let modal_width = (area.width * 3 / 5).max(30).min(area.width.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    // Clear background and draw bordered box
    frame.render_widget(Clear, modal_area);

    let title = format!(" New: {} ", sub_form.note_name);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, modal_area);

    // Inner area (inside borders)
    let inner = Rect::new(
        modal_area.x + 1,
        modal_area.y + 1,
        modal_area.width.saturating_sub(2),
        modal_area.height.saturating_sub(2),
    );

    // Render each template field
    let on_submit_button = sub_form.active_field == field_count;
    for (i, tfield) in template.fields.iter().enumerate() {
        let row_y = inner.y + i as u16;
        if row_y >= inner.y + inner.height.saturating_sub(2) {
            break; // leave room for button + hints
        }

        let is_active = i == sub_form.active_field;
        let value = sub_form
            .field_values
            .get(&tfield.name)
            .map(|s| s.as_str())
            .unwrap_or("");

        // Prompt label
        let label_width = 14u16;
        let label_area = Rect::new(inner.x, row_y, label_width.min(inner.width), 1);
        let label_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let indicator = if is_active { "▸ " } else { "  " };
        let label_text = format!("{indicator}{}", tfield.prompt);
        let label = Paragraph::new(Line::from(Span::styled(
            if label_text.chars().count() > label_width as usize {
                let truncated: String = label_text
                    .chars()
                    .take(label_width as usize - 1)
                    .collect();
                format!("{truncated}…")
            } else {
                // Pad with spaces to fill label_width (char-aware)
                let char_count = label_text.chars().count();
                let padding = label_width as usize - char_count;
                format!("{label_text}{}", " ".repeat(padding))
            },
            label_style,
        )));
        frame.render_widget(label, label_area);

        // Value
        let value_x = inner.x + label_width;
        let value_width = inner.width.saturating_sub(label_width);
        let value_area = Rect::new(value_x, row_y, value_width, 1);

        let (display_val, value_style) =
            if tfield.field_type == TemplateFieldType::StaticSelect {
                let inner_val = if value.is_empty() { "select" } else { value };
                let text = if is_active {
                    format!("◂ {inner_val} ▸")
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
                    "…".to_string()
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

        // Place cursor for active text/number fields
        if is_active && tfield.field_type != TemplateFieldType::StaticSelect {
            let cx = value_x + sub_form.cursor_position as u16;
            if cx < value_x + value_width {
                frame.set_cursor_position(Position::new(cx, row_y));
            }
        }
    }

    // Submit button row (above error + hint)
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

    // Error line (above hint, only when set)
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

    // Hint line
    let hint_y = inner.y + inner.height.saturating_sub(1);
    if hint_y > inner.y {
        let hint_area = Rect::new(inner.x, hint_y, inner.width, 1);
        let hint = Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("←→", Style::default().fg(Color::Yellow)),
            Span::raw(" select  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" submit  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ]));
        frame.render_widget(hint, hint_area);
    }

}

/// Recompute which field `active_field` should point at after a potential
/// visibility change.
///
/// Two-phase logic:
///
/// **Phase 1 — in-range check**: If `active_field <= submit_idx` (i.e., within
/// the current visible set or exactly on the submit button), check whether the
/// config field it resolves to is the same one recorded in `active_config_idx`.
/// - If they agree (or `active_field == submit_idx` and `active_config_idx`
///   is None): no action needed; just sync `active_config_idx` to match and return.
/// - If they disagree (stale `active_config_idx`, e.g. from a direct test
///   assignment): `active_field` wins — sync `active_config_idx` to the config
///   field at the current visible position and return.
///
/// **Phase 2 — out-of-range recovery**: If `active_field > submit_idx`, the
/// visible set has shrunk. Use `active_config_idx` to locate the intended field:
/// - If `active_config_idx` is `None` (was on submit), land on the new submit.
/// - If `active_config_idx` is `Some(ci)` and `ci` is still visible, move to
///   its new visible position.
/// - If `ci` is no longer visible, prefer next visible field (higher config
///   index), then previous, then submit.
fn clamp_active_to_visible(
    form_state: &mut FormState,
    fields: &[crate::config::FieldConfig],
) {
    let visible = visible_field_indices(fields, &form_state.field_values);
    let visible_count = visible.len();
    let submit_idx = visible_count;

    if form_state.active_field <= submit_idx {
        // Phase 1: active_field is in a valid position.
        let current_ci = visible.get(form_state.active_field).copied(); // None = submit
        // Always keep active_field, just sync the config index.
        form_state.active_config_idx = current_ci;
        return;
    }

    // Phase 2: active_field is out of range — visible set shrank.
    let prev_ci = match form_state.active_config_idx {
        None => {
            // Was on submit — keep on submit.
            form_state.active_field = submit_idx;
            return;
        }
        Some(ci) => ci,
    };

    if let Some(new_vi) = visible.iter().position(|&ci| ci == prev_ci) {
        form_state.active_field = new_vi;
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else if let Some(new_vi) = visible.iter().position(|&ci| ci > prev_ci) {
        form_state.active_field = new_vi;
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else if let Some(new_vi) = visible.iter().rposition(|&ci| ci < prev_ci) {
        form_state.active_field = new_vi;
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else {
        form_state.active_field = submit_idx;
        form_state.active_config_idx = None;
    }
}

/// Resolve the currently active `FieldConfig` using the visible index.
///
/// Returns `None` when the form is on the submit button.
fn active_field_config<'a>(
    form_state: &FormState,
    module: &'a crate::config::ModuleConfig,
) -> Option<&'a crate::config::FieldConfig> {
    let visible = visible_field_indices(&module.fields, &form_state.field_values);
    visible
        .get(form_state.active_field)
        .and_then(|&ci| module.fields.get(ci))
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

    let form_state = match &mut app.form_state {
        Some(fs) => fs,
        None => return FormAction::None,
    };

    // Recompute visibility on every key — accounts for any mutations from
    // the previous key that may have changed which fields are visible.
    clamp_active_to_visible(form_state, &module.fields);

    // navigable_count and submit detection are based on the VISIBLE set.
    let visible_indices = visible_field_indices(&module.fields, &form_state.field_values);
    let visible_count = visible_indices.len();
    let navigable_count = visible_count + 1; // +1 for submit button

    let on_submit_button = form_state.active_field == visible_count;
    // Resolve the active FieldConfig through the visible index.
    let active_field = visible_indices
        .get(form_state.active_field)
        .and_then(|&ci| module.fields.get(ci));
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
    // True only for dynamic_select fields that explicitly opt in to freetext creation.
    let is_dynamic_allow_create = active_field
        .map(|f| f.field_type == FieldType::DynamicSelect && f.allow_create.unwrap_or(false))
        .unwrap_or(false);

    // Sub-form overlay takes priority over all other overlays
    if form_state.sub_form.is_some() {
        return handle_sub_form_key(form_state, &app.config, key);
    }

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
            // If the search buffer has content, clear it first (without closing dropdown).
            if is_dynamic_allow_create
                && let Some(field) = active_field
                && form_state
                    .search_buffers
                    .get(&field.name)
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
            {
                form_state
                    .search_buffers
                    .insert(field.name.clone(), String::new());
                return FormAction::None;
            }
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
            if let Some(field) = active_field {
                form_state.search_buffers.remove(&field.name);
            }
            form_state.dropdown_open = false;
            form_state.textarea_open = false;
            form_state.textarea_scroll_offset = 0;
            form_state.composite_open = false;
            let new_af = (form_state.active_field + 1) % navigable_count;
            form_state.active_field = new_af;
            form_state.active_config_idx = visible_indices.get(new_af).copied();
            form_state.cursor_position = current_value_len(form_state, module);
            FormAction::None
        }

        // Shift+Tab: always move backward one field, close overlays
        KeyCode::BackTab => {
            if let Some(field) = active_field {
                form_state.search_buffers.remove(&field.name);
            }
            form_state.dropdown_open = false;
            form_state.textarea_open = false;
            form_state.textarea_scroll_offset = 0;
            form_state.composite_open = false;
            let new_af = if form_state.active_field == 0 {
                navigable_count - 1
            } else {
                form_state.active_field - 1
            };
            form_state.active_field = new_af;
            form_state.active_config_idx = visible_indices.get(new_af).copied();
            form_state.cursor_position = current_value_len(form_state, module);
            FormAction::None
        }

        // Up: cycle options when dropdown is open; navigate to previous field otherwise
        KeyCode::Up => {
            if is_select && form_state.dropdown_open {
                if let Some(field) = active_field {
                    let search = if is_dynamic_allow_create {
                        form_state
                            .search_buffers
                            .get(&field.name)
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    cycle_select_filtered(form_state, &field.name, -1, &search);
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
                let new_af = if form_state.active_field == 0 {
                    navigable_count - 1
                } else {
                    form_state.active_field - 1
                };
                form_state.active_field = new_af;
                form_state.active_config_idx = visible_indices.get(new_af).copied();
                form_state.cursor_position = current_value_len(form_state, module);
            }
            FormAction::None
        }

        // Down: cycle options when dropdown is open; navigate to next field otherwise
        KeyCode::Down => {
            if is_select && form_state.dropdown_open {
                if let Some(field) = active_field {
                    let search = if is_dynamic_allow_create {
                        form_state
                            .search_buffers
                            .get(&field.name)
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    cycle_select_filtered(form_state, &field.name, 1, &search);
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
                let new_af = (form_state.active_field + 1) % navigable_count;
                form_state.active_field = new_af;
                form_state.active_config_idx = visible_indices.get(new_af).copied();
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
                if is_dynamic_allow_create && let Some(field) = active_field {
                    let search = form_state
                        .search_buffers
                        .get(&field.name)
                        .cloned()
                        .unwrap_or_default();
                    if !search.is_empty() {
                        // Collect filtered options to decide what Enter does.
                        let filtered: Vec<String> = form_state
                            .field_options
                            .get(&field.name)
                            .map(|opts| {
                                opts.iter()
                                    .filter(|o| o.to_lowercase().contains(&search.to_lowercase()))
                                    .cloned()
                                    .collect()
                            })
                            .unwrap_or_default();
                        if filtered.is_empty() {
                            // No matches — novel value.
                            // Check for create_template: open sub-form overlay
                            if let Some(ref tpl_name) = field.create_template {
                                let term_size = crossterm::terminal::size().unwrap_or((80, 24));
                                if term_size.1 >= 10 && term_size.0 >= 30 {
                                    // module already borrows app.config, so look up template through it
                                    let template = module
                                        .fields
                                        .iter()
                                        .find(|f| f.name == field.name)
                                        .and_then(|f| f.create_template.as_ref())
                                        .and_then(|tn| {
                                            app.config
                                                .templates
                                                .as_ref()
                                                .and_then(|t| t.get(tn.as_str()))
                                        });
                                    if let Some(template) = template {
                                        let fname = field.name.clone();
                                        form_state.dropdown_open = false;
                                        form_state.sub_form =
                                            Some(crate::app::SubFormState::new(
                                                tpl_name.clone(),
                                                search,
                                                fname.clone(),
                                                template,
                                            ));
                                        form_state.search_buffers.remove(&fname);
                                        return FormAction::None;
                                    }
                                }
                            }
                            // Fallback: accept typed text as novel value (bare stub creation)
                            let fname = field.name.clone();
                            form_state.field_values.insert(fname.clone(), search);
                            form_state.search_buffers.remove(&fname);
                            form_state.dropdown_open = false;
                            return FormAction::None;
                        }
                        // Matches exist — select the highlighted one and close.
                        let current = form_state
                            .field_values
                            .get(&field.name)
                            .cloned()
                            .unwrap_or_default();
                        let best = if filtered.contains(&current) {
                            current
                        } else {
                            filtered.into_iter().next().unwrap_or_default()
                        };
                        let fname = field.name.clone();
                        form_state.field_values.insert(fname.clone(), best);
                        form_state.search_buffers.remove(&fname);
                        form_state.dropdown_open = false;
                        return FormAction::None;
                    }
                }
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
                let new_af = (form_state.active_field + 1) % navigable_count;
                form_state.active_field = new_af;
                form_state.active_config_idx = visible_indices.get(new_af).copied();
                form_state.cursor_position = current_value_len(form_state, module);
                FormAction::None
            }
        }

        KeyCode::Char(c) => {
            // For allow_create dynamic_select fields, route typing into the search buffer.
            if is_dynamic_allow_create && let Some(field) = active_field {
                let buf = form_state
                    .search_buffers
                    .entry(field.name.clone())
                    .or_default();
                // Cap search buffer at 100 chars to prevent unbounded growth.
                if buf.len() < 100 {
                    buf.push(c);
                }
                // Auto-open the dropdown so the user sees filtered options.
                form_state.dropdown_open = true;
                return FormAction::None;
            }
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
            // For allow_create dynamic_select, backspace trims the search buffer.
            if is_dynamic_allow_create && let Some(field) = active_field {
                let buf = form_state
                    .search_buffers
                    .entry(field.name.clone())
                    .or_default();
                buf.pop();
                return FormAction::None;
            }
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
            // Cycle callout type backward when textarea is closed
            if is_textarea
                && !form_state.textarea_open
                && let Some(field) = active_field
                && form_state.callout_overrides.contains_key(&field.name)
            {
                let options = crate::app::CALLOUT_OPTIONS;
                let current = &form_state.callout_overrides[&field.name];
                // If current value is not in the list (custom callout), wrap to last option
                let prev = match options.iter().position(|(_, s)| *s == current) {
                    Some(0) => options.len() - 1,
                    Some(idx) => idx - 1,
                    None => options.len() - 1,
                };
                form_state
                    .callout_overrides
                    .insert(field.name.clone(), options[prev].1.to_string());
                return FormAction::None;
            }
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
            // Cycle callout type forward when textarea is closed
            if is_textarea
                && !form_state.textarea_open
                && let Some(field) = active_field
                && form_state.callout_overrides.contains_key(&field.name)
            {
                let options = crate::app::CALLOUT_OPTIONS;
                let current = &form_state.callout_overrides[&field.name];
                // If current value is not in the list (custom callout), wrap to first option
                let next = match options.iter().position(|(_, s)| *s == current) {
                    Some(idx) => (idx + 1) % options.len(),
                    None => 0,
                };
                form_state
                    .callout_overrides
                    .insert(field.name.clone(), options[next].1.to_string());
                return FormAction::None;
            }
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
    /// User submitted the sub-form overlay for template-driven note creation.
    CreateFromTemplate {
        field_name: String,
        template_name: String,
        note_name: String,
        field_values: std::collections::HashMap<String, String>,
    },
}

/// Cycle the selected value within the subset of options matching `search`
/// (case-insensitive substring). When `search` is empty all options are used.
fn cycle_select_filtered(form_state: &mut FormState, field_name: &str, delta: i32, search: &str) {
    let all_options = match form_state.field_options.get(field_name) {
        Some(opts) if !opts.is_empty() => opts,
        _ => return,
    };

    let options: Vec<String> = if search.is_empty() {
        all_options.clone()
    } else {
        all_options
            .iter()
            .filter(|o| o.to_lowercase().contains(&search.to_lowercase()))
            .cloned()
            .collect()
    };

    if options.is_empty() {
        return;
    }

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
///
/// Uses the visible index to resolve the active field, matching the semantics
/// of `form_state.active_field` after TASK-A04.
fn current_value_len(form_state: &FormState, module: &crate::config::ModuleConfig) -> usize {
    active_field_config(form_state, module)
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
/// Handle key events when the sub-form overlay is open.
///
/// All keys are consumed by the sub-form. Tab/Shift+Tab navigate fields,
/// Enter submits or toggles dropdowns, Esc cancels.
fn handle_sub_form_key(
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
    let navigable_count = field_count + 1; // +1 for submit button
    let on_submit_button = sub_form.active_field == field_count;
    let active_tfield = template.fields.get(sub_form.active_field);
    let is_static_select = active_tfield
        .map(|f| f.field_type == TemplateFieldType::StaticSelect)
        .unwrap_or(false);

    // Helper: advance cursor to end of the current field value after navigation
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
        // ── Cancel ───────────────────────────────────────────────────────────
        KeyCode::Esc => {
            form_state.sub_form = None;
            FormAction::None
        }

        // ── Field navigation: Down / Tab ─────────────────────────────────────
        KeyCode::Down | KeyCode::Tab => {
            sub_form.active_field = (sub_form.active_field + 1) % navigable_count;
            sync_cursor(sub_form, template);
            FormAction::None
        }

        // ── Field navigation: Up / BackTab ───────────────────────────────────
        KeyCode::Up | KeyCode::BackTab => {
            sub_form.active_field = if sub_form.active_field == 0 {
                navigable_count - 1
            } else {
                sub_form.active_field - 1
            };
            sync_cursor(sub_form, template);
            FormAction::None
        }

        // ── Submit or advance ─────────────────────────────────────────────────
        KeyCode::Enter => {
            if on_submit_button {
                // Emit the action with all data needed for note creation.
                // Do NOT close the sub-form or set the parent value here —
                // that happens in main.rs after successful transport write.
                // This prevents data loss if the action is dropped or fails.
                return FormAction::CreateFromTemplate {
                    field_name: sub_form.parent_field_name.clone(),
                    template_name: sub_form.template_name.clone(),
                    note_name: sub_form.note_name.clone(),
                    field_values: sub_form.field_values.clone(),
                };
            }
            // Any field: advance to next
            sub_form.active_field = (sub_form.active_field + 1) % navigable_count;
            sync_cursor(sub_form, template);
            FormAction::None
        }

        // ── Left: cycle static_select backward, or move text cursor ──────────
        KeyCode::Left => {
            if is_static_select {
                if let Some(tf) = active_tfield {
                    if let Some(opts) = sub_form.field_options.get(&tf.name) {
                        if !opts.is_empty() {
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
                    }
                }
            } else if sub_form.cursor_position > 0 {
                sub_form.cursor_position -= 1;
            }
            FormAction::None
        }

        // ── Right: cycle static_select forward, or move text cursor ──────────
        KeyCode::Right => {
            if is_static_select {
                if let Some(tf) = active_tfield {
                    if let Some(opts) = sub_form.field_options.get(&tf.name) {
                        if !opts.is_empty() {
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
                    }
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

        // ── Text / number input ───────────────────────────────────────────────
        KeyCode::Char(c) => {
            if on_submit_button || is_static_select {
                return FormAction::None;
            }
            if let Some(tf) = active_tfield {
                // Number fields: only allow digits, decimal, minus
                if tf.field_type == TemplateFieldType::Number
                    && !c.is_ascii_digit()
                    && c != '.'
                    && c != '-'
                {
                    return FormAction::None;
                }
                let value = sub_form.field_values.entry(tf.name.clone()).or_default();
                // cursor_position is a char index — convert to byte offset
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
            if on_submit_button || is_static_select {
                return FormAction::None;
            }
            if let Some(tf) = active_tfield {
                let value = sub_form.field_values.entry(tf.name.clone()).or_default();
                if sub_form.cursor_position > 0 {
                    // Convert char index to byte range for the char to remove
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
