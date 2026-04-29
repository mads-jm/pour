use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{
    App, CalloutTitleEdit, FieldPresetPickerState, FormState, PresetDialogFocus,
    PresetDialogTarget, PresetPickerState, PresetSaveDialog, SubFormState,
};
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

    // has_picker is false when axes are empty OR axes failed validation (axis_warnings non-empty).
    let has_picker = !module.preset_axes.is_empty() && form_state.axis_warnings.is_empty();
    render_fields(frame, chunks[1], &module.fields, form_state, has_picker);

    // Footer: validation errors, axis warnings, delete confirmation, or key hints
    let footer_content = if !form_state.validation_errors.is_empty() {
        let error_text = form_state.validation_errors.join("; ");
        Line::from(Span::styled(
            format!(" Error: {error_text}"),
            Style::default().fg(Color::Red),
        ))
    } else if !form_state.axis_warnings.is_empty() {
        let warn_text = form_state.axis_warnings.join(", ");
        Line::from(vec![
            Span::styled(
                format!(" \u{26A0} preset_axes invalid: {warn_text}"),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" — picker disabled", Style::default().fg(Color::DarkGray)),
        ])
    } else if form_state.confirm_delete_preset {
        let name = form_state
            .selected_preset_name
            .clone()
            .unwrap_or_default();
        Line::from(vec![
            Span::styled(
                format!(" Delete \"{name}\"?"),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" (y/n)", Style::default().fg(Color::Yellow)),
        ])
    } else {
        // Contextual hints: "t title" appears when focused on a textarea row
        // that has an active callout and the editor is closed.
        let visible = visible_field_indices(&module.fields, &form_state.field_values);
        let active_field_cfg =
            if form_state.active_field >= 1 && form_state.active_field <= visible.len() {
                visible
                    .get(form_state.active_field - 1)
                    .and_then(|&ci| module.fields.get(ci))
            } else {
                None
            };
        let show_title_hint = !form_state.textarea_open
            && active_field_cfg.is_some_and(|f| {
                f.field_type == FieldType::Textarea
                    && (form_state.callout_overrides.contains_key(&f.name)
                        || f.callout.is_some()
                        || form_state.callout_overrides.contains_key("_callout_type"))
            });

        let mut spans = vec![
            Span::styled(" s", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(" delete  "),
        ];
        if has_picker {
            spans.push(Span::styled("p", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" picker  "));
        } else {
            spans.push(Span::styled("←→", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" cycle  "));
        }
        spans.extend([
            Span::styled("↑↓/Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" interact  "),
        ]);
        if show_title_hint {
            spans.push(Span::styled("t", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" title  "));
        }
        spans.push(Span::styled("Esc", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" clear/back"));
        Line::from(spans)
    };
    let footer = Paragraph::new(footer_content).block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);

    // Preset picker overlay: drilldown tree modal
    if let Some(ref picker) = form_state.preset_picker {
        render_preset_picker_overlay(frame, area, picker, &module.preset_axes);
    }

    // Preset save overlay renders before sub-form overlay
    if let Some(ref overlay) = form_state.preset_overlay {
        render_preset_save_overlay(frame, area, overlay);
    }

    // Callout-title edit overlay.
    if let Some(ref edit) = form_state.callout_title_edit {
        render_callout_title_overlay(frame, area, edit);
    }

    // Sub-form overlay renders LAST so it paints over footer and fields
    if let Some(sub_form) = &form_state.sub_form
        && let Some(templates) = &app.config.templates
        && let Some(template) = templates.get(&sub_form.template_name)
    {
        render_sub_form(frame, area, sub_form, template);
    }
}

/// Render the vertical list of form fields plus a submit button row.
///
/// `active_field` layout:
///   0                      = preset row
///   1..=visible_count      = real fields (visible_indices[active_field - 1])
///   visible_count + 1      = submit button
fn render_fields(frame: &mut Frame, area: Rect, fields: &[FieldConfig], form_state: &FormState, has_picker: bool) {
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
    super::render_overflow_hints(frame, area, item_count, 0);

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
        render_composite_editor(frame, area, field, form_state);

        // Render the per-field preset picker on top of the composite editor.
        if let Some(picker) = &form_state.field_preset_picker {
            render_field_preset_picker(frame, area, picker);
        }
    }
}

