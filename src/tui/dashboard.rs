use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::App;
use crate::data::history::format_relative;

/// Direction for module reordering.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MoveDirection {
    Up,
    Down,
}

/// Render the dashboard view: header, ambient stats, module list, recent/gaps, footer.
pub fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Build the recent + gaps section content to determine its height
    let recent_entries = app.history.recent(3);
    let last_per_module = app.history.last_per_module();

    // Gaps: modules with no entry this week
    let week_start = {
        let today = chrono::Local::now().date_naive();
        let wd = chrono::Datelike::weekday(&today).num_days_from_monday();
        today - chrono::Duration::days(wd as i64)
    };
    let gap_modules: Vec<(&str, String)> = app
        .module_keys
        .iter()
        .filter_map(|key| {
            match last_per_module.get(key.as_str()) {
                Some(ts) => {
                    let d = ts.with_timezone(&chrono::Local).date_naive();
                    if d < week_start {
                        Some((key.as_str(), format!("last: {}", format_relative(*ts))))
                    } else {
                        None
                    }
                }
                None => Some((key.as_str(), "never".to_string())),
            }
        })
        .collect();

    // Height for the recent/gaps section: "recent" header + entries + optional gap header + gaps
    let recent_lines = if recent_entries.is_empty() { 0 } else { 1 + recent_entries.len() };
    let gap_lines = if gap_modules.is_empty() { 0 } else { 1 + gap_modules.len() };
    let bottom_section_height = (recent_lines + gap_lines) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Length(1), // ambient stats
            Constraint::Min(3),   // module list
            Constraint::Length(bottom_section_height.max(1)), // recent + gaps
            Constraint::Length(3), // footer
        ])
        .split(area);

    // ── Header ──
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " ▽ pour",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // ── Ambient stats row ──
    let mode = app.transport.mode();
    let last_text = match app.history.last_pour() {
        Some(e) => format_relative(e.timestamp),
        None => "never".to_string(),
    };
    let today_count = app.history.today_count();
    let week_count = app.history.week_count();
    let streak = app.history.streak();

    let dim = Style::default().fg(Color::DarkGray);
    let val = Style::default().fg(Color::White);

    let mut stats_spans = vec![
        Span::styled(" last: ", dim),
        Span::styled(last_text, val),
        Span::styled("   today: ", dim),
        Span::styled(today_count.to_string(), val),
        Span::styled("   week: ", dim),
        Span::styled(week_count.to_string(), val),
    ];
    if streak > 0 {
        stats_spans.push(Span::styled("   streak: ", dim));
        stats_spans.push(Span::styled(format!("{streak}d"), val));
    }
    stats_spans.push(Span::styled("   ", dim));
    stats_spans.push(Span::styled(format!("[{mode}]"), Style::default().fg(Color::Green)));

    let stats_row = Paragraph::new(Line::from(stats_spans));
    frame.render_widget(stats_row, chunks[1]);

    // ── Module list ──
    if app.module_keys.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            " no modules configured. add modules to config.toml.",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty_msg, chunks[2]);
    } else {
        let max_key_len = app.module_keys.iter().map(|k| k.len()).max().unwrap_or(0);
        let module_today = app.history.per_module_today();

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

                let indicator = if is_selected { "▸ " } else { "  " };
                let padded_tag = format!("[{key}]{:pad$}", "", pad = max_key_len - key.len() + 1);

                let count = module_today.get(key.as_str()).copied().unwrap_or(0);
                let count_span = if count > 0 {
                    Span::styled(format!("  {count}"), Style::default().fg(Color::DarkGray))
                } else {
                    Span::raw("")
                };

                ListItem::new(Line::from(vec![
                    Span::styled(indicator.to_string(), style),
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
                    count_span,
                ]))
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::NONE));
        frame.render_widget(list, chunks[2]);
    }

    // ── Recent activity + Gaps ──
    let mut bottom_lines: Vec<Line> = Vec::new();

    if !recent_entries.is_empty() {
        bottom_lines.push(Line::from(Span::styled(
            " recent",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        )));
        for entry in &recent_entries {
            let time_str = format_relative(entry.timestamp);
            bottom_lines.push(Line::from(vec![
                Span::styled(format!("   {:<12}", entry.module_key), Style::default().fg(Color::White)),
                Span::styled(time_str, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    if !gap_modules.is_empty() {
        bottom_lines.push(Line::from(Span::styled(
            " gaps",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        )));
        for (module, label) in &gap_modules {
            bottom_lines.push(Line::from(vec![
                Span::styled(format!("   {:<12}", module), Style::default().fg(Color::Yellow)),
                Span::styled(label.clone(), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    if bottom_lines.is_empty() {
        bottom_lines.push(Line::from(""));
    }

    let bottom_section = Paragraph::new(bottom_lines);
    frame.render_widget(bottom_section, chunks[3]);

    // ── Footer ──
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate  "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" select  "),
        Span::styled("?", Style::default().fg(Color::Yellow)),
        Span::raw(" help  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ]))
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[4]);

    // Overlays — rendered on top of everything else
    if app.help_open {
        render_help_overlay(frame, area);
    } else if !app.startup_warnings.is_empty() {
        render_warnings_overlay(app, frame, area);
    }
}

/// Render a centered help overlay listing all dashboard keybindings.
fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let key_style = Style::default().fg(Color::Yellow);
    let desc_style = Style::default().fg(Color::White);
    let dim = Style::default().fg(Color::DarkGray);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑↓        ", key_style),
            Span::styled("navigate modules", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  Enter     ", key_style),
            Span::styled("open selected module", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  q         ", key_style),
            Span::styled("quit", desc_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  e         ", key_style),
            Span::styled("configure module", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  v         ", key_style),
            Span::styled("vault settings", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  n         ", key_style),
            Span::styled("new module", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+↑↓   ", key_style),
            Span::styled("reorder modules", desc_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ?/Esc     ", dim),
            Span::styled("close", dim),
        ]),
    ];

    let overlay_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4)); // +2 for border
    let overlay_width = 40u16.min(area.width);
    let overlay_area = centered_rect(overlay_width, overlay_height, area);

    frame.render_widget(Clear, overlay_area);

    let overlay = Paragraph::new(lines).block(
        Block::default()
            .title(" Keybindings ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(overlay, overlay_area);
}

/// Render a centered "Path Warnings" overlay on the given area.
fn render_warnings_overlay(app: &App, frame: &mut Frame, area: Rect) {
    // Height: 2 (border) + 1 (blank) + warnings + 1 (blank) + 1 (footer) + 1 (border bottom)
    let warning_count = app.startup_warnings.len() as u16;
    let overlay_height = (warning_count + 5).min(area.height.saturating_sub(4));
    let overlay_width = (area.width * 3 / 4).max(50).min(area.width);

    let overlay_area = centered_rect(overlay_width, overlay_height, area);

    // Clear the background so the overlay isn't transparent
    frame.render_widget(Clear, overlay_area);

    // Build warning lines
    let mut lines: Vec<Line> = vec![Line::from("")];
    for w in &app.startup_warnings {
        lines.push(Line::from(Span::styled(
            format!("  {w}"),
            Style::default().fg(Color::Yellow),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    let footer = Line::from(vec![
        Span::styled(" Enter", Style::default().fg(Color::Green)),
        Span::raw(" continue  "),
        Span::styled("e", Style::default().fg(Color::Green)),
        Span::raw(" configure "),
    ]);

    let overlay = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Path Warnings ")
                .title_alignment(Alignment::Center)
                .title_bottom(footer)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(overlay, overlay_area);
}

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

/// Actions the dashboard can signal.
#[derive(Debug, PartialEq, Eq)]
pub enum DashboardAction {
    None,
    Quit,
    SelectModule,
    ConfigureModule,
    ConfigureVault,
    /// Move the selected module up or down in the list.
    ReorderModule(MoveDirection),
    /// Open the new module creation screen.
    NewModule,
}

/// Handle a key event while on the dashboard.
pub fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) -> DashboardAction {
    use crossterm::event::{KeyCode, KeyModifiers};

    // While the help overlay is visible, intercept all keys
    if app.help_open {
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => app.help_open = false,
            _ => {}
        }
        return DashboardAction::None;
    }

    // While the startup warnings overlay is visible, intercept all keys
    if !app.startup_warnings.is_empty() {
        if key.code == KeyCode::Char('e') {
            // Try to select the module mentioned in the first warning so
            // ConfigureModule opens the right one (warnings start with
            // "module '<key>': ...").
            if let Some(first) = app.startup_warnings.first() {
                if let Some(start) = first.find("'") {
                    if let Some(end) = first[start + 1..].find("'") {
                        let key = &first[start + 1..start + 1 + end];
                        if let Some(idx) = app.module_keys.iter().position(|k| k == key) {
                            app.selected_module = idx;
                        }
                    }
                }
            }
            app.startup_warnings.clear();
            if !app.module_keys.is_empty() {
                return DashboardAction::ConfigureModule;
            }
            return DashboardAction::None;
        }
        if key.code == KeyCode::Enter {
            app.startup_warnings.clear();
        }
        return DashboardAction::None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            // Ctrl+Up — move selected module up
            KeyCode::Up => {
                if app.selected_module > 0 {
                    return DashboardAction::ReorderModule(MoveDirection::Up);
                }
                return DashboardAction::None;
            }
            // Ctrl+Down — move selected module down
            KeyCode::Down => {
                if !app.module_keys.is_empty()
                    && app.selected_module < app.module_keys.len() - 1
                {
                    return DashboardAction::ReorderModule(MoveDirection::Down);
                }
                return DashboardAction::None;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => DashboardAction::Quit,

        // e — open configurator for the selected module
        KeyCode::Char('e') => {
            if !app.module_keys.is_empty() {
                return DashboardAction::ConfigureModule;
            }
            DashboardAction::None
        }
        // v — open vault settings editor
        KeyCode::Char('v') => DashboardAction::ConfigureVault,
        // n — create a new module
        KeyCode::Char('n') => DashboardAction::NewModule,

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

        KeyCode::Char('?') => {
            app.help_open = true;
            DashboardAction::None
        }

        _ => DashboardAction::None,
    }
}
