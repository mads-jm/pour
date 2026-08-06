// LINTOK: oversized: render code is repetitive but cohesive; render-tier file kept whole
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, ConfigureLevel, PendingConfirm, SettingKind};

/// Return a centered `Rect` of the given width and height within `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

/// Preview a path template by expanding only date/time tokens and strftime
/// specifiers, leaving `{{field}}` placeholders visible so the user can see
/// which parts are dynamic.
fn preview_path_template(template: &str, date_format: Option<&str>) -> String {
    let now = chrono::Local::now();
    let mut result = template.to_string();
    let date_fmt = date_format.unwrap_or("%Y%m%d");
    result = result.replace("{{date}}", &now.format(date_fmt).to_string());
    result = result.replace("{{time}}", &now.format("%H:%M").to_string());
    // Temporarily replace {{…}} placeholders so strftime doesn't mangle them.
    let mut placeholders: Vec<String> = Vec::new();
    while let Some(start) = result.find("{{") {
        if let Some(end) = result[start..].find("}}") {
            let token = result[start..start + end + 2].to_string();
            let marker = format!("\x00PH{}\x00", placeholders.len());
            result = result.replacen(&token, &marker, 1);
            placeholders.push(token);
        } else {
            break;
        }
    }
    // Use write! to catch fmt::Error from invalid partial strftime specifiers
    // (e.g. a trailing `%` mid-edit). Fall back to the un-expanded string.
    use std::fmt::Write;
    let mut buf = String::new();
    if write!(buf, "{}", now.format(&result)).is_ok() {
        result = buf;
    }
    // else: leave result as-is with unexpanded specifiers
    // Restore {{…}} placeholders.
    for (i, token) in placeholders.iter().enumerate() {
        let marker = format!("\x00PH{}\x00", i);
        result = result.replace(&marker, token);
    }
    result
}

