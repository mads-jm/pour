use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::{App, ConfigSetting, SettingKind};

/// Actions the configure screen can signal to the wiring layer.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigureAction {
    None,
    Cancel,
    Save,
    /// Request a directory listing for the given vault-relative path.
    BrowseDirectory(String),
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
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" ▽ configure {} ", state.module_key),
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

    // Body: browser or settings list
    if state.browser_open {
        render_browser(app, frame, chunks[1]);
    } else {
        render_settings(app, frame, chunks[1]);
    }

    // Footer
    let footer_line = if let Some(ref msg) = state.status_message {
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
    } else if state.editing {
        Line::from(vec![
            Span::styled(" Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" confirm  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
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

            let indicator = if is_active { ">" } else { " " };

            // When editing this field, show the edit buffer instead of the stored value
            let value_display = if is_active && state.editing {
                state.edit_buffer.clone()
            } else {
                setting.value.clone()
            };

            // Suffix for path fields
            let kind_hint = match &setting.kind {
                SettingKind::Path => " [Browse]",
                SettingKind::Toggle(_) => " [toggle]",
                SettingKind::Text => "",
            };

            let line = Line::from(vec![
                Span::styled(format!("{indicator} "), label_style),
                Span::styled(format!("{}:  ", setting.label), label_style),
                Span::styled(
                    if value_display.is_empty() {
                        "<empty>".to_string()
                    } else {
                        value_display
                    },
                    if is_active {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
                Span::styled(
                    kind_hint,
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, area);

    // Cursor placement when editing a text/path field
    if state.editing
        && let Some(setting) = state.settings.get(state.active_field)
    {
        // prefix = "> " (2) + label + ":  " (3)
        let prefix_len = 2 + setting.label.len() + 3;
        let cursor_x = area.x + prefix_len as u16 + state.cursor_position as u16;
        let cursor_y = area.y + state.active_field as u16;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position(Position::new(cursor_x, cursor_y));
        }
    }
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
            let ind = if is_sel { "> " } else { "  " };
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
            let ind = if is_sel { "> " } else { "  " };
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

                let chosen_path = if !at_root_local && selected == 0 {
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

                // Apply to the Path setting
                if let Some(setting) = state.settings.iter_mut().find(|s| s.key == "path") {
                    setting.value = chosen_path;
                    state.dirty = true;
                }
                state.browser_open = false;
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- Editing mode ---
    if state.editing {
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
                return ConfigureAction::None;
            }
            KeyCode::Char(c) => {
                // Use char indices for correct Unicode handling
                let byte_pos = state.edit_buffer.char_indices()
                    .nth(state.cursor_position)
                    .map(|(i, _)| i)
                    .unwrap_or(state.edit_buffer.len());
                state.edit_buffer.insert(byte_pos, c);
                state.cursor_position += 1;
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
                }
                return ConfigureAction::None;
            }
            KeyCode::Left => {
                if state.cursor_position > 0 {
                    state.cursor_position -= 1;
                }
                return ConfigureAction::None;
            }
            KeyCode::Right => {
                let char_count = state.edit_buffer.chars().count();
                if state.cursor_position < char_count {
                    state.cursor_position += 1;
                }
                return ConfigureAction::None;
            }
            _ => return ConfigureAction::None,
        }
    }

    // --- Settings navigation mode ---
    let setting_count = state.settings.len();

    match key.code {
        KeyCode::Esc => ConfigureAction::Cancel,

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

        KeyCode::Char('s') => ConfigureAction::Save,

        KeyCode::Char('e') => {
            // 'e' on any field starts freetext editing (including Path)
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
                    SettingKind::Text => {
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
                        }
                    }
                }
            }
            ConfigureAction::None
        }

        _ => ConfigureAction::None,
    }
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
