use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::{App, ConfigSetting, ConfigureLevel, ConfigureState, PendingConfirm, SettingKind};

const SCROLL_MARGIN: usize = 2;

/// Adjust `state.scroll_offset` so the cursor stays visible in the edit viewport.
///
/// `term_cols` is the full terminal width in columns. We reconstruct avail from
/// the active setting's label length and kind_hint the same way render does.
fn sync_scroll_offset(state: &mut ConfigureState, term_cols: u16) {
    let setting = match state.settings.get(state.active_field) {
        Some(s) => s,
        None => return,
    };
    let kind_hint_len = match &setting.kind {
        SettingKind::Path => 9,       // " [Browse]"
        SettingKind::Toggle(_) => 8,  // " [toggle]"
        SettingKind::Text => 0,
        SettingKind::NavLink => 2,    // " >"
        SettingKind::ListEditor => 10, // " [Edit list]"
        SettingKind::Identifier => 0,
    };
    let prefix_len = 2 + setting.label.len() + 3;
    let avail = (term_cols as usize).saturating_sub(prefix_len + kind_hint_len);

    if avail == 0 {
        return;
    }

    let cursor = state.cursor_position;

    // Scroll right: cursor too far right
    let scroll_right_edge = state.scroll_offset + avail.saturating_sub(SCROLL_MARGIN + 1);
    if cursor >= scroll_right_edge {
        state.scroll_offset = cursor.saturating_sub(avail.saturating_sub(SCROLL_MARGIN + 1));
    }

    // Scroll left: cursor too far left
    if cursor < state.scroll_offset + SCROLL_MARGIN && state.scroll_offset > 0 {
        state.scroll_offset = cursor.saturating_sub(SCROLL_MARGIN);
    }

    // Never scroll past start
    if state.scroll_offset > cursor {
        state.scroll_offset = 0;
    }
}

