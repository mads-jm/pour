use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;

/// Render the summary view after a write operation.
pub fn render(app: &App, frame: &mut Frame) {
    let summary = match &app.summary_state {
        Some(s) => s,
        None => return,
    };

    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(1),    // message body
            Constraint::Length(3), // footer
        ])
        .split(area);

    // Header
    let header_style = if summary.file_path.is_some() {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };
    let header_text = if summary.file_path.is_some() {
        " ▽ saved "
    } else {
        " ! error "
    };
    let header = Paragraph::new(Line::from(Span::styled(header_text, header_style)))
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Message body
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", summary.message),
            Style::default().fg(Color::White),
        )),
    ];

    if let Some(path) = &summary.file_path {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  path: ", Style::default().fg(Color::DarkGray)),
            Span::styled(path.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  transport: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", summary.transport_mode),
            Style::default().fg(Color::Yellow),
        ),
    ]));

    // Auto-created notes section
    if !summary.auto_created_notes.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  auto-created notes:",
            Style::default().fg(Color::DarkGray),
        )));
        for note in &summary.auto_created_notes {
            lines.push(Line::from(vec![
                Span::styled("    + ", Style::default().fg(Color::Green)),
                Span::styled(note.vault_path.clone(), Style::default().fg(Color::Cyan)),
            ]));
        }
    }

    let body = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);

    // Footer: key hints
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" dashboard  "),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::raw(" another entry  "),
        Span::styled("o", Style::default().fg(Color::Yellow)),
        Span::raw(" open  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ]))
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);
}

/// Actions the summary view can signal.
#[derive(Debug, PartialEq, Eq)]
pub enum SummaryAction {
    None,
    Quit,
    Dashboard,
    AnotherEntry,
    OpenInObsidian,
}

/// Handle a key event while on the summary screen.
pub fn handle_key(key: crossterm::event::KeyEvent) -> SummaryAction {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Enter => SummaryAction::Dashboard,
        KeyCode::Char('a') => SummaryAction::AnotherEntry,
        KeyCode::Char('o') => SummaryAction::OpenInObsidian,
        KeyCode::Char('q') => SummaryAction::Quit,
        _ => SummaryAction::None,
    }
}
