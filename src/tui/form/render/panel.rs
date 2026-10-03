//! Render the read-only priors review panel beside the capture form.
//!
//! Layout (§8.1): one column per prior, to the right of the form, full height.
//! Each visible field's panel row sits on the same screen line as that field's
//! first line in the form. The row positions come from the item heights
//! [`super::fields::render_fields`] returns, so two-line items (a preset with a
//! description, a callout textarea) shift the panel exactly as they shift the
//! form. The panel has no field-name column: the form's labels label it.
//!
//! As the terminal narrows, the panel drops columns ([`fit`]): the summary
//! column first, then priors from the right, down to one. Below one column's
//! width it collapses to a one-line hint at the bottom of the screen. The
//! empty state and the `Ctrl+R` collapse use the same one-line slot.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::{FieldConfig, FieldType};
use crate::data::wikilink::strip_wikilink;
use crate::priors::{PriorColumn, ResolvedPanel, age_label, same_as_current};

/// Width the form keeps before the panel gets any column. A panel column is
/// only drawn when the form still has at least this many cells.
pub const FORM_MIN_WIDTH: u16 = 60;

/// Cells of text in one panel column. Longer values truncate with `…`.
const CELL_WIDTH: usize = 9;

/// One panel column: the cell plus a one-cell gap.
const COLUMN_WIDTH: u16 = CELL_WIDTH as u16 + 1;

/// Left gutter inside the border, holding the active-row `▸`.
const GUTTER: &str = "  ";

/// Total panel width (borders included) for `columns` columns.
pub fn panel_width(columns: usize) -> u16 {
    2 + GUTTER.len() as u16 + columns as u16 * COLUMN_WIDTH
}

/// How much of the panel fits beside the form at a given terminal width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Draw the panel with this many prior columns, plus the summary column
    /// when `summary` is set.
    Columns { priors: usize, summary: bool },
    /// Not even one column fits: show the one-line hint instead.
    Hint,
}

/// Decide how many columns fit in `total_width` while the form keeps
/// [`FORM_MIN_WIDTH`]. The summary column goes first, then prior columns from
/// the right. The summary is only drawn when every prior column fits, so it
/// always summarizes exactly the priors on screen.
///
/// Thresholds at the default `limit = 3`: 3 columns from 94 cells, 2 from 84,
/// 1 from 74, the hint below 74. With `summary = true`, the summary column
/// needs 104.
pub fn fit(total_width: u16, panel: &ResolvedPanel) -> Fit {
    let n = panel.columns.len();
    let room = total_width.saturating_sub(FORM_MIN_WIDTH);
    if panel.summary.is_some() && room >= panel_width(n + 1) {
        return Fit::Columns {
            priors: n,
            summary: true,
        };
    }
    (1..=n)
        .rev()
        .find(|&k| room >= panel_width(k))
        .map_or(Fit::Hint, |k| Fit::Columns {
            priors: k,
            summary: false,
        })
}

/// One-line text for the empty state (no prior survived the hard filter).
pub fn empty_hint() -> String {
    "priors: no prior captures match this form".to_string()
}

/// One-line text when the user collapsed the panel with `Ctrl+R`.
pub fn collapsed_hint(panel: &ResolvedPanel) -> String {
    format!("priors: {} — ^R to expand", state_text(panel))
}

/// One-line text when the terminal is too narrow for one column.
///
/// Leaves out the `rank_by` label and keeps the tail short: this line only
/// shows below one column's threshold (74 cells at `limit = 3`), so the full
/// header plus a long tail would always be cut. The longest form,
/// ` ▸ priors: no close match · 3 priors — widen to see them`, is 56 cells.
pub fn narrow_hint(panel: &ResolvedPanel) -> String {
    let n = panel.columns.len();
    let noun = if n == 1 { "prior" } else { "priors" };
    format!(
        "priors: {} · {n} {noun} — widen to see them",
        panel.header_base()
    )
}

/// `similar · rating desc · 3 priors`: the header plus how many priors resolved.
fn state_text(panel: &ResolvedPanel) -> String {
    let n = panel.columns.len();
    let noun = if n == 1 { "prior" } else { "priors" };
    format!("{} · {n} {noun}", panel.header())
}

