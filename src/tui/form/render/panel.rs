//! Render the read-only priors review panel beside the capture form.
//!
//! Layout (§8.1):
//! - Terminal width ≥ [`SIDE_BY_SIDE_MIN_WIDTH`] → panel to the *right* of the
//!   form, fixed width.
//! - Narrower → panel *stacked below* the form, full width. On short terminals
//!   the stacked panel collapses to a one-line summary hint rather than pushing
//!   form fields off-screen (Architect decision #3, see [`STACKED_MIN_ROWS`]).
//!
//! The panel is read-only. It shows the matched tier + rank qualifier in the
//! header, the ranked rows (texture rows dimmed), and a `repeat:` summary line
//! plus an `N of M` match-count. When the resolver returns no panel, callers
//! render nothing (the empty state is a one-line hint in the layout owner).

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::priors::ResolvedPanel;

/// Minimum terminal width for the side-by-side (panel-right) layout (§8.1).
pub const SIDE_BY_SIDE_MIN_WIDTH: u16 = 100;

/// Width reserved for the panel in the side-by-side layout.
pub const PANEL_WIDTH: u16 = 34;

/// Minimum rows the stacked (below-form) panel needs to be worth rendering as a
/// box: border (2) + header (1) + at least two data rows (2) + summary (1).
/// Below this, the panel collapses to a one-line hint (Architect decision #3).
pub const STACKED_MIN_ROWS: u16 = 6;

/// Build the header line: `<tier> · <rank-qualifier>` (§8.1).
fn header_text(panel: &ResolvedPanel) -> String {
    let tier = if panel.tier_fields.is_empty() {
        "recent".to_string()
    } else {
        panel.tier_fields.join(" · ")
    };
    match &panel.rank_qualifier {
        Some(q) => format!("{tier} · {q}"),
        None => tier,
    }
}

/// The one-line collapsed hint shown when the user collapses the panel (Ctrl+R)
/// or when the stacked panel has no room. Prefers the `repeat:` summary.
pub fn collapsed_hint(panel: &ResolvedPanel) -> String {
    match &panel.summary {
        Some(summary) if !summary.cells.is_empty() => {
            let vals: Vec<String> = summary.cells.iter().map(|(_, v)| v.clone()).collect();
            format!("repeat: {} — ^R for rows", vals.join(" · "))
        }
        _ => format!(
            "{} of {} — ^R for rows",
            panel.rows.len(),
            panel.corpus_size
        ),
    }
}

/// Render the full panel box (header, rows, summary) into `area`.
pub fn render_panel(frame: &mut Frame, area: Rect, panel: &ResolvedPanel) {
    let mut lines: Vec<Line> = Vec::new();

    // Column header row.
    if !panel.columns.is_empty() {
        let cols = panel.columns.join("  ");
        lines.push(Line::from(Span::styled(
            cols,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
    }

    // Data rows — texture rows dimmed.
    for row in &panel.rows {
        let text = row.cells.join("  ");
        let style = if row.is_texture {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(text, style)));
    }

    // Separator + summary line.
    if let Some(summary) = &panel.summary
        && !summary.cells.is_empty()
    {
        lines.push(Line::from(Span::styled(
            "─".repeat((area.width.saturating_sub(2)).min(30) as usize),
            Style::default().fg(Color::DarkGray),
        )));
        let vals: Vec<String> = summary.cells.iter().map(|(_, v)| v.clone()).collect();
        lines.push(Line::from(vec![
            Span::styled("repeat: ", Style::default().fg(Color::Cyan)),
            Span::styled(vals.join(" · "), Style::default().fg(Color::Cyan)),
        ]));
    }

    // Match-count footer.
    lines.push(Line::from(Span::styled(
        format!("{} of {} captures", panel.rows.len(), panel.corpus_size),
        Style::default().fg(Color::DarkGray),
    )));

    let header = header_text(panel);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {header} "))
        .border_style(Style::default().fg(Color::DarkGray));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Render the one-line collapsed hint into a single-row `area`.
pub fn render_collapsed(frame: &mut Frame, area: Rect, panel: &ResolvedPanel) {
    let line = Line::from(vec![
        Span::styled(" ▸ ", Style::default().fg(Color::Cyan)),
        Span::styled(collapsed_hint(panel), Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