/// Render the centered overlay for naming a preset before saving.
/// Render a compact centered modal for editing a textarea field's callout title.
fn render_callout_title_overlay(frame: &mut Frame, area: Rect, edit: &CalloutTitleEdit) {
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
    let title = format!(" Callout Title — {} ", edit.field_name);
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
        ("<title — blank to clear>".to_string(), placeholder_style)
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

/// Handle key events when the callout-title edit overlay is open.
///
/// Enter commits the buffer (trimmed) into `callout_titles`; an empty trimmed
/// value removes the entry so the config default takes over. Esc cancels
/// without saving.
fn handle_callout_title_key(
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

fn render_preset_picker_overlay(
    frame: &mut Frame,
    area: Rect,
    picker: &PresetPickerState,
    axes: &[String],
) {
    use crate::data::preset_tree::TreeNode;

    if area.height < 6 || area.width < 30 {
        return;
    }

    let modal_width = (area.width * 2 / 3).max(40).min(area.width.saturating_sub(4));
    let modal_height = (area.height * 2 / 3).max(8).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    // Build breadcrumb from path.
    let breadcrumb = build_breadcrumb(picker, axes);
    let title = format!(" Preset: {breadcrumb} ");
    frame.render_widget(Clear, modal_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.as_str())
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, modal_area);

    let inner = Rect::new(
        modal_area.x + 1,
        modal_area.y + 1,
        modal_area.width.saturating_sub(2),
        modal_area.height.saturating_sub(2),
    );

    let list_height = inner.height.saturating_sub(1) as usize; // last row is hint

    let nodes = current_nodes(picker);
    let total = nodes.len();

    let items: Vec<ListItem> = nodes
        .iter()
        .skip(picker.viewport_offset)
        .take(list_height)
        .enumerate()
        .map(|(i, node)| {
            let abs_i = i + picker.viewport_offset;
            let is_selected = abs_i == picker.selected;
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };
            match node {
                TreeNode::Branch { axis_value, count, .. } => {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("  {axis_value}"), style),
                        Span::styled(
                            format!("  ({count})"),
                            if is_selected {
                                Style::default().fg(Color::DarkGray).bg(Color::Cyan)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            },
                        ),
                        Span::styled(" ▸", style),
                    ]))
                }
                TreeNode::Leaf { preset_name, description } => {
                    let name_style = style;
                    if let Some(desc) = description {
                        let desc_style = if is_selected {
                            Style::default().fg(Color::DarkGray).bg(Color::Cyan)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC)
                        };
                        ListItem::new(Text::from(vec![
                            Line::from(Span::styled(format!("  {preset_name}"), name_style)),
                            Line::from(Span::styled(format!("    {desc}"), desc_style)),
                        ]))
                    } else {
                        ListItem::new(Line::from(Span::styled(
                            format!("  {preset_name}"),
                            name_style,
                        )))
                    }
                }
            }
        })
        .collect();

    let list_area = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(1));
    frame.render_widget(List::new(items), list_area);

    // Scroll indicator
    if total > list_height {
        let pct = (picker.viewport_offset * 100) / total.max(1);
        let scroll_text = format!(" {}/{} ({}%)", picker.selected + 1, total, pct);
        let indicator_area = Rect::new(
            modal_area.x + modal_area.width.saturating_sub(scroll_text.len() as u16 + 1),
            modal_area.y,
            scroll_text.len() as u16,
            1,
        );
        frame.render_widget(
            Paragraph::new(Span::styled(scroll_text, Style::default().fg(Color::DarkGray))),
            indicator_area,
        );
    }

    // Hint row
    let hint_area = Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(" nav  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" select  "),
            Span::styled("Bksp/←", Style::default().fg(Color::Yellow)),
            Span::raw(" back  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])),
        hint_area,
    );
}

fn build_breadcrumb(picker: &PresetPickerState, axes: &[String]) -> String {
    use crate::data::preset_tree::TreeNode;
    if picker.path.is_empty() {
        return axes.first().cloned().unwrap_or_else(|| "Preset".to_string());
    }
    let mut parts: Vec<String> = Vec::new();
    // Walk through roots_with_ungrouped (which includes the synthetic Ungrouped branch).
    let mut nodes: &[TreeNode] = &picker.tree.roots_with_ungrouped;
    for &idx in picker.path.iter() {
        if let Some(TreeNode::Branch { axis_value, children, .. }) = nodes.get(idx) {
            parts.push(axis_value.clone());
            nodes = children;
        }
    }
    // Append next axis label (not applicable when inside the Ungrouped virtual branch).
    let next_axis = axes.get(picker.path.len()).cloned();
    let mut breadcrumb = parts.join(" \u{25B8} ");
    if let Some(ax) = next_axis {
        if !breadcrumb.is_empty() {
            breadcrumb.push_str(" \u{25B8} ");
        }
        breadcrumb.push_str(&ax);
    }
    breadcrumb
}

fn current_nodes(picker: &PresetPickerState) -> &[crate::data::preset_tree::TreeNode] {
    use crate::data::preset_tree::TreeNode;
    if picker.path.is_empty() {
        // roots_with_ungrouped contains the alphabetical branches followed by a synthetic
        // "Ungrouped (N)" branch when ungrouped presets exist.  Empty if no presets at all.
        return &picker.tree.roots_with_ungrouped;
    }
    let mut nodes: &[TreeNode] = &picker.tree.roots_with_ungrouped;
    for &idx in &picker.path {
        if let Some(TreeNode::Branch { children, .. }) = nodes.get(idx) {
            nodes = children;
        } else {
            return &[];
        }
    }
    nodes
}