/// Render the path placeholder help overlay.
pub(super) fn render_path_help_overlay(app: &App, frame: &mut Frame, area: Rect) {
    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };

    let key_style = Style::default().fg(Color::Yellow);
    let desc_style = Style::default().fg(Color::White);
    let dim = Style::default().fg(Color::DarkGray);
    let section = Style::default().fg(Color::Cyan);

    let mut lines: Vec<Line> = vec![Line::from("")];

    // Module-specific field placeholders first (most contextual).
    let fields: Vec<_> = app
        .config
        .modules
        .get(&state.module_key)
        .map(|m| m.fields.iter().collect())
        .unwrap_or_default();

    if !fields.is_empty() {
        lines.push(Line::from(Span::styled("  Fields", section)));
        for f in &fields {
            let placeholder = format!("  {{{{{}}}}}  ", f.name);
            let padded = format!("{:14}", placeholder);
            lines.push(Line::from(vec![
                Span::styled(padded, key_style),
                Span::styled(&f.prompt, desc_style),
            ]));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled("  Tokens", section)));
    lines.push(Line::from(vec![
        Span::styled("  {{date}}    ", key_style),
        Span::styled("current date (vault format)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  {{time}}    ", key_style),
        Span::styled("current time HH:MM", desc_style),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  Strftime", section)));
    lines.push(Line::from(vec![
        Span::styled("  %Y %m %d   ", key_style),
        Span::styled("year / month / day", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  %H %M      ", key_style),
        Span::styled("hour / minute", desc_style),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ?/Esc       ", dim),
        Span::styled("close", dim),
    ]));

    let total_content = lines.len();
    let overlay_height = (total_content as u16 + 2).min(area.height.saturating_sub(2));
    let overlay_width = 44u16.min(area.width);
    let overlay_area = centered_rect(overlay_width, overlay_height, area);

    frame.render_widget(Clear, overlay_area);

    let overlay = Paragraph::new(lines).block(
        Block::default()
            .title(" Path Placeholders ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(overlay, overlay_area);
    let inner = Rect {
        x: overlay_area.x + 1,
        y: overlay_area.y + 1,
        width: overlay_area.width.saturating_sub(2),
        height: overlay_area.height.saturating_sub(2),
    };
    super::super::render_overflow_hints(frame, inner, total_content, 0);
}

/// Render the quick-select overlay for callout type (or any QuickSelect field).
pub(super) fn render_quick_select_overlay(
    frame: &mut Frame,
    area: Rect,
    options: &[(char, String)],
    current_value: &str,
) {
    let key_style = Style::default().fg(Color::Yellow);
    let selected_style = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(Color::White);
    let dim = Style::default().fg(Color::DarkGray);

    let mut lines: Vec<Line> = vec![Line::from("")];

    // Render options in two columns
    let half = options.len().div_ceil(2);

    for i in 0..half {
        let mut spans = Vec::new();

        // Left column
        let (key, label) = &options[i];
        let is_selected = label == current_value;
        let style = if is_selected {
            selected_style
        } else {
            normal_style
        };
        spans.push(Span::styled(format!("  [{key}] "), key_style));
        spans.push(Span::styled(format!("{:12}", label), style));

        // Right column (if exists)
        if let Some((key2, label2)) = options.get(i + half) {
            let is_selected2 = label2 == current_value;
            let style2 = if is_selected2 {
                selected_style
            } else {
                normal_style
            };
            spans.push(Span::styled(format!("[{key2}] "), key_style));
            spans.push(Span::styled(label2.to_string(), style2));
        }

        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    if current_value.is_empty() {
        lines.push(Line::from(Span::styled("  (none selected)", dim)));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  Current: ", dim),
            Span::styled(current_value, selected_style),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Backspace ", dim),
        Span::styled("clear  ", dim),
        Span::styled("Esc ", dim),
        Span::styled("cancel", dim),
    ]));

    let total_content = lines.len();
    let overlay_height = (total_content as u16 + 2).min(area.height.saturating_sub(2));
    let overlay_width = 42u16.min(area.width);
    let overlay_area = centered_rect(overlay_width, overlay_height, area);

    frame.render_widget(Clear, overlay_area);

    let overlay = Paragraph::new(lines).block(
        Block::default()
            .title(" Callout Type ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(overlay, overlay_area);
}

/// Render the configure screen.
pub fn render(app: &App, frame: &mut Frame) {
    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };

    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(1),    // body
            Constraint::Length(3), // footer
        ])
        .split(area);

    // Header
    let header_title = match &state.level {
        ConfigureLevel::ModuleSettings => format!(" ▽ configure {} ", state.module_key),
        ConfigureLevel::FieldList => format!(" ▽ configure {} — fields ", state.module_key),
        ConfigureLevel::FieldEditor(idx) => {
            let field_name = app
                .config
                .modules
                .get(&state.module_key)
                .and_then(|m| m.fields.get(*idx))
                .map(|f| f.name.as_str())
                .unwrap_or("?");
            format!(" ▽ configure {} — {} ", state.module_key, field_name)
        }
        ConfigureLevel::SubFieldList(field_idx) => {
            let field_name = app
                .config
                .modules
                .get(&state.module_key)
                .and_then(|m| m.fields.get(*field_idx))
                .map(|f| f.name.as_str())
                .unwrap_or("?");
            format!(
                " ▽ configure {} — {} — columns ",
                state.module_key, field_name
            )
        }
        ConfigureLevel::SubFieldEditor(field_idx, _sub_idx) => {
            let field_name = app
                .config
                .modules
                .get(&state.module_key)
                .and_then(|m| m.fields.get(*field_idx))
                .map(|f| f.name.as_str())
                .unwrap_or("?");
            format!(
                " ▽ configure {} — {} — edit column ",
                state.module_key, field_name
            )
        }
        ConfigureLevel::VaultSettings => " ▽ vault settings ".to_string(),
        // Stub — full implementation in Phase 4c.
        ConfigureLevel::NewModule => " ▽ new module ".to_string(),
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            header_title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        if state.dirty {
            Span::styled("[modified]", Style::default().fg(Color::Yellow))
        } else {
            Span::raw("")
        },
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Body: confirmation dialog, browser, list editor, field list, sub-field list, or settings list
    if state.confirm.is_some() {
        // Render the underlying view first, then overlay the confirm dialog
        if state.level == ConfigureLevel::FieldList {
            render_field_list(app, frame, chunks[1]);
        } else if matches!(state.level, ConfigureLevel::SubFieldList(_)) {
            render_sub_field_list(app, frame, chunks[1]);
        } else {
            render_settings(app, frame, chunks[1]);
        }
        render_confirm_dialog(app, frame, chunks[1]);
    } else if state.browser_open {
        render_browser(app, frame, chunks[1]);
    } else if state.list_editor_open {
        render_list_editor(app, frame, chunks[1]);
    } else if state.level == ConfigureLevel::FieldList {
        render_field_list(app, frame, chunks[1]);
    } else if matches!(state.level, ConfigureLevel::SubFieldList(_)) {
        render_sub_field_list(app, frame, chunks[1]);
    } else {
        render_settings(app, frame, chunks[1]);
    }

    // Path help overlay (on top of body)
    if state.help_overlay_open {
        render_path_help_overlay(app, frame, chunks[1]);
    }

    // Quick-select overlay (on top of body)
    if state.quick_select_open
        && let Some(setting) = state.settings.get(state.active_field)
        && let SettingKind::QuickSelect(ref options) = setting.kind
    {
        render_quick_select_overlay(frame, chunks[1], options, &setting.value);
    }

    // Footer
    let footer_line = if state.confirm.is_some() {
        Line::from(vec![
            Span::styled(" y", Style::default().fg(Color::Yellow)),
            Span::raw(" confirm  "),
            Span::styled("n/Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])
    } else if let Some(ref msg) = state.status_message {
        Line::from(Span::styled(
            format!(" {msg}"),
            Style::default().fg(Color::Red),
        ))
    } else if state.browser_open {
        Line::from(vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" open  "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" select dir  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])
    } else if state.list_editor_open {
        Line::from(vec![
            Span::styled(" Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" new line  "),
            Span::styled("Ctrl+S", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])
    } else if state.quick_select_open {
        Line::from(vec![
            Span::styled(" key", Style::default().fg(Color::Yellow)),
            Span::raw(" select  "),
            Span::styled("Backspace", Style::default().fg(Color::Yellow)),
            Span::raw(" clear  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])
    } else if state.editing {
        let is_path = state
            .settings
            .get(state.active_field)
            .map(|s| matches!(s.kind, SettingKind::Path))
            .unwrap_or(false);
        if is_path {
            Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Yellow)),
                Span::raw(" confirm  "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(" cancel  "),
                Span::styled("?", Style::default().fg(Color::Yellow)),
                Span::raw(" placeholders"),
            ])
        } else {
            Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Yellow)),
                Span::raw(" confirm  "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(" cancel"),
            ])
        }
    } else if state.level == ConfigureLevel::FieldList
        || matches!(state.level, ConfigureLevel::SubFieldList(_))
    {
        Line::from(vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" open  "),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw(" new  "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(" delete  "),
            Span::styled("Ctrl+↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(" reorder  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" back"),
        ])
    } else if matches!(state.level, ConfigureLevel::SubFieldEditor(_, _)) {
        Line::from(vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" back to columns"),
        ])
    } else if matches!(state.level, ConfigureLevel::FieldEditor(_)) {
        Line::from(vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" back to fields"),
        ])
    } else if state.level == ConfigureLevel::VaultSettings {
        Line::from(vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" back to dashboard"),
        ])
    } else if state.level == ConfigureLevel::NewModule {
        Line::from(vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("Ctrl+S", Style::default().fg(Color::Yellow)),
            Span::raw(" create  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel  "),
            Span::styled("(a-z, 0-9, _, -)", Style::default().fg(Color::DarkGray)),
            Span::raw(" key format"),
        ])
    } else {
        let on_path = state
            .settings
            .get(state.active_field)
            .map(|s| matches!(s.kind, SettingKind::Path))
            .unwrap_or(false);
        let mut spans = vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(" delete  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" back"),
        ];
        if on_path {
            spans.push(Span::raw("  "));
            spans.push(Span::styled("?", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" placeholders"));
        }
        Line::from(spans)
    };

    let footer = Paragraph::new(footer_line).block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);
}

