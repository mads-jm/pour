use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::app::FormState;
use crate::config::{FieldConfig, FieldType};
use crate::visibility::visible_field_indices;

/// Render the vertical list of form fields plus a submit button row.
///
/// `active_field` layout:
///   0                      = preset row
///   1..=visible_count      = real fields (visible_indices[active_field - 1])
///   visible_count + 1      = submit button
pub(super) fn render_fields(
    frame: &mut Frame,
    area: Rect,
    fields: &[FieldConfig],
    form_state: &FormState,
    has_picker: bool,
) {
    // Compute which fields are currently visible given the form's current values.
    // `vi` (visible index) is the render position; `ci` (config index) is the field's
    // position in the original `fields` slice.
    let visible_indices = visible_field_indices(fields, &form_state.field_values);
    let visible_count = visible_indices.len();

    let on_preset_row = form_state.active_field == 0;
    let submit_active = form_state.active_field == visible_count + 1;

    // --- Preset row (always at position 0) ---
    let preset_label_style = if on_preset_row {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let preset_value_style = if on_preset_row {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::Gray)
    };
    let preset_name_display = form_state
        .selected_preset_name
        .clone()
        .unwrap_or_else(|| "<none>".to_string());
    let preset_value_text = if on_preset_row && !has_picker {
        // Legacy cycler: show chevrons when cycling is active.
        format!("◂ {preset_name_display} ▸")
    } else {
        preset_name_display
    };
    let preset_indicator = if on_preset_row { "▸" } else { " " };
    let preset_title_line = Line::from(vec![
        Span::styled(format!("{preset_indicator} "), preset_label_style),
        Span::styled("Preset: ", preset_label_style),
        Span::styled(preset_value_text, preset_value_style),
    ]);

    // Description subtitle: shown only when a real preset is selected and it
    // has a non-empty description. Rendered dim, indented under the preset name.
    let preset_description = if let Some(ref name) = form_state.selected_preset_name {
        form_state
            .preset_names
            .iter()
            .position(|n| n == name)
            .and_then(|i| form_state.preset_descriptions.get(i))
            .and_then(|d| d.clone())
    } else {
        None
    };

    let preset_item = if let Some(desc) = preset_description {
        let subtitle_style = if on_preset_row {
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC)
        } else {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC)
        };
        // Indent to align under the preset value (past "▸ Preset: ").
        let subtitle_line = Line::from(vec![
            Span::raw("          "),
            Span::styled(desc, subtitle_style),
        ]);
        ListItem::new(Text::from(vec![preset_title_line, subtitle_line]))
    } else {
        ListItem::new(preset_title_line)
    };

    let mut items: Vec<ListItem> = vec![preset_item];

    // --- Real fields (offset: visible index vi maps to active_field vi+1) ---
    items.extend(visible_indices.iter().enumerate().map(|(vi, &ci)| {
        let field = &fields[ci];
        // active_field == vi+1 because preset row occupies slot 0
        let is_active = (vi + 1) == form_state.active_field;
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
                        format!("◂ {display_text} ▸ [v]")
                    }
                } else {
                    display_text
                }
            }
            FieldType::Textarea => {
                // Two display modes:
                // - With active callout: dedicated header line `[!type] title`,
                //   content preview on a second line.
                // - Without callout: single line with content preview.
                let callout_type = form_state
                    .callout_overrides
                    .get(&field.name)
                    .cloned()
                    .or_else(|| field.callout.clone())
                    .or_else(|| form_state.callout_overrides.get("_callout_type").cloned());
                let content_preview = if value.is_empty() {
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
                if let Some(c) = callout_type {
                    let title = form_state
                        .callout_titles
                        .get(&field.name)
                        .cloned()
                        .or_else(|| field.callout_title.clone())
                        .unwrap_or_default();
                    let header = if title.trim().is_empty() {
                        format!("[!{c}]")
                    } else {
                        format!("[!{c}] {title}")
                    };
                    let header_with_chevron = if is_active {
                        if form_state.textarea_open {
                            format!("{header} [^]")
                        } else {
                            format!("{header} [v]")
                        }
                    } else {
                        header
                    };
                    // Marker used below to split header vs body across two lines.
                    format!("{header_with_chevron}\n{content_preview}")
                } else {
                    let label = content_preview;
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

        let icon_prefix = field
            .icon
            .as_deref()
            .map(|i| format!("{i} "))
            .unwrap_or_default();

        // Multi-line value_display (used by textarea + callout) renders across
        // two rows: header on the prompt line, body preview on a second row
        // indented to align under the value column.
        if let Some((header, body)) = value_display.split_once('\n') {
            let indent_width = 2 + icon_prefix.chars().count() + field.prompt.chars().count() + 3;
            let indent: String = " ".repeat(indent_width);
            let header_line = Line::from(vec![
                Span::styled(format!("{indicator} "), prompt_style),
                Span::styled(
                    format!("{icon_prefix}{}{}: ", field.prompt, required_marker),
                    prompt_style,
                ),
                Span::styled(header.to_string(), value_style),
            ]);
            let body_style = Style::default().fg(Color::DarkGray);
            let body_line = Line::from(vec![
                Span::raw(indent),
                Span::styled(body.to_string(), body_style),
            ]);
            ListItem::new(Text::from(vec![header_line, body_line]))
        } else {
            let line = Line::from(vec![
                Span::styled(format!("{indicator} "), prompt_style),
                Span::styled(
                    format!("{icon_prefix}{}{}: ", field.prompt, required_marker),
                    prompt_style,
                ),
                Span::styled(value_display, value_style),
            ]);
            ListItem::new(line)
        }
    }));

    // Submit button row (now at visual index visible_count + 1, because preset row is at 0)
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
    crate::tui::render_overflow_hints(frame, area, item_count, 0);

    // Resolve the active field's config-index from the visible list.
    // active_field == 0 => preset row (no real field config)
    // active_field == vi+1 => visible_indices[vi]
    let active_config_field = if form_state.active_field > 0 && !submit_active {
        visible_indices
            .get(form_state.active_field - 1)
            .and_then(|&ci| fields.get(ci))
    } else {
        None
    };

    // Place the terminal block cursor for text/textarea/number fields
    if !submit_active
        && !on_preset_row
        && let Some(field) = active_config_field
    {
        let is_text_input = matches!(field.field_type, FieldType::Text | FieldType::Number);
        if is_text_input {
            // prefix: "▸ " (2 cols) + prompt (display width) + required_marker (1) + ": " (2)
            let prefix_len = 2 + UnicodeWidthStr::width(field.prompt.as_str()) + 1 + 2;
            let cursor_x = area.x + prefix_len as u16 + form_state.cursor_position as u16;
            // active_field is the visual row index (preset row at 0, fields at 1+).
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
        super::composite::render_composite_editor(frame, area, field, form_state);

        // Render the per-field preset picker on top of the composite editor.
        if let Some(picker) = &form_state.field_preset_picker {
            super::composite::render_field_preset_picker(frame, area, picker);
        }
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
                let match_pos = opt.char_indices().find_map(|(byte_idx, _)| {
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
    crate::tui::render_overflow_hints(frame, inner, options.len(), scroll);
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