/// Actions the configure screen can signal to the wiring layer.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigureAction {
    None,
    Cancel,
    Save,
    /// Request a directory listing for the given vault-relative path.
    BrowseDirectory(String),
    /// Add a new default field to the current module.
    AddField,
    /// Remove the field at the given index (confirmed by user).
    RemoveField(usize),
    /// Swap the two field indices in the current module.
    ReorderFields(usize, usize),
    /// Delete the current module (confirmed by user).
    DeleteModule,
    /// Save the new module being configured to disk (Phase 4c stub).
    SaveNewModule,
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

    // Body: confirmation dialog, browser, list editor, field list, or settings list
    if state.confirm.is_some() {
        // Render the underlying view first, then overlay the confirm dialog
        if state.level == ConfigureLevel::FieldList {
            render_field_list(app, frame, chunks[1]);
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
    } else {
        render_settings(app, frame, chunks[1]);
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
                Span::styled("tokens: ", Style::default().fg(Color::DarkGray)),
                Span::styled("{{date}} {{field}} %Y %m %d", Style::default().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Yellow)),
                Span::raw(" confirm  "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(" cancel"),
            ])
        }
    } else if state.level == ConfigureLevel::FieldList {
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
        Line::from(vec![
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
        ])
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

    let items: Vec<ListItem> = state
        .settings
        .iter()
        .enumerate()
        .map(|(i, setting)| {
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
                let items: Vec<&str> = setting.value.lines().filter(|l| !l.is_empty()).collect();
                if items.is_empty() {
                    String::new()
                } else {
                    items.join(", ")
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
                // Stub — full implementation in Phase 4c.
                SettingKind::Identifier => "",
            };

            // Horizontal scroll viewport when editing this row.
            // prefix = "▸ " (2) + label + ":  " (3)
            let prefix_len = 2usize + setting.label.len() + 3;
            let hint_len = kind_hint.len();
            let avail = (area.width as usize).saturating_sub(prefix_len + hint_len);

            let (value_display, left_clipped, right_clipped) = if is_active && state.editing && avail > 0 {
                let char_count = raw_value.chars().count();
                let scroll = state.scroll_offset;
                // avail already accounts for the indicator chars we'll add
                let view_end = scroll + avail;
                let left = scroll > 0;
                let right = char_count > view_end;
                // Shrink the content window by the number of indicator chars shown
                let content_start = scroll;
                let content_take = avail.saturating_sub(left as usize + right as usize);
                let slice: String = raw_value.chars().skip(content_start).take(content_take).collect();
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
            spans.push(Span::styled(kind_hint, Style::default().fg(Color::DarkGray)));
            let line = Line::from(spans);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);

    // Cursor placement when editing a text/path field
    if state.editing
        && let Some(setting) = state.settings.get(state.active_field)
    {
        // prefix = "▸ " (2) + label + ":  " (3)
        let prefix_len = 2 + setting.label.len() + 3;
        // Offset within the viewport: cursor_position minus scroll, plus 1 if left indicator shown
        let left_indicator: u16 = if state.scroll_offset > 0 { 1 } else { 0 };
        let viewport_col = state.cursor_position.saturating_sub(state.scroll_offset) as u16;
        let cursor_x = area.x + prefix_len as u16 + left_indicator + viewport_col;
        let cursor_y = area.y + state.active_field as u16;
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
    };

    // Center a small box
    let dialog_width = (message.len() as u16 + 6).min(area.width);
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
fn render_field_list(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
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

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);
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
}

/// Handle a key event on the configure screen.
///
/// Returns a `ConfigureAction` that the wiring layer should act on.
pub fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) -> ConfigureAction {
    use crossterm::event::KeyCode;

    let state = match &mut app.configure_state {
        Some(s) => s,
        None => return ConfigureAction::None,
    };

    // --- Confirmation dialog mode ---
    if let Some(ref pending) = state.confirm.clone() {
        match key.code {
            KeyCode::Char('y') => {
                state.confirm = None;
                match pending {
                    PendingConfirm::DeleteField { field_index, .. } => {
                        return ConfigureAction::RemoveField(*field_index);
                    }
                    PendingConfirm::DeleteModule { .. } => {
                        return ConfigureAction::DeleteModule;
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                state.confirm = None;
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- Browser mode ---
    if state.browser_open {
        let browser = match &mut state.browser_state {
            Some(b) => b,
            None => {
                // No state yet; only Esc works
                if key.code == KeyCode::Esc {
                    state.browser_open = false;
                }
                return ConfigureAction::None;
            }
        };

        let at_root =
            browser.current_path.is_empty() || browser.current_path == "/";

        let dirs: Vec<String> = browser
            .entries
            .iter()
            .filter(|e| e.is_dir)
            .map(|e| e.name.clone())
            .collect();

        let total = if at_root { dirs.len() } else { dirs.len() + 1 };

        match key.code {
            KeyCode::Up => {
                if total > 0 && browser.selected > 0 {
                    browser.selected -= 1;
                }
                return ConfigureAction::None;
            }
            KeyCode::Down => {
                if total > 0 && browser.selected + 1 < total {
                    browser.selected += 1;
                }
                return ConfigureAction::None;
            }
            KeyCode::Esc => {
                state.browser_open = false;
                return ConfigureAction::None;
            }
            KeyCode::Backspace => {
                // Go up one level
                let parent = parent_path(&browser.current_path);
                return ConfigureAction::BrowseDirectory(parent);
            }
            KeyCode::Enter => {
                let selected = browser.selected;
                let current_path = browser.current_path.clone();

                if !at_root && selected == 0 {
                    // ".." — go up
                    let parent = parent_path(&current_path);
                    return ConfigureAction::BrowseDirectory(parent);
                }

                let dir_idx = if at_root { selected } else { selected - 1 };
                if let Some(name) = dirs.get(dir_idx) {
                    let new_path = if current_path.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{}", current_path.trim_end_matches('/'), name)
                    };
                    return ConfigureAction::BrowseDirectory(new_path);
                }
                return ConfigureAction::None;
            }
            KeyCode::Tab => {
                // Select current directory as the path value
                let selected = browser.selected;
                let current_path = browser.current_path.clone();
                let at_root_local =
                    current_path.is_empty() || current_path == "/";

                let chosen_dir = if !at_root_local && selected == 0 {
                    // ".." selected → use parent
                    parent_path(&current_path)
                } else {
                    let dir_idx = if at_root_local { selected } else { selected - 1 };
                    if let Some(name) = dirs.get(dir_idx) {
                        if current_path.is_empty() {
                            name.clone()
                        } else {
                            format!(
                                "{}/{}",
                                current_path.trim_end_matches('/'),
                                name
                            )
                        }
                    } else {
                        // Nothing selected — just use current directory
                        current_path
                    }
                };

                // Snapshot the active setting key and configure level before
                // we take a mutable borrow of settings.
                let active_setting_key = state
                    .settings
                    .get(state.active_field)
                    .map(|s| s.key.clone())
                    .unwrap_or_default();
                let level = state.level.clone();
                let module_key = state.module_key.clone();

                // For append-mode module paths, auto-append /{date_format}.md
                // so the browser (which can only select directories) produces a
                // valid file path with a date token.
                let chosen_path = if active_setting_key == "path"
                    && matches!(level, ConfigureLevel::ModuleSettings)
                {
                    let is_append = app
                        .config
                        .modules
                        .get(&module_key)
                        .map(|m| m.mode == crate::config::WriteMode::Append)
                        .unwrap_or(false);

                    let date_fmt = app
                        .config
                        .vault
                        .date_format
                        .as_deref()
                        .unwrap_or("%Y%m%d");

                    if is_append {
                        format!("{}/{}.md", chosen_dir.trim_end_matches('/'), date_fmt)
                    } else {
                        // Create mode: auto-append a date-based filename template
                        // so each entry gets a unique file.
                        format!("{}/{}.md", chosen_dir.trim_end_matches('/'), date_fmt)
                    }
                } else {
                    chosen_dir
                };

                // Apply to the active Path setting and transition to
                // freetext edit so the user can tweak the filename template
                // (e.g. append `{{bean}} {{date}}.md`).
                // Re-borrow configure_state mutably after the immutable app.config
                // access above is complete.
                let is_module_path = active_setting_key == "path"
                    && matches!(level, ConfigureLevel::ModuleSettings);
                if let Some(state) = &mut app.configure_state {
                    if let Some(setting) = state.settings.get_mut(state.active_field) {
                        setting.value = chosen_path.clone();
                        state.dirty = true;
                    }
                    state.browser_open = false;

                    // For module path settings, drop into freetext edit so
                    // the user can append a filename template after browsing.
                    if is_module_path {
                        // Ensure trailing slash so cursor is ready for filename entry
                        let mut path = chosen_path;
                        if !path.ends_with('/') && !path.contains('.') {
                            path.push('/');
                        }
                        state.edit_original = path.clone();
                        state.edit_buffer = path.clone();
                        state.cursor_position = path.chars().count();
                        state.editing = true;
                    }
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- List editor overlay mode ---
    if state.list_editor_open {
        use crossterm::event::KeyModifiers;

        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                // Save: write buffer back to the setting value
                let buf = state.list_editor_buffer.clone();
                if let Some(setting) = state.settings.get_mut(state.active_field) {
                    setting.value = buf;
                    state.dirty = true;
                }
                state.list_editor_open = false;
                state.list_editor_buffer.clear();
                state.list_editor_cursor_line = 0;
                state.list_editor_cursor_col = 0;
                return ConfigureAction::None;
            }
            (_, KeyCode::Esc) => {
                // Cancel — discard changes
                state.list_editor_open = false;
                state.list_editor_buffer.clear();
                state.list_editor_cursor_line = 0;
                state.list_editor_cursor_col = 0;
                return ConfigureAction::None;
            }
            (_, KeyCode::Enter) => {
                // Insert newline at cursor position
                let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                let line_idx = state.list_editor_cursor_line.min(lines.len().saturating_sub(1));
                let col = state.list_editor_cursor_col;

                // Find byte offset for the cursor position
                let mut byte_offset = 0;
                for (i, line) in state.list_editor_buffer.lines().enumerate() {
                    if i == line_idx {
                        byte_offset += col.min(line.len());
                        break;
                    }
                    byte_offset += line.len() + 1; // +1 for '\n'
                }
                // Handle empty buffer or cursor at end
                byte_offset = byte_offset.min(state.list_editor_buffer.len());
                state.list_editor_buffer.insert(byte_offset, '\n');
                state.list_editor_cursor_line += 1;
                state.list_editor_cursor_col = 0;
                return ConfigureAction::None;
            }
            (_, KeyCode::Char(c)) => {
                let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                let line_idx = state.list_editor_cursor_line.min(lines.len().saturating_sub(1));
                let col = state.list_editor_cursor_col;

                let mut byte_offset = 0;
                for (i, line) in state.list_editor_buffer.lines().enumerate() {
                    if i == line_idx {
                        byte_offset += col.min(line.len());
                        break;
                    }
                    byte_offset += line.len() + 1;
                }
                byte_offset = byte_offset.min(state.list_editor_buffer.len());
                state.list_editor_buffer.insert(byte_offset, c);
                state.list_editor_cursor_col += 1;
                return ConfigureAction::None;
            }
            (_, KeyCode::Backspace) => {
                if state.list_editor_cursor_col > 0 {
                    let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                    let line_idx = state.list_editor_cursor_line.min(lines.len().saturating_sub(1));
                    let col = state.list_editor_cursor_col;

                    let mut byte_offset = 0;
                    for (i, line) in state.list_editor_buffer.lines().enumerate() {
                        if i == line_idx {
                            byte_offset += (col - 1).min(line.len());
                            break;
                        }
                        byte_offset += line.len() + 1;
                    }
                    if byte_offset < state.list_editor_buffer.len() {
                        state.list_editor_buffer.remove(byte_offset);
                    }
                    state.list_editor_cursor_col -= 1;
                } else if state.list_editor_cursor_line > 0 {
                    // Merge with previous line
                    let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                    let prev_line_len = lines.get(state.list_editor_cursor_line - 1).map(|l| l.len()).unwrap_or(0);

                    // Find the newline byte offset at end of previous line
                    let mut byte_offset = 0;
                    for (i, line) in state.list_editor_buffer.lines().enumerate() {
                        if i == state.list_editor_cursor_line - 1 {
                            byte_offset += line.len();
                            break;
                        }
                        byte_offset += line.len() + 1;
                    }
                    if byte_offset < state.list_editor_buffer.len() {
                        state.list_editor_buffer.remove(byte_offset); // remove '\n'
                    }
                    state.list_editor_cursor_line -= 1;
                    state.list_editor_cursor_col = prev_line_len;
                }
                return ConfigureAction::None;
            }
            (_, KeyCode::Up) => {
                if state.list_editor_cursor_line > 0 {
                    state.list_editor_cursor_line -= 1;
                    let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                    let line_len = lines.get(state.list_editor_cursor_line).map(|l| l.len()).unwrap_or(0);
                    state.list_editor_cursor_col = state.list_editor_cursor_col.min(line_len);
                }
                return ConfigureAction::None;
            }
            (_, KeyCode::Down) => {
                let line_count = state.list_editor_buffer.lines().count().max(1);
                if state.list_editor_cursor_line + 1 < line_count {
                    state.list_editor_cursor_line += 1;
                    let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                    let line_len = lines.get(state.list_editor_cursor_line).map(|l| l.len()).unwrap_or(0);
                    state.list_editor_cursor_col = state.list_editor_cursor_col.min(line_len);
                }
                return ConfigureAction::None;
            }
            (_, KeyCode::Left) => {
                if state.list_editor_cursor_col > 0 {
                    state.list_editor_cursor_col -= 1;
                }
                return ConfigureAction::None;
            }
            (_, KeyCode::Right) => {
                let lines: Vec<&str> = state.list_editor_buffer.lines().collect();
                let line_len = lines.get(state.list_editor_cursor_line).map(|l| l.len()).unwrap_or(0);
                if state.list_editor_cursor_col < line_len {
                    state.list_editor_cursor_col += 1;
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- Editing mode ---
    if state.editing {
        let term_cols = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
        match key.code {
            KeyCode::Enter => {
                // Confirm edit
                let buf = state.edit_buffer.clone();
                if let Some(setting) = state.settings.get_mut(state.active_field) {
                    setting.value = buf;
                    state.dirty = true;
                }
                state.editing = false;
                state.edit_buffer.clear();
                state.edit_original.clear();
                state.cursor_position = 0;
                state.scroll_offset = 0;
                return ConfigureAction::None;
            }
            KeyCode::Esc => {
                // Cancel edit — restore original
                if let Some(setting) = state.settings.get_mut(state.active_field) {
                    setting.value = state.edit_original.clone();
                }
                state.editing = false;
                state.edit_buffer.clear();
                state.edit_original.clear();
                state.cursor_position = 0;
                state.scroll_offset = 0;
                return ConfigureAction::None;
            }
            KeyCode::Char(c) => {
                // Identifier fields: reject characters that aren't TOML-key-safe
                if matches!(
                    state.settings.get(state.active_field).map(|s| &s.kind),
                    Some(SettingKind::Identifier)
                ) && !(c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return ConfigureAction::None;
                }
                // Use char indices for correct Unicode handling
                let byte_pos = state.edit_buffer.char_indices()
                    .nth(state.cursor_position)
                    .map(|(i, _)| i)
                    .unwrap_or(state.edit_buffer.len());
                state.edit_buffer.insert(byte_pos, c);
                state.cursor_position += 1;
                sync_scroll_offset(state, term_cols);
                return ConfigureAction::None;
            }
            KeyCode::Backspace => {
                if state.cursor_position > 0 {
                    let char_count = state.edit_buffer.chars().count();
                    let pos = state.cursor_position.min(char_count);
                    let byte_pos = state.edit_buffer.char_indices()
                        .nth(pos - 1)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    state.edit_buffer.remove(byte_pos);
                    state.cursor_position = pos - 1;
                    sync_scroll_offset(state, term_cols);
                }
                return ConfigureAction::None;
            }
            KeyCode::Left => {
                if state.cursor_position > 0 {
                    state.cursor_position -= 1;
                    sync_scroll_offset(state, term_cols);
                }
                return ConfigureAction::None;
            }
            KeyCode::Right => {
                let char_count = state.edit_buffer.chars().count();
                if state.cursor_position < char_count {
                    state.cursor_position += 1;
                    sync_scroll_offset(state, term_cols);
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- Field list navigation mode ---
    if state.level == ConfigureLevel::FieldList {
        use crossterm::event::KeyModifiers;

        // Ctrl+Up / Ctrl+Down: reorder fields
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            let field_count = app
                .config
                .modules
                .get(&state.module_key)
                .map(|m| m.fields.len())
                .unwrap_or(0);

            match key.code {
                KeyCode::Up => {
                    // active_field > 1: not on Back row and not first field
                    if state.active_field > 1 {
                        let a = state.active_field - 2; // field index of item above
                        let b = state.active_field - 1; // field index of current
                        state.active_field -= 1;
                        return ConfigureAction::ReorderFields(a, b);
                    }
                    return ConfigureAction::None;
                }
                KeyCode::Down => {
                    // active_field > 0 (not on Back) and not last field
                    if state.active_field > 0 && state.active_field < field_count {
                        let a = state.active_field - 1; // field index of current
                        let b = state.active_field;     // field index of item below
                        state.active_field += 1;
                        return ConfigureAction::ReorderFields(a, b);
                    }
                    return ConfigureAction::None;
                }
                _ => return ConfigureAction::None,
            }
        }

        let field_count = app
            .config
            .modules
            .get(&state.module_key)
            .map(|m| m.fields.len())
            .unwrap_or(0);
        // total items = 1 ("< Back") + field_count
        let total = 1 + field_count;

        match key.code {
            KeyCode::Esc => {
                // Back to module settings
                state.level = ConfigureLevel::ModuleSettings;
                state.active_field = 0;
                ConfigureAction::None
            }
            KeyCode::Up => {
                if state.active_field > 0 {
                    state.active_field -= 1;
                }
                ConfigureAction::None
            }
            KeyCode::Down => {
                if state.active_field + 1 < total {
                    state.active_field += 1;
                }
                ConfigureAction::None
            }
            KeyCode::Enter => {
                if state.active_field == 0 {
                    // "< Back" row
                    state.level = ConfigureLevel::ModuleSettings;
                    state.active_field = 0;
                } else {
                    // Select a field — transition to FieldEditor
                    let field_idx = state.active_field - 1;
                    if let Some(field) = app
                        .config
                        .modules
                        .get(&state.module_key)
                        .and_then(|m| m.fields.get(field_idx))
                    {
                        state.settings = crate::app::App::build_field_settings(field);
                        state.level = ConfigureLevel::FieldEditor(field_idx);
                        state.active_field = 0;
                    }
                }
                ConfigureAction::None
            }
            KeyCode::Char('n') => {
                // Add a new field
                ConfigureAction::AddField
            }
            KeyCode::Char('d') => {
                // Delete selected field (requires confirmation)
                if state.active_field > 0 {
                    let field_idx = state.active_field - 1;
                    let field_name = app
                        .config
                        .modules
                        .get(&state.module_key)
                        .and_then(|m| m.fields.get(field_idx))
                        .map(|f| f.name.clone())
                        .unwrap_or_else(|| "?".to_string());
                    state.confirm = Some(PendingConfirm::DeleteField {
                        field_index: field_idx,
                        field_name,
                    });
                }
                ConfigureAction::None
            }
            _ => ConfigureAction::None,
        }
    } else {
    // --- Settings navigation mode ---
    use crossterm::event::KeyModifiers;

    // Ctrl+S in NewModule level → trigger save of new module
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('s')
        && state.level == ConfigureLevel::NewModule
    {
        // Validate module_key is non-empty and TOML-safe
        let module_key_val = state
            .settings
            .iter()
            .find(|s| s.key == "module_key")
            .map(|s| s.value.clone())
            .unwrap_or_default();

        if module_key_val.is_empty() {
            state.status_message = Some("Module Key must not be empty".to_string());
            return ConfigureAction::None;
        }

        let valid_key = module_key_val
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !valid_key {
            state.status_message =
                Some("Module Key: only a-z, A-Z, 0-9, _ and - are allowed".to_string());
            return ConfigureAction::None;
        }

        state.status_message = None;
        return ConfigureAction::SaveNewModule;
    }

    let setting_count = state.settings.len();

    match key.code {
        KeyCode::Esc => {
            if let ConfigureLevel::FieldEditor(_) = state.level {
                // Back to field list, restore module-level settings
                state.level = ConfigureLevel::FieldList;
                state.active_field = 0;
                // Rebuild module-level settings since we replaced them with field settings
                if let Some(module) = app.config.modules.get(&state.module_key) {
                    let mode_str = match module.mode {
                        crate::config::WriteMode::Append => "append".to_string(),
                        crate::config::WriteMode::Create => "create".to_string(),
                    };
                    let mut settings = vec![
                        ConfigSetting {
                            label: "Path".to_string(),
                            key: "path".to_string(),
                            value: module.path.clone(),
                            kind: SettingKind::Path,
                        },
                        ConfigSetting {
                            label: "Display Name".to_string(),
                            key: "display_name".to_string(),
                            value: module.display_name.clone().unwrap_or_default(),
                            kind: SettingKind::Text,
                        },
                        ConfigSetting {
                            label: "Mode".to_string(),
                            key: "mode".to_string(),
                            value: mode_str.clone(),
                            kind: SettingKind::Toggle(vec!["append".to_string(), "create".to_string()]),
                        },
                    ];
                    if mode_str == "append" {
                        settings.push(ConfigSetting {
                            label: "Append Header".to_string(),
                            key: "append_under_header".to_string(),
                            value: module.append_under_header.clone().unwrap_or_default(),
                            kind: SettingKind::Text,
                        });
                    }
                    let field_count = module.fields.len();
                    settings.push(ConfigSetting {
                        label: "Fields".to_string(),
                        key: "fields".to_string(),
                        value: format!("{field_count} field{}", if field_count == 1 { "" } else { "s" }),
                        kind: SettingKind::NavLink,
                    });
                    state.settings = settings;
                }
                ConfigureAction::None
            } else {
                // ModuleSettings, VaultSettings, and NewModule all return to dashboard
                ConfigureAction::Cancel
            }
        }

        KeyCode::Up => {
            if setting_count > 0 && state.active_field > 0 {
                state.active_field -= 1;
            }
            ConfigureAction::None
        }

        KeyCode::Down => {
            if setting_count > 0 && state.active_field + 1 < setting_count {
                state.active_field += 1;
            }
            ConfigureAction::None
        }

        // 's' saves for module/vault settings but is NOT wired for NewModule
        // (NewModule uses Ctrl+S to avoid confusion with typing 's' in an identifier)
        KeyCode::Char('s') if state.level != ConfigureLevel::NewModule && !state.editing => {
            ConfigureAction::Save
        }

        // 'd' on ModuleSettings: prompt to delete the entire module
        KeyCode::Char('d')
            if state.level == ConfigureLevel::ModuleSettings && !state.editing =>
        {
            let module_key = state.module_key.clone();
            state.confirm = Some(PendingConfirm::DeleteModule { module_key });
            ConfigureAction::None
        }

        KeyCode::Char('e') => {
            // 'e' on any field starts freetext editing (including Path and Identifier)
            if let Some(setting) = state.settings.get(state.active_field) {
                state.edit_original = setting.value.clone();
                state.edit_buffer = setting.value.clone();
                state.cursor_position = setting.value.chars().count();
                state.editing = true;
            }
            ConfigureAction::None
        }

        KeyCode::Enter => {
            if let Some(setting) = state.settings.get(state.active_field) {
                match &setting.kind.clone() {
                    SettingKind::Path => {
                        // Open vault browser at current path's directory
                        let browse_path = dir_of(&setting.value);
                        return ConfigureAction::BrowseDirectory(browse_path);
                    }
                    SettingKind::Text | SettingKind::Identifier => {
                        // Start freetext editing
                        state.edit_original = setting.value.clone();
                        state.edit_buffer = setting.value.clone();
                        state.cursor_position = setting.value.chars().count();
                        state.editing = true;
                    }
                    SettingKind::Toggle(options) => {
                        // Cycle to next option
                        let current = setting.value.clone();
                        let key = setting.key.clone();
                        let idx = options.iter().position(|o| *o == current);
                        let next_idx = match idx {
                            Some(i) => (i + 1) % options.len(),
                            None => 0,
                        };
                        if let Some(next) = options.get(next_idx) {
                            let next = next.clone();
                            state.settings[state.active_field].value = next.clone();
                            state.dirty = true;

                            // Dynamically add/remove append_under_header when mode toggles
                            if key == "mode" {
                                let has_header = state.settings.iter().any(|s| s.key == "append_under_header");
                                if next == "append" && !has_header {
                                    state.settings.push(ConfigSetting {
                                        label: "Append Header".to_string(),
                                        key: "append_under_header".to_string(),
                                        value: "## Log".to_string(),
                                        kind: SettingKind::Text,
                                    });
                                } else if next == "create" && has_header {
                                    state.settings.retain(|s| s.key != "append_under_header");
                                }
                            }

                            // Dynamically add/remove type-specific settings in field editor
                            if key == "field_type" {
                                // Remove existing type-conditional settings
                                state.settings.retain(|s| s.key != "options" && s.key != "source");

                                if next == "static_select" {
                                    state.settings.push(ConfigSetting {
                                        label: "Options".to_string(),
                                        key: "options".to_string(),
                                        value: String::new(),
                                        kind: SettingKind::ListEditor,
                                    });
                                } else if next == "dynamic_select" {
                                    state.settings.push(ConfigSetting {
                                        label: "Source".to_string(),
                                        key: "source".to_string(),
                                        value: String::new(),
                                        kind: SettingKind::Path,
                                    });
                                }
                            }
                        }
                    }
                    SettingKind::NavLink => {
                        // Navigate to the linked sub-screen
                        let key = setting.key.clone();
                        if key == "fields" {
                            state.level = ConfigureLevel::FieldList;
                            state.active_field = 0;
                        }
                    }
                    SettingKind::ListEditor => {
                        // Open the list editor overlay
                        state.list_editor_buffer = setting.value.clone();
                        let line_count = state.list_editor_buffer.lines().count().max(1);
                        let last_line_len = state.list_editor_buffer.lines().last().map(|l| l.len()).unwrap_or(0);
                        state.list_editor_cursor_line = line_count - 1;
                        state.list_editor_cursor_col = last_line_len;
                        state.list_editor_open = true;
                    }
                }
            }
            ConfigureAction::None
        }

        _ => ConfigureAction::None,
    }
    } // end else (settings navigation)
}

/// Compute the parent directory of a vault-relative path.
/// Returns an empty string if already at the root.
fn parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if let Some(pos) = trimmed.rfind('/') {
        trimmed[..pos].to_string()
    } else {
        String::new()
    }
}

/// Get the directory portion of a vault-relative file path.
/// If the path contains no slash, returns an empty string (vault root).
fn dir_of(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if let Some(pos) = trimmed.rfind('/') {
        trimmed[..pos].to_string()
    } else {
        String::new()
    }
}