/// Render the settings list.
fn render_settings(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };

    let date_format = app.config.vault.date_format.as_deref();

    let mut items: Vec<ListItem> = Vec::new();
    // Map each setting index to its visual row in the list (accounting for
    // preview lines that occupy extra rows).
    let mut visual_row_for: Vec<usize> = Vec::new();
    let mut visual_row: usize = 0;

    for (i, setting) in state.settings.iter().enumerate() {
        visual_row_for.push(visual_row);

        let is_active = i == state.active_field;

        let label_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let indicator = if is_active { "▸" } else { " " };

        // When editing this field, show the edit buffer instead of the stored value
        let raw_value = if is_active && state.editing {
            state.edit_buffer.clone()
        } else if matches!(setting.kind, SettingKind::ListEditor) {
            // Show comma-separated summary for list values
            let list_items: Vec<&str> = setting.value.lines().filter(|l| !l.is_empty()).collect();
            if list_items.is_empty() {
                String::new()
            } else {
                list_items.join(", ")
            }
        } else {
            setting.value.clone()
        };

        // Suffix for path fields
        let kind_hint = match &setting.kind {
            SettingKind::Path => " [Browse]",
            SettingKind::Toggle(_) => " [toggle]",
            SettingKind::Text => "",
            SettingKind::NavLink => " >",
            SettingKind::ListEditor => " [Edit list]",
            SettingKind::Identifier => "",
            SettingKind::QuickSelect(_) => " [select]",
        };

        // Horizontal scroll viewport when editing this row.
        // prefix = "▸ " (2) + label + ":  " (3)
        let prefix_len = 2usize + UnicodeWidthStr::width(setting.label.as_str()) + 3;
        let hint_len = UnicodeWidthStr::width(kind_hint);
        let avail = (area.width as usize).saturating_sub(prefix_len + hint_len);

        let (value_display, left_clipped, right_clipped) =
            if is_active && state.editing && avail > 0 {
                let char_count = raw_value.chars().count();
                let scroll = state.scroll_offset;
                let view_end = scroll + avail;
                let left = scroll > 0;
                let right = char_count > view_end;
                let content_start = scroll;
                let content_take = avail.saturating_sub(left as usize + right as usize);
                let slice: String = raw_value
                    .chars()
                    .skip(content_start)
                    .take(content_take)
                    .collect();
                (slice, left, right)
            } else {
                (raw_value.clone(), false, false)
            };

        let display_text = if !is_active || !state.editing {
            if value_display.is_empty() {
                "<empty>".to_string()
            } else {
                value_display.clone()
            }
        } else {
            value_display.clone()
        };

        let mut value_spans: Vec<Span> = Vec::new();
        if left_clipped {
            value_spans.push(Span::styled("◂", Style::default().fg(Color::DarkGray)));
        }
        value_spans.push(Span::styled(
            if display_text.is_empty() && !(is_active && state.editing) {
                "<empty>".to_string()
            } else {
                display_text
            },
            if is_active {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            },
        ));
        if right_clipped {
            value_spans.push(Span::styled("▸", Style::default().fg(Color::DarkGray)));
        }

        let mut spans = vec![
            Span::styled(format!("{indicator} "), label_style),
            Span::styled(format!("{}:  ", setting.label), label_style),
        ];
        spans.extend(value_spans);
        spans.push(Span::styled(
            kind_hint,
            Style::default().fg(Color::DarkGray),
        ));

        items.push(ListItem::new(Line::from(spans)));
        visual_row += 1;

        // Path preview line: show the resolved template below the path value.
        if matches!(setting.kind, SettingKind::Path) && !raw_value.is_empty() {
            let preview = preview_path_template(&raw_value, date_format);
            if preview != raw_value {
                let preview_line = Line::from(Span::styled(
                    format!("     → {preview}"),
                    Style::default().fg(Color::DarkGray),
                ));
                items.push(ListItem::new(preview_line));
                visual_row += 1;
            }
        }
    }

    let total_visual_rows = visual_row;
    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);
    super::super::render_overflow_hints(frame, area, total_visual_rows, 0);

    // Cursor placement when editing a text/path field
    if state.editing
        && let Some(setting) = state.settings.get(state.active_field)
    {
        // prefix = "▸ " (2) + label + ":  " (3)
        let prefix_len = 2 + UnicodeWidthStr::width(setting.label.as_str()) + 3;
        // Offset within the viewport: cursor_position minus scroll, plus 1 if left indicator shown
        let left_indicator: u16 = if state.scroll_offset > 0 { 1 } else { 0 };
        let viewport_col = state.cursor_position.saturating_sub(state.scroll_offset) as u16;
        let cursor_x = area.x + prefix_len as u16 + left_indicator + viewport_col;
        let cursor_y = area.y
            + visual_row_for
                .get(state.active_field)
                .copied()
                .unwrap_or(state.active_field) as u16;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position(Position::new(cursor_x, cursor_y));
        }
    }
}