fn render_preset_save_overlay(frame: &mut Frame, area: Rect, overlay: &PresetSaveDialog) {
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

    // Name input line
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

    // Description input line
    let desc_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    let (desc_text, desc_style) = if overlay.description_buffer.is_empty() {
        ("<description — optional>".to_string(), placeholder_style)
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

    // Hint line (two rows below inputs leaves a visual gap)
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

    // Place cursor inside the focused input ("Name: " / "Desc: " are both 6 chars)
    let (cursor_row, cursor_col) = match overlay.focus {
        PresetDialogFocus::Name => (inner.y, overlay.cursor_position),
        PresetDialogFocus::Description => (inner.y + 1, overlay.description_cursor),
    };
    let cx = inner.x + 6 + cursor_col as u16;
    if cx < modal_area.x + modal_area.width - 1 {
        frame.set_cursor_position(Position::new(cx, cursor_row));
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
fn render_field_preset_picker(frame: &mut Frame, area: Rect, picker: &FieldPresetPickerState) {
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
    let error_row: u16 = if sub_form.error_message.is_some() {
        1
    } else {
        0
    };
    let modal_height = (field_count as u16 + 5 + error_row).min(area.height.saturating_sub(4)); // fields + title + button + hints + borders + optional error
    let modal_width = (area.width * 3 / 5)
        .max(30)
        .min(area.width.saturating_sub(4));
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
                let truncated: String = label_text.chars().take(label_width as usize - 1).collect();
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

        let (display_val, value_style) = if tfield.field_type == TemplateFieldType::StaticSelect {
            let extensible = tfield.allow_create.unwrap_or(false);
            let inner_val = if value.is_empty() { "select" } else { value };
            // Extensible active fields render as plain text so the cursor has
            // room to land — the ◂ ▸ chevrons imply cycle-only input and
            // mislead users who need to type a novel value.
            let text = if is_active && !extensible {
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

        // Place cursor for active text/number fields, and for extensible
        // static_select fields (allow_create) so the user can see where typing
        // will land.
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

    // Hint line — adapt wording when on an extensible static_select so the user
    // knows they can type a novel value instead of only cycling options.
    let active_tfield = template.fields.get(sub_form.active_field);
    let on_extensible_static = active_tfield.is_some_and(|tf| {
        tf.field_type == TemplateFieldType::StaticSelect && tf.allow_create.unwrap_or(false)
    });
    let hint_y = inner.y + inner.height.saturating_sub(1);
    if hint_y > inner.y {
        let hint_area = Rect::new(inner.x, hint_y, inner.width, 1);
        let mut spans = vec![
            Span::styled(" ↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("←→", Style::default().fg(Color::Yellow)),
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
fn clamp_active_to_visible(form_state: &mut FormState, fields: &[crate::config::FieldConfig]) {
    let visible = visible_field_indices(fields, &form_state.field_values);
    let visible_count = visible.len();
    // active_field layout: 0=preset, 1..=visible_count=fields, visible_count+1=submit
    let submit_idx = visible_count + 1;

    if form_state.active_field == 0 {
        // On preset row — always valid, no config_idx to sync.
        form_state.active_config_idx = None;
        return;
    }

    if form_state.active_field <= submit_idx {
        // Phase 1: active_field is in a valid position.
        // Map real-field slot (1..=visible_count) to visible index (0..visible_count).
        let current_ci = if form_state.active_field <= visible_count {
            visible.get(form_state.active_field - 1).copied()
        } else {
            None // submit button
        };
        form_state.active_config_idx = current_ci;
        return;
    }

    // Phase 2: active_field is out of range — visible set shrank.
    let prev_ci = match form_state.active_config_idx {
        None => {
            // Was on submit or preset — keep on submit (new boundary).
            form_state.active_field = submit_idx;
            return;
        }
        Some(ci) => ci,
    };

    if let Some(new_vi) = visible.iter().position(|&ci| ci == prev_ci) {
        form_state.active_field = new_vi + 1; // +1 for preset row offset
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else if let Some(new_vi) = visible.iter().position(|&ci| ci > prev_ci) {
        form_state.active_field = new_vi + 1;
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else if let Some(new_vi) = visible.iter().rposition(|&ci| ci < prev_ci) {
        form_state.active_field = new_vi + 1;
        form_state.active_config_idx = visible.get(new_vi).copied();
    } else {
        form_state.active_field = submit_idx;
        form_state.active_config_idx = None;
    }
}

/// Resolve the currently active `FieldConfig` using the visible index.
///
/// Returns `None` when the form is on the preset row (active_field == 0) or submit button.
fn active_field_config<'a>(
    form_state: &FormState,
    module: &'a crate::config::ModuleConfig,
) -> Option<&'a crate::config::FieldConfig> {
    if form_state.active_field == 0 {
        return None; // preset row
    }
    let visible = visible_field_indices(&module.fields, &form_state.field_values);
    let vi = form_state.active_field - 1; // convert from visual index to 0-based visible index
    visible.get(vi).and_then(|&ci| module.fields.get(ci))
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
    // Layout: 0=preset row, 1..=visible_count=real fields, visible_count+1=submit
    let visible_indices = visible_field_indices(&module.fields, &form_state.field_values);
    let visible_count = visible_indices.len();
    let navigable_count = visible_count + 2; // +1 preset row, +1 submit button

    let on_preset_row = form_state.active_field == 0;
    let on_submit_button = form_state.active_field == visible_count + 1;
    // Resolve the active FieldConfig through the visible index (offset by 1 for preset row).
    let active_field = if form_state.active_field > 0 && !on_submit_button {
        let vi = form_state.active_field - 1;
        visible_indices
            .get(vi)
            .and_then(|&ci| module.fields.get(ci))
    } else {
        None
    };
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
    // True for static_select fields that opt in to typing novel values.
    // Novel values are appended to the field's options list in-memory and on disk.
    let is_static_allow_create = active_field
        .map(|f| f.field_type == FieldType::StaticSelect && f.allow_create.unwrap_or(false))
        .unwrap_or(false);
    // Union gate: any select-type field that accepts novel typed values.
    let is_select_allow_create = is_dynamic_allow_create || is_static_allow_create;

    // Callout-title edit overlay intercepts ALL keys when open.
    if form_state.callout_title_edit.is_some() {
        return handle_callout_title_key(form_state, key);
    }

    // Preset save overlay intercepts ALL keys when open (before sub-form check)
    if form_state.preset_overlay.is_some() {
        return handle_preset_overlay_key(form_state, &module_key, module, key);
    }

    // Preset picker overlay intercepts ALL keys when open.
    if form_state.preset_picker.is_some() {
        return handle_preset_picker_key(form_state, &module_key, module, &app.presets, key);
    }

    // Delete confirmation intercepts y/n/Esc
    if form_state.confirm_delete_preset {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let name = form_state
                    .selected_preset_name
                    .clone()
                    .unwrap_or_default();
                form_state.confirm_delete_preset = false;
                return FormAction::DeletePreset { name };
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                form_state.confirm_delete_preset = false;
                return FormAction::None;
            }
            _ => return FormAction::None,
        }
    }

    // Sub-form overlay takes priority over all other overlays
    if form_state.sub_form.is_some() {
        return handle_sub_form_key(form_state, &app.config, key);
    }

    // Composite overlay has its own key handling
    if is_composite && form_state.composite_open {
        return handle_composite_key(
            form_state,
            active_field.unwrap(),
            &app.field_presets,
            &module_key,
            key,
        );
    }

    // Open callout-title editor for the currently-active textarea field,
    // provided it has an active callout type (per-field override or module default).
    // Bare `t` is the primary binding — safe here because the textarea editor
    // is closed, so no text input is active. We intentionally avoid Ctrl+T:
    // some IDEs/terminals swallow Ctrl-letter chords before they reach the TUI.
    if matches!(key.code, KeyCode::Char('t') | KeyCode::Char('T'))
        && !form_state.textarea_open
        && let Some(field) = active_field
        && field.field_type == FieldType::Textarea
    {
        let has_callout = form_state.callout_overrides.contains_key(&field.name)
            || (field.callout.is_some()
                && !form_state
                    .callout_overrides
                    .get(&field.name)
                    .is_some_and(|s| s.is_empty()))
            || form_state.callout_overrides.contains_key("_callout_type");
        if has_callout {
            let prefill = form_state
                .callout_titles
                .get(&field.name)
                .cloned()
                .or_else(|| field.callout_title.clone())
                .unwrap_or_default();
            let cursor = prefill.chars().count();
            form_state.callout_title_edit = Some(CalloutTitleEdit {
                field_name: field.name.clone(),
                buffer: prefill,
                cursor,
            });
            return FormAction::None;
        }
    }

    // Save preset: bare 's' on preset/submit row, or Ctrl+S from any non-editing context.
    // Bare 's' is safe on preset/submit rows because no text input is active there.
    // Ctrl+S covers the case where the user is on a field row but not inside an overlay.
    if key.code == KeyCode::Char('s')
        && !form_state.textarea_open
        && !form_state.composite_open
        && ((on_preset_row || on_submit_button)
            || key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL))
    {
        let (prefill_name, prefill_desc, name_was_user_edited) =
            if let Some(ref sel_name) = form_state.selected_preset_name.clone() {
                // Editing an existing preset: prefill its current name + desc.
                let idx = form_state.preset_names.iter().position(|n| n == sel_name);
                let desc = idx
                    .and_then(|i| form_state.preset_descriptions.get(i))
                    .and_then(|d| d.clone())
                    .unwrap_or_default();
                (sel_name.clone(), desc, true)
            } else {
                // New preset: auto-suggest from axes if available.
                let suggested = if !module.preset_axes.is_empty() {
                    crate::data::preset_tree::suggest_preset_name(
                        &form_state.field_values,
                        &module.preset_axes,
                    )
                } else {
                    String::new()
                };
                (suggested, String::new(), false)
            };
        let cursor_position = prefill_name.chars().count();
        let description_cursor = prefill_desc.chars().count();
        form_state.preset_overlay = Some(PresetSaveDialog {
            name_buffer: prefill_name,
            cursor_position,
            description_buffer: prefill_desc,
            description_cursor,
            focus: PresetDialogFocus::Name,
            target: PresetDialogTarget::Module,
            name_was_user_edited,
            awaiting_overwrite_confirm: false,
        });
        return FormAction::None;
    }

    // Delete preset: bare 'd' or Ctrl+D on preset row with a real preset selected.
    if key.code == KeyCode::Char('d') && on_preset_row && form_state.selected_preset_name.is_some() {
        form_state.confirm_delete_preset = true;
        return FormAction::None;
    }

    // Open preset picker: bare 'p' on the preset row, or Ctrl+P from any non-editing context.
    // Bare 'p' only fires when on the preset row AND picker is configured.
    // Ctrl+P fires from any non-editing context when picker is configured.
    // Disabled when axis_warnings is non-empty (invalid axes → cycler resumes instead).
    let picker_trigger = !module.preset_axes.is_empty()
        && form_state.axis_warnings.is_empty()
        && !form_state.textarea_open
        && !form_state.composite_open
        && (key.code == KeyCode::Char('p')
            && (on_preset_row
                || key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)));
    if picker_trigger {
        let presets = app.presets.get(&module_key);
        let tree =
            crate::data::preset_tree::build(&presets, &module.preset_axes);
        form_state.preset_picker = Some(PresetPickerState {
            tree,
            path: Vec::new(),
            selected: 0,
            viewport_offset: 0,
        });
        return FormAction::None;
    }

    // Preset row navigation: Left/Right cycle; Ctrl+Left/Right reorder
    if on_preset_row {
        let preset_count = form_state.preset_names.len();
        // names: [None, "A", "B", ...] — None means <none>, index into preset_names is i-1
        let total = preset_count + 1;
        // axes_empty: treat as empty when axes failed validation (warnings present) so cycler stays active.
        let axes_empty = module.preset_axes.is_empty() || !form_state.axis_warnings.is_empty();

        // Helper: current numeric index (0 = <none>)
        let current_idx = form_state
            .selected_preset_name
            .as_ref()
            .and_then(|n| form_state.preset_names.iter().position(|p| p == n))
            .map(|i| i + 1)
            .unwrap_or(0);

        match key.code {
            KeyCode::Left => {
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                {
                    // Ctrl+Left: reorder backward (always available)
                    if let Some(ref name) = form_state.selected_preset_name.clone() {
                        return FormAction::ReorderPreset {
                            name: name.clone(),
                            direction: -1,
                        };
                    }
                } else if axes_empty {
                    // Left: cycle backward (only when no picker)
                    if total > 0 {
                        let new_idx = (current_idx + total - 1) % total;
                        let new_name = if new_idx > 0 {
                            form_state.preset_names.get(new_idx - 1).cloned()
                        } else {
                            None
                        };
                        let preset_entry = new_name.as_ref().and_then(|n| {
                            app.presets
                                .get(&module_key)
                                .into_iter()
                                .find(|p| p.name == *n)
                        });
                        form_state.selected_preset_name = new_name;
                        App::apply_preset(form_state, &module.fields, preset_entry.as_ref());
                        form_state.active_field = 0;
                        form_state.active_config_idx = None;
                    }
                }
                return FormAction::None;
            }
            KeyCode::Right => {
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                {
                    // Ctrl+Right: reorder forward (always available)
                    if let Some(ref name) = form_state.selected_preset_name.clone() {
                        return FormAction::ReorderPreset {
                            name: name.clone(),
                            direction: 1,
                        };
                    }
                } else if axes_empty {
                    // Right: cycle forward (only when no picker)
                    if total > 0 {
                        let new_idx = (current_idx + 1) % total;
                        let new_name = if new_idx > 0 {
                            form_state.preset_names.get(new_idx - 1).cloned()
                        } else {
                            None
                        };
                        let preset_entry = new_name.as_ref().and_then(|n| {
                            app.presets
                                .get(&module_key)
                                .into_iter()
                                .find(|p| p.name == *n)
                        });
                        form_state.selected_preset_name = new_name;
                        App::apply_preset(form_state, &module.fields, preset_entry.as_ref());
                        form_state.active_field = 0;
                        form_state.active_config_idx = None;
                    }
                }
                return FormAction::None;
            }
            KeyCode::Up => {
                // Navigate away from preset row — wrap to submit
                form_state.active_field = visible_count + 1;
                form_state.active_config_idx = None;
                form_state.cursor_position = 0;
                return FormAction::None;
            }
            KeyCode::Down | KeyCode::Tab => {
                // Navigate to first real field
                form_state.active_field = if visible_count > 0 {
                    1
                } else {
                    visible_count + 1
                };
                form_state.active_config_idx = visible_indices.first().copied();
                form_state.cursor_position = current_value_len(form_state, module);
                return FormAction::None;
            }
            KeyCode::BackTab => {
                // Shift+Tab from preset row: go to submit
                form_state.active_field = visible_count + 1;
                form_state.active_config_idx = None;
                form_state.cursor_position = 0;
                return FormAction::None;
            }
            KeyCode::Esc => {
                return FormAction::Cancel;
            }
            _ => return FormAction::None,
        }
    }

    match key.code {
        // Esc (layered):
        //   1. overlay open (dropdown/textarea) → close it
        //   2. current field has content → clear it
        //   3. field already empty → cancel form (back to dashboard)
        KeyCode::Esc => {
            // If the search buffer has content, clear it first (without closing dropdown).
            if is_select_allow_create
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
            // active_field 0=preset (no config), 1..=vc=fields, vc+1=submit
            form_state.active_config_idx = if new_af > 0 && new_af <= visible_count {
                visible_indices.get(new_af - 1).copied()
            } else {
                None
            };
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
            form_state.active_config_idx = if new_af > 0 && new_af <= visible_count {
                visible_indices.get(new_af - 1).copied()
            } else {
                None
            };
            form_state.cursor_position = current_value_len(form_state, module);
            FormAction::None
        }

        // Up: cycle options when dropdown is open; navigate to previous field otherwise
        KeyCode::Up => {
            if is_select && form_state.dropdown_open {
                if let Some(field) = active_field {
                    let search = if is_select_allow_create {
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
                form_state.active_config_idx = if new_af > 0 && new_af <= visible_count {
                    visible_indices.get(new_af - 1).copied()
                } else {
                    None
                };
                form_state.cursor_position = current_value_len(form_state, module);
            }
            FormAction::None
        }

        // Down: cycle options when dropdown is open; navigate to next field otherwise
        KeyCode::Down => {
            if is_select && form_state.dropdown_open {
                if let Some(field) = active_field {
                    let search = if is_select_allow_create {
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
                form_state.active_config_idx = if new_af > 0 && new_af <= visible_count {
                    visible_indices.get(new_af - 1).copied()
                } else {
                    None
                };
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
                if is_select_allow_create && let Some(field) = active_field {
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
                                        form_state.sub_form = Some(crate::app::SubFormState::new(
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
                            // For static_select, append the novel option to
                            // both the in-memory options list and persist to
                            // disk so it's available next session.
                            if is_static_allow_create {
                                let fname = field.name.clone();
                                if let Some(opts) = form_state.field_options.get_mut(&fname)
                                    && !opts.iter().any(|o| o == &search)
                                {
                                    opts.push(search.clone());
                                }
                                let field_index = form_state.active_config_idx;
                                form_state
                                    .field_values
                                    .insert(fname.clone(), search.clone());
                                form_state.search_buffers.remove(&fname);
                                form_state.dropdown_open = false;
                                if let Some(idx) = field_index {
                                    return FormAction::AppendStaticOption {
                                        field_index: idx,
                                        value: search,
                                    };
                                }
                                return FormAction::None;
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
                form_state.active_config_idx = if new_af > 0 && new_af <= visible_count {
                    visible_indices.get(new_af - 1).copied()
                } else {
                    None
                };
                form_state.cursor_position = current_value_len(form_state, module);
                FormAction::None
            }
        }

        KeyCode::Char(c) => {
            // For allow_create select fields (static or dynamic), route typing into the search buffer.
            if is_select_allow_create && let Some(field) = active_field {
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
                        .cloned()
                        .unwrap_or_default();
                    let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
                    let avail = term_cols.saturating_sub(8).min(60).saturating_sub(2);
                    sync_textarea_scroll(form_state, &value_snap, avail);
                }
            }
            FormAction::None
        }

        KeyCode::Backspace => {
            // For allow_create select fields (static or dynamic), backspace trims the search buffer.
            if is_select_allow_create && let Some(field) = active_field {
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
                        .cloned()
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
            if is_textarea
                && !form_state.textarea_open
                && let Some(field) = active_field
                && !form_state.callout_overrides.contains_key(&field.name)
                && form_state.callout_overrides.contains_key("_callout_type")
            {
                let options = crate::app::CALLOUT_OPTIONS;
                let current = &form_state.callout_overrides["_callout_type"];
                let prev = match options.iter().position(|(_, s)| *s == current) {
                    Some(0) => options.len() - 1,
                    Some(idx) => idx - 1,
                    None => options.len() - 1,
                };
                form_state
                    .callout_overrides
                    .insert("_callout_type".to_string(), options[prev].1.to_string());
                return FormAction::None;
            }
            // Cycle select fields backward when dropdown is closed
            if is_select && !form_state.dropdown_open {
                if let Some(field) = active_field
                    && let Some(opts) = form_state.field_options.get(&field.name).cloned()
                    && !opts.is_empty()
                {
                    let current = form_state
                        .field_values
                        .get(&field.name)
                        .cloned()
                        .unwrap_or_default();
                    let idx = opts.iter().position(|o| o == &current).unwrap_or(0);
                    let new_idx = if idx == 0 { opts.len() - 1 } else { idx - 1 };
                    form_state
                        .field_values
                        .insert(field.name.clone(), opts[new_idx].clone());
                }
                return FormAction::None;
            }
            if form_state.cursor_position > 0 {
                form_state.cursor_position -= 1;
            }
            if is_textarea
                && form_state.textarea_open
                && let Some(field) = active_field
            {
                let value_snap = form_state
                    .field_values
                    .get(&field.name)
                    .cloned()
                    .unwrap_or_default();
                let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
                let avail = term_cols.saturating_sub(8).min(60).saturating_sub(2);
                sync_textarea_scroll(form_state, &value_snap, avail);
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
            if is_textarea
                && !form_state.textarea_open
                && let Some(field) = active_field
                && !form_state.callout_overrides.contains_key(&field.name)
                && form_state.callout_overrides.contains_key("_callout_type")
            {
                let options = crate::app::CALLOUT_OPTIONS;
                let current = &form_state.callout_overrides["_callout_type"];
                let next = match options.iter().position(|(_, s)| *s == current) {
                    Some(idx) => (idx + 1) % options.len(),
                    None => 0,
                };
                form_state
                    .callout_overrides
                    .insert("_callout_type".to_string(), options[next].1.to_string());
                return FormAction::None;
            }
            // Cycle select fields forward when dropdown is closed
            if is_select && !form_state.dropdown_open {
                if let Some(field) = active_field
                    && let Some(opts) = form_state.field_options.get(&field.name).cloned()
                    && !opts.is_empty()
                {
                    let current = form_state
                        .field_values
                        .get(&field.name)
                        .cloned()
                        .unwrap_or_default();
                    let idx = opts.iter().position(|o| o == &current).unwrap_or(0);
                    let new_idx = (idx + 1) % opts.len();
                    form_state
                        .field_values
                        .insert(field.name.clone(), opts[new_idx].clone());
                }
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
                        .cloned()
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
    /// Save a preset with the given name and field values for the current module.
    SavePreset {
        name: String,
        description: Option<String>,
        values: std::collections::HashMap<String, String>,
    },
    /// Delete the preset with the given name for the current module.
    DeletePreset {
        name: String,
    },
    /// Reorder the preset with the given name by `direction` (+1 or -1).
    ReorderPreset {
        name: String,
        direction: i32,
    },
    /// Append a novel option to a static_select field's `options` list,
    /// persisting the change to config.toml. The field value has already
    /// been set in-memory; this action only handles persistence.
    AppendStaticOption {
        field_index: usize,
        value: String,
    },
    /// Save the current rows of a composite_array field as a named preset.
    SaveFieldPreset {
        field_name: String,
        name: String,
        description: Option<String>,
        rows: Vec<Vec<String>>,
    },
    /// Apply (replace rows with) a saved preset for a composite_array field.
    ApplyFieldPreset {
        field_name: String,
        preset_name: String,
    },
    /// Quick-cycle to the next/previous saved preset for a composite_array field.
    /// Direction is +1 (next) or -1 (previous). No-op if no presets are saved.
    CycleFieldPreset {
        field_name: String,
        direction: i32,
    },
    /// Delete a saved preset for a composite_array field by name.
    DeleteFieldPreset {
        field_name: String,
        preset_name: String,
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

/// Handle key events while the preset picker drilldown is open.
///
/// All keys consumed. ↑↓ navigate, Enter drill/apply, Backspace/Left pop, Esc cancel.
fn handle_preset_picker_key(
    form_state: &mut FormState,
    module_key: &str,
    module: &crate::config::ModuleConfig,
    presets: &crate::data::presets::Presets,
    key: crossterm::event::KeyEvent,
) -> FormAction {
    use crate::data::preset_tree::TreeNode;
    use crossterm::event::KeyCode;

    let picker = match &mut form_state.preset_picker {
        Some(p) => p,
        None => return FormAction::None,
    };

    let nodes_len = current_nodes(picker).len();

    match key.code {
        KeyCode::Esc => {
            form_state.preset_picker = None;
            FormAction::None
        }
        KeyCode::Up => {
            if picker.selected > 0 {
                picker.selected -= 1;
                if picker.selected < picker.viewport_offset {
                    picker.viewport_offset = picker.selected;
                }
            }
            FormAction::None
        }
        KeyCode::Down => {
            if picker.selected + 1 < nodes_len {
                picker.selected += 1;
                // keep selected visible (lazy: no fixed window height here — clamped at render)
                if picker.selected >= picker.viewport_offset + 20 {
                    picker.viewport_offset = picker.selected.saturating_sub(19);
                }
            }
            FormAction::None
        }
        KeyCode::Enter => {
            // Clone the node at selected to avoid borrow issues.
            let node = current_nodes(picker).get(picker.selected).cloned();
            match node {
                Some(TreeNode::Branch { .. }) => {
                    // Drill in: push current selected index onto path.
                    let selected = picker.selected;
                    let picker = form_state.preset_picker.as_mut().unwrap();
                    picker.path.push(selected);
                    picker.selected = 0;
                    picker.viewport_offset = 0;
                    FormAction::None
                }
                Some(TreeNode::Leaf { preset_name, .. }) => {
                    // Apply the preset and close the picker.
                    let preset_entry = presets
                        .get(module_key)
                        .into_iter()
                        .find(|p| p.name == preset_name);
                    form_state.selected_preset_name = Some(preset_name);
                    form_state.preset_picker = None;
                    App::apply_preset(form_state, &module.fields, preset_entry.as_ref());
                    form_state.active_field = 0;
                    form_state.active_config_idx = None;
                    FormAction::None
                }
                None => FormAction::None,
            }
        }
        KeyCode::Backspace | KeyCode::Left => {
            let picker = form_state.preset_picker.as_mut().unwrap();
            if picker.path.is_empty() {
                // At root — close picker.
                form_state.preset_picker = None;
            } else {
                // Pop one level.
                let prev_selected = picker.path.pop().unwrap_or(0);
                picker.selected = prev_selected;
                picker.viewport_offset = prev_selected.saturating_sub(10);
            }
            FormAction::None
        }
        _ => FormAction::None,
    }
}

/// Handle key events while the preset save overlay is open.
///
/// All keys are consumed. Enter saves, Esc cancels, text keys edit the name buffer.
fn handle_preset_overlay_key(
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

    // Borrow helpers: operate on (buffer, cursor) for the focused input.
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
            // Overwrite confirm: if the name collides with an existing preset
            // AND this is a new preset (not editing the same name), gate with
            // a single-confirm step before committing.
            if matches!(&overlay.target, PresetDialogTarget::Module) {
                let is_collision = form_state.preset_names.contains(&name);
                // Allow through if editing the preset that already has this name.
                let editing_same = form_state
                    .selected_preset_name
                    .as_deref()
                    .map(|n| n == name)
                    .unwrap_or(false);
                if is_collision && !editing_same && !overlay.awaiting_overwrite_confirm {
                    overlay.awaiting_overwrite_confirm = true;
                    return FormAction::None;
                }
                // Reset confirm flag after passing through.
                overlay.awaiting_overwrite_confirm = false;
            }
            // Branch on dialog target — composite-field saves use the rows of
            // the named field; module-level saves collect a flat field-value
            // map from visible non-excluded fields.
            match &overlay.target {
                PresetDialogTarget::CompositeField { field_name } => {
                    let field_name = field_name.clone();
                    // Strip rows that have no data; reject if everything is empty.
                    let rows: Vec<Vec<String>> = form_state
                        .composite_values
                        .get(&field_name)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|r| r.iter().any(|c| !c.is_empty()))
                        .collect();
                    if rows.is_empty() {
                        // Defensive — the open path already guards this.
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
            // Any edit to the name field — including deletion — marks it as user-edited.
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
                // Typing changes the name — reset any pending confirm.
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
    let is_static_select_extensible =
        is_static_select && active_tfield.and_then(|f| f.allow_create).unwrap_or(false);

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

        // ── Right: cycle static_select forward, or move text cursor ──────────
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

        // ── Text / number input ───────────────────────────────────────────────
        KeyCode::Char(c) => {
            if on_submit_button || (is_static_select && !is_static_select_extensible) {
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
                // Extensible static_select: if the current value is one of the
                // existing options (i.e. came from cycling or initial default),
                // clear it on first keystroke so the user types a fresh novel
                // value instead of appending to "Ethiopia" → "EthiopiaH".
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
            if on_submit_button || (is_static_select && !is_static_select_extensible) {
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

/// Handle key events while the per-field preset picker is open.
///
/// Up/Down navigate the list, Enter applies the selected preset (replaces
/// rows silently), Ctrl+D deletes it, Esc cancels.
fn handle_field_preset_picker_key(
    form_state: &mut FormState,
    field_name: &str,
    key: crossterm::event::KeyEvent,
) -> FormAction {
    use crossterm::event::{KeyCode, KeyModifiers};

    let picker = match &mut form_state.field_preset_picker {
        Some(p) => p,
        None => return FormAction::None,
    };
    let count = picker.names.len();

    match key.code {
        KeyCode::Esc => {
            form_state.field_preset_picker = None;
            FormAction::None
        }
        KeyCode::Up => {
            if count > 0 {
                picker.selected = (picker.selected + count - 1) % count;
            }
            FormAction::None
        }
        KeyCode::Down => {
            if count > 0 {
                picker.selected = (picker.selected + 1) % count;
            }
            FormAction::None
        }
        KeyCode::Enter => {
            if count == 0 {
                return FormAction::None;
            }
            let preset_name = picker.names[picker.selected].clone();
            form_state.field_preset_picker = None;
            FormAction::ApplyFieldPreset {
                field_name: field_name.to_string(),
                preset_name,
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            if count == 0 {
                return FormAction::None;
            }
            let preset_name = picker.names[picker.selected].clone();
            FormAction::DeleteFieldPreset {
                field_name: field_name.to_string(),
                preset_name,
            }
        }
        _ => FormAction::None,
    }
}

fn handle_composite_key(
    form_state: &mut FormState,
    field: &FieldConfig,
    field_presets: &crate::data::field_presets::FieldPresets,
    module_key: &str,
    key: crossterm::event::KeyEvent,
) -> FormAction {
    use crossterm::event::KeyCode;

    let sub_fields = match &field.sub_fields {
        Some(subs) if !subs.is_empty() => subs,
        _ => return FormAction::None,
    };
    let col_count = sub_fields.len();
    let field_name = field.name.clone();

    // Picker overlay intercepts ALL keys when open.
    if form_state.field_preset_picker.is_some() {
        return handle_field_preset_picker_key(form_state, &field_name, key);
    }

    let preset_storage_key = crate::data::field_presets::preset_key(module_key, &field_name);

    // Snapshot navigation state to avoid borrow issues
    let row = form_state.composite_row;
    let col = form_state.composite_col;

    // ── per-field preset bindings ──────────────────────────────────────────
    // `s` save: open the preset save dialog with CompositeField target.
    // Empty editor → status message, no dialog.
    if key.code == KeyCode::Char('s') {
        let rows = form_state
            .composite_values
            .get(&field_name)
            .cloned()
            .unwrap_or_default();
        let has_data = rows.iter().any(|r| r.iter().any(|c| !c.is_empty()));
        if !has_data {
            form_state.composite_status = Some("nothing to save".to_string());
            return FormAction::None;
        }
        let prefill_name = form_state
            .last_applied_field_preset
            .get(&field_name)
            .cloned()
            .unwrap_or_default();
        let cursor_position = prefill_name.chars().count();
        form_state.composite_status = None;
        form_state.preset_overlay = Some(PresetSaveDialog {
            name_buffer: prefill_name,
            cursor_position,
            description_buffer: String::new(),
            description_cursor: 0,
            focus: PresetDialogFocus::Name,
            target: PresetDialogTarget::CompositeField {
                field_name: field_name.clone(),
            },
            name_was_user_edited: false,
            awaiting_overwrite_confirm: false,
        });
        return FormAction::None;
    }

    // `l` open load picker, populating names from the saved preset list.
    if key.code == KeyCode::Char('l') {
        form_state.composite_status = None;
        let entries = field_presets.get(&preset_storage_key);
        if entries.is_empty() {
            form_state.composite_status = Some("no presets saved for this field".to_string());
            return FormAction::None;
        }
        let names: Vec<String> = entries.iter().map(|p| p.name.clone()).collect();
        let descriptions: Vec<Option<String>> =
            entries.iter().map(|p| p.description.clone()).collect();
        // Pre-select the most recently applied preset if any.
        let selected = form_state
            .last_applied_field_preset
            .get(&field_name)
            .and_then(|cur| names.iter().position(|n| n == cur))
            .unwrap_or(0);
        form_state.field_preset_picker = Some(FieldPresetPickerState {
            field_name: field_name.clone(),
            names,
            descriptions,
            selected,
        });
        return FormAction::None;
    }

    // `p` quick-cycle: pick the next preset in the saved list and apply it.
    if key.code == KeyCode::Char('p') {
        form_state.composite_status = None;
        let entries = field_presets.get(&preset_storage_key);
        if entries.is_empty() {
            form_state.composite_status = Some("no presets saved for this field".to_string());
            return FormAction::None;
        }
        let names: Vec<String> = entries.iter().map(|p| p.name.clone()).collect();
        let cur_idx = form_state
            .last_applied_field_preset
            .get(&field_name)
            .and_then(|cur| names.iter().position(|n| n == cur));
        let next_idx = match cur_idx {
            Some(i) => (i + 1) % names.len(),
            None => 0,
        };
        return FormAction::ApplyFieldPreset {
            field_name,
            preset_name: names[next_idx].clone(),
        };
    }

    match key.code {
        KeyCode::Esc => {
            form_state.composite_open = false;
            form_state.composite_status = None;
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
