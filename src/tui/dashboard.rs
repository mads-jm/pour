use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;

/// Render the dashboard view: header with connection status, module list, footer.
pub fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(1),    // module list
            Constraint::Length(3), // footer
        ])
        .split(area);

    // Header: title + connection status
    let mode = app.transport.mode();
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " Pour ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!("[{mode}]"), Style::default().fg(Color::Green)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Module list (or empty message)
    if app.module_keys.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            " No modules configured. Add modules to your config.toml.",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty_msg, chunks[1]);
    } else {
        // Find the longest key so we can right-pad the [key] column
        let max_key_len = app.module_keys.iter().map(|k| k.len()).max().unwrap_or(0);

        let items: Vec<ListItem> = app
            .module_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let is_selected = i == app.selected_module;
                let display_name = app
                    .config
                    .modules
                    .get(key)
                    .and_then(|m| m.display_name.as_deref())
                    .unwrap_or(key.as_str());

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let indicator = if is_selected { "> " } else { "  " };

                // Pad key tag to align display names: "[me]     " vs "[coffee]  "
                let padded_tag = format!("[{key}]{:pad$}", "", pad = max_key_len - key.len() + 1);

                ListItem::new(Line::from(vec![
                    Span::styled(format!("{indicator}"), style),
                    Span::styled(
                        padded_tag,
                        if is_selected {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                    Span::styled(display_name.to_string(), style),
                ]))
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::NONE));
        frame.render_widget(list, chunks[1]);
    }

    // Footer: key hints
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" select  "),
        Span::styled("Ctrl+E", Style::default().fg(Color::Yellow)),
        Span::raw(" configure  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ]))
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);
}

/// Actions the dashboard can signal.
#[derive(Debug, PartialEq, Eq)]
pub enum DashboardAction {
    None,
    Quit,
    SelectModule,
    ConfigureModule,
}

/// Handle a key event while on the dashboard.
pub fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) -> DashboardAction {
    use crossterm::event::{KeyCode, KeyModifiers};

    // Ctrl+E — open configurator for the selected module
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('e') {
        if !app.module_keys.is_empty() {
            return DashboardAction::ConfigureModule;
        }
        return DashboardAction::None;
    }

    match key.code {
        KeyCode::Char('q') => DashboardAction::Quit,

        KeyCode::Up => {
            if !app.module_keys.is_empty() {
                app.selected_module = if app.selected_module == 0 {
                    app.module_keys.len() - 1
                } else {
                    app.selected_module - 1
                };
            }
            DashboardAction::None
        }

        KeyCode::Down => {
            if !app.module_keys.is_empty() {
                app.selected_module = (app.selected_module + 1) % app.module_keys.len();
            }
            DashboardAction::None
        }

        KeyCode::Enter => {
            if !app.module_keys.is_empty() {
                DashboardAction::SelectModule
            } else {
                DashboardAction::None
            }
        }

        _ => DashboardAction::None,
    }
}