/// Render a centered confirmation dialog overlay.
fn render_confirm_dialog(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    use ratatui::widgets::Clear;

    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };
    let confirm = match &state.confirm {
        Some(c) => c,
        None => return,
    };

    let message = match confirm {
        PendingConfirm::DeleteField { field_name, .. } => {
            format!("Delete field '{field_name}'?")
        }
        PendingConfirm::DeleteModule { module_key } => {
            format!("Delete module '{module_key}'?")
        }
        PendingConfirm::DeleteSubField { sub_field_name, .. } => {
            format!("Delete column '{sub_field_name}'?")
        }
    };

    // Center a small box
    let dialog_width = (UnicodeWidthStr::width(message.as_str()) as u16 + 6).min(area.width);
    let dialog_height = 3_u16;
    let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = ratatui::layout::Rect::new(x, y, dialog_width, dialog_height);

    frame.render_widget(Clear, dialog_area);
    let dialog = Paragraph::new(Line::from(Span::styled(
        format!(" {message} "),
        Style::default().fg(Color::Yellow),
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)),
    );
    frame.render_widget(dialog, dialog_area);
}

/// Render the list editor overlay (one item per line, multiline text editor).
fn render_list_editor(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    use ratatui::widgets::Clear;

    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };

    let label = state
        .settings
        .get(state.active_field)
        .map(|s| s.label.as_str())
        .unwrap_or("List");

    let title = format!(" {label} (one per line) ");

    // Clear the area and draw the editor
    frame.render_widget(Clear, area);

    let text = Paragraph::new(state.list_editor_buffer.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(text, area);

    // Place cursor
    let inner = Block::default().borders(Borders::ALL).inner(area);
    let cursor_x = inner.x + state.list_editor_cursor_col as u16;
    let cursor_y = inner.y + state.list_editor_cursor_line as u16;
    if cursor_x < inner.x + inner.width && cursor_y < inner.y + inner.height {
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}

/// Render the field list for the current module.
pub(super) fn render_field_list(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };

    let module = match app.config.modules.get(&state.module_key) {
        Some(m) => m,
        None => return,
    };

    let mut items: Vec<ListItem> = Vec::with_capacity(module.fields.len() + 1);

    // "< Back" row at index 0
    let back_active = state.active_field == 0;
    let back_style = if back_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let back_ind = if back_active { "▸" } else { " " };
    items.push(ListItem::new(Line::from(Span::styled(
        format!("{back_ind} ‹ Back to settings"),
        back_style,
    ))));

    // One row per field
    for (i, field) in module.fields.iter().enumerate() {
        let idx = i + 1; // offset by 1 for "< Back"
        let is_active = state.active_field == idx;

        let label_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let indicator = if is_active { "▸" } else { " " };

        let type_str = match field.field_type {
            crate::config::FieldType::Text => "text",
            crate::config::FieldType::Textarea => "textarea",
            crate::config::FieldType::Number => "number",
            crate::config::FieldType::StaticSelect => "static_select",
            crate::config::FieldType::DynamicSelect => "dynamic_select",
            crate::config::FieldType::CompositeArray => "composite_array",
            crate::config::FieldType::Toggle => "toggle",
            crate::config::FieldType::Counter => "counter",
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{indicator} "), label_style),
            Span::styled(&field.name, label_style),
            Span::styled(
                format!("  ({type_str})"),
                Style::default().fg(Color::DarkGray),
            ),
        ])));
    }

    if module.fields.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  (no fields)",
            Style::default().fg(Color::DarkGray),
        ))));
    }

    let item_count = items.len();
    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);
    super::super::render_overflow_hints(frame, area, item_count, 0);
}