/// Where the panel's rows go, worked out from the form's list items.
pub struct RowLayout<'a> {
    /// Each visible field the form draws, with the screen row of its first
    /// line. Fields the form clips at the bottom are left out.
    pub fields: Vec<(&'a FieldConfig, u16)>,
    /// Row for the column headers (`rank_by` values, summary label): the line
    /// just above the first field.
    pub header_y: Option<u16>,
    /// Row for the age footer: the form's blank spacer line under the last
    /// field.
    pub footer_y: Option<u16>,
    /// The field whose row is highlighted. `None` on the preset or `[ pour ]`
    /// row.
    pub active: Option<&'a str>,
}

impl<'a> RowLayout<'a> {
    /// Build the layout from the form's field list area, the item heights
    /// `render_fields` returned (preset, visible fields, spacer, submit), the
    /// visible fields in order, and the form's `active_field` index.
    ///
    /// An item is on screen only when it fits whole, matching how ratatui's
    /// `List` clips an unscrolled list at the bottom of its area.
    pub fn new(
        fields_area: Rect,
        item_heights: &[usize],
        visible: &[&'a FieldConfig],
        active_field: usize,
    ) -> RowLayout<'a> {
        let mut tops = Vec::with_capacity(item_heights.len());
        let mut y = 0usize;
        for h in item_heights {
            let fits = y + h <= fields_area.height as usize;
            tops.push(fits.then(|| fields_area.y + y as u16));
            y += h;
        }
        let top = |i: usize| tops.get(i).copied().flatten();

        let fields: Vec<(&FieldConfig, u16)> = visible
            .iter()
            .enumerate()
            .filter_map(|(vi, f)| top(vi + 1).map(|y| (*f, y)))
            .collect();
        let header_y = fields.first().map(|(_, y)| y.saturating_sub(1));
        let footer_y = top(visible.len() + 1);
        let active = active_field
            .checked_sub(1)
            .and_then(|vi| visible.get(vi))
            .map(|f| f.name.as_str());

        RowLayout {
            fields,
            header_y,
            footer_y,
            active,
        }
    }
}