/// Render the sub-field list for a composite_array field.
pub(super) fn render_sub_field_list(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };

    let field_idx = match state.level {
        ConfigureLevel::SubFieldList(idx) => idx,
        _ => return,
    };

    let sub_fields = app
        .config
        .modules
        .get(&state.module_key)
        .and_then(|m| m.fields.get(field_idx))
        .and_then(|f| f.sub_fields.as_ref());

    let mut items: Vec<ListItem> = Vec::new();

    // "< Back" row at index 0
    let back_active = state.active_field == 0;
    let back_style = if back_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let back_ind = if back_active { "▸" } else { " " };
    items.push(ListItem::new(Line::from(Span::styled(
        format!("{back_ind} ‹ Back to field"),
        back_style,
    ))));

    // One row per sub-field
    if let Some(subs) = sub_fields {
        for (i, sf) in subs.iter().enumerate() {
            let idx = i + 1; // offset by 1 for "< Back"
            let is_active = state.active_field == idx;

            let label_style = if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let indicator = if is_active { "▸" } else { " " };

            let type_str = match sf.field_type {
                crate::config::SubFieldType::Text => "text",
                crate::config::SubFieldType::Number => "number",
                crate::config::SubFieldType::StaticSelect => "static_select",
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("{indicator} "), label_style),
                Span::styled(&sf.name, label_style),
                Span::styled(
                    format!("  ({type_str})"),
                    Style::default().fg(Color::DarkGray),
                ),
            ])));
        }

        if subs.is_empty() {
            items.push(ListItem::new(Line::from(Span::styled(
                "  (no columns)",
                Style::default().fg(Color::DarkGray),
            ))));
        }
    } else {
        items.push(ListItem::new(Line::from(Span::styled(
            "  (no columns)",
            Style::default().fg(Color::DarkGray),
        ))));
    }

    let item_count = items.len();
    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);
    super::super::render_overflow_hints(frame, area, item_count, 0);
}

/// Render the vault browser overlay.
fn render_browser(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let state = match &app.configure_state {
        Some(s) => s,
        None => return,
    };
    let browser = match &state.browser_state {
        Some(b) => b,
        None => {
            // Browser open but state not yet populated — show loading
            let loading = Paragraph::new(Line::from(Span::styled(
                " loading...",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(loading, area);
            return;
        }
    };

    // Surface a listing error (if any) above the entry list
    if let Some(msg) = browser.error.as_deref() {
        let (err_area, list_area) = {
            let err_h = area.height.min(3);
            (
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: err_h,
                },
                Rect {
                    x: area.x,
                    y: area.y + err_h,
                    width: area.width,
                    height: area.height.saturating_sub(err_h),
                },
            )
        };
        let err = Paragraph::new(Line::from(Span::styled(
            format!(" ! {msg}"),
            Style::default().fg(Color::Red),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(" error "),
        );
        frame.render_widget(err, err_area);
        render_browser_list(browser, frame, list_area);
        return;
    }

    render_browser_list(browser, frame, area);
}

fn render_browser_list(
    browser: &crate::app::BrowserState,
    frame: &mut Frame,
    area: ratatui::layout::Rect,
) {
    // Build entry list: ".." first (unless at root/empty), then dirs only
    let at_root = browser.current_path.is_empty() || browser.current_path == "/";

    let dirs: Vec<&str> = browser
        .entries
        .iter()
        .filter(|e| e.is_dir)
        .map(|e| e.name.as_str())
        .collect();

    let total_entries = if at_root { dirs.len() } else { dirs.len() + 1 };

    let items: Vec<ListItem> = {
        let mut v = Vec::with_capacity(total_entries);

        // ".." entry
        if !at_root {
            let is_sel = browser.selected == 0;
            let style = if is_sel {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let ind = if is_sel { "▸ " } else { "  " };
            v.push(ListItem::new(Line::from(Span::styled(
                format!("{ind}.."),
                style,
            ))));
        }

        // Directory entries
        let offset = if at_root { 0 } else { 1 };
        for (i, name) in dirs.iter().enumerate() {
            let idx = i + offset;
            let is_sel = browser.selected == idx;
            let style = if is_sel {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let ind = if is_sel { "▸ " } else { "  " };
            v.push(ListItem::new(Line::from(Span::styled(
                format!("{ind}{name}/"),
                style,
            ))));
        }

        if v.is_empty() {
            v.push(ListItem::new(Line::from(Span::styled(
                "  (no subdirectories)",
                Style::default().fg(Color::DarkGray),
            ))));
        }

        v
    };

    let title = format!(" browse: {} ", browser.current_path);
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .highlight_style(Style::default()); // selection styling is already inline
    let mut list_state = ListState::default().with_selected(Some(browser.selected));
    frame.render_stateful_widget(list, area, &mut list_state);
    // Inner area excludes borders (1 on each side)
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    // ListState scrolls internally; estimate offset from selection.
    let visible = inner.height as usize;
    let scroll = browser.selected.saturating_sub(visible.saturating_sub(1));
    super::super::render_overflow_hints(frame, inner, total_entries, scroll);
}