/// Render the panel box into `area` with the columns [`fit`] chose.
///
/// `form_values` is the form's live state: a cell equal to the current value
/// renders `·`, so editing any field moves the marks on the next frame.
pub fn render_panel(
    frame: &mut Frame,
    area: Rect,
    panel: &ResolvedPanel,
    fit: (usize, bool),
    rows: &RowLayout,
    form_values: &std::collections::HashMap<String, String>,
) {
    let (shown, with_summary) = fit;
    let columns: Vec<&PriorColumn> = panel.columns.iter().take(shown).collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", header_for(panel, area.width)))
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut draw = |y: u16, line: Line, row_style: Style| {
        if y >= inner.y && y < inner.bottom() {
            let rect = Rect {
                y,
                height: 1,
                ..inner
            };
            frame.render_widget(Paragraph::new(line).style(row_style), rect);
        }
    };

    // Column headers: each prior's `rank_by` value, and the summary label.
    if let Some(y) = rows.header_y {
        let mut spans = vec![Span::raw(GUTTER)];
        for col in &columns {
            let text = col.rank_value.as_deref().unwrap_or("");
            spans.push(cell_span(text, Style::default().fg(Color::Cyan)));
        }
        if with_summary && let Some(summary) = &panel.summary {
            spans.push(cell_span(
                &summary.label,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
        draw(y, Line::from(spans), Style::default());
    }

    // One row per visible field, on that field's screen line.
    for (field, y) in &rows.fields {
        let active = rows.active == Some(field.name.as_str());
        let current = form_values
            .get(&field.name)
            .map(String::as_str)
            .unwrap_or("");
        let mut spans = vec![if active {
            Span::styled(
                "▸ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw(GUTTER)
        }];
        // Composite rows stay blank: presets cover recipe stages (§8.1).
        // `·` is decided on the full value; only the text cells left over
        // get their shared prefixes elided.
        let cells: Vec<Option<&str>> = columns
            .iter()
            .map(|col| {
                if field.field_type == FieldType::CompositeArray {
                    None
                } else {
                    panel.cell(col, &field.name)
                }
            })
            .collect();
        let texts: Vec<&str> = cells
            .iter()
            .flatten()
            .copied()
            .filter(|v| !same_as_current(v, current))
            .collect();
        let shown = elide_shared_prefixes(&texts, current);
        let mut shown = shown.iter();
        for (col, cell) in columns.iter().zip(&cells) {
            spans.push(match cell {
                None => cell_span("", Style::default()),
                Some(v) if same_as_current(v, current) => {
                    let fg = if active { Color::Gray } else { Color::DarkGray };
                    cell_span("·", Style::default().fg(fg))
                }
                Some(v) => {
                    let style = if col.dimmed {
                        Style::default().fg(if active { Color::Gray } else { Color::DarkGray })
                    } else {
                        Style::default().fg(Color::Yellow)
                    };
                    cell_span(shown.next().map_or(*v, String::as_str), style)
                }
            });
        }
        if with_summary {
            let cell = panel.summary_cell(&field.name).unwrap_or("");
            spans.push(cell_span(cell, Style::default().fg(Color::Cyan)));
        }
        let row_style = if active {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        draw(*y, Line::from(spans), row_style);
    }

    // Column footers: each prior's age.
    if let Some(y) = rows.footer_y {
        let now = chrono::Utc::now().timestamp_millis();
        let mut spans = vec![Span::raw(GUTTER)];
        for col in &columns {
            spans.push(cell_span(
                &age_label(col.recency, now),
                Style::default().fg(Color::DarkGray),
            ));
        }
        draw(y, Line::from(spans), Style::default());
    }
}

/// The box title for a panel `width` cells wide: the full header when it
/// fits between the corners, else just `similar` / `no close match`, so a
/// one- or two-column panel doesn't show a label cut mid-word.
fn header_for(panel: &ResolvedPanel, width: u16) -> String {
    let room = (width as usize).saturating_sub(4); // corners + padding spaces
    let full = panel.header();
    if full.width() <= room {
        full
    } else {
        panel.header_base().to_string()
    }
}

/// Render a one-line hint (collapsed, too narrow, or empty) into `area`.
pub fn render_hint(frame: &mut Frame, area: Rect, text: &str) {
    let line = Line::from(vec![
        Span::styled(" ▸ ", Style::default().fg(Color::Cyan)),
        Span::styled(text.to_string(), Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// What each text cell in one panel row shows, given the row's text values
/// (in column order) and the form's current value for the field.
///
/// A value that fits in a cell shows as is. A longer value drops the longest
/// prefix it shares with a *different* value on the same screen line (another
/// text cell, or the form's own value) and shows `…` in its place, so
/// `YesPlz - Homestar` beside `YesPlz - SOE Ethiopia` reads `…Homestar` and
/// `…SOE Ethi…` instead of `YesPlz -…` twice. The shared prefix is cut back to
/// just after its last non-alphanumeric character, so a word is never split at
/// the front (`Ethiopia` vs `Ethiopian` share no elidable prefix). Nothing is
/// elided when that would leave the value blank or save no width.
fn elide_shared_prefixes(texts: &[&str], current: &str) -> Vec<String> {
    let current = strip_wikilink(current.trim());
    texts
        .iter()
        .map(|&v| {
            if v.width() <= CELL_WIDTH {
                return v.to_string();
            }
            let cut = texts
                .iter()
                .copied()
                .chain((!current.is_empty()).then_some(current.as_str()))
                .filter(|&o| o != v)
                .map(|o| shared_prefix_at_boundary(v, o))
                .max()
                .unwrap_or(0);
            let rest = &v[cut..];
            // `…` takes one cell, so the elided head must be at least two.
            if v[..cut].width() < 2 || rest.trim().is_empty() {
                v.to_string()
            } else {
                format!("…{rest}")
            }
        })
        .collect()
}

/// Byte length of the prefix `a` shares with `b`, cut back to just after the
/// last non-alphanumeric character inside it. `0` when there is none.
fn shared_prefix_at_boundary(a: &str, b: &str) -> usize {
    let common = a
        .char_indices()
        .zip(b.chars())
        .take_while(|((_, x), y)| x == y)
        .last()
        .map_or(0, |((i, c), _)| i + c.len_utf8());
    a[..common]
        .char_indices()
        .rfind(|(_, c)| !c.is_alphanumeric())
        .map_or(0, |(i, c)| i + c.len_utf8())
}

/// One column's cell: `text` cut to [`CELL_WIDTH`] cells (`…` marks a cut),
/// padded to the full column width.
fn cell_span(text: &str, style: Style) -> Span<'static> {
    let mut out = String::new();
    if text.width() <= CELL_WIDTH {
        out.push_str(text);
    } else {
        let mut used = 0;
        for c in text.chars() {
            let w = c.width().unwrap_or(0);
            if used + w > CELL_WIDTH - 1 {
                break;
            }
            used += w;
            out.push(c);
        }
        out.push('…');
    }
    let pad = (COLUMN_WIDTH as usize).saturating_sub(out.width());
    out.push_str(&" ".repeat(pad));
    Span::styled(out, style)
}
