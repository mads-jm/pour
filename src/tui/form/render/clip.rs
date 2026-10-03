//! Cutting one row of editable text down to the cells it has on screen.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::tui::form::cells::char_cells;

/// Chars a single-line input keeps on screen after the cursor once it scrolls.
const SCROLL_MARGIN: usize = 2;

/// The first char of `text` to draw in a `width`-cell row so the cursor, and
/// up to [`SCROLL_MARGIN`] chars after it, stay on screen.
///
/// Worked out fresh each frame from the cursor alone: the row starts at the
/// first char until the cursor nears the right edge, then the cursor rides
/// `SCROLL_MARGIN` chars in from it. Counts what [`clip_line`] draws: a `◂`
/// cell once anything is hidden on the left, a `▸` cell while anything past
/// the margin is hidden on the right.
pub(super) fn single_line_skip(text: &str, cursor: usize, width: usize) -> usize {
    let cells = char_cells(text);
    let cursor = cursor.min(cells.len());
    let end = (cursor + SCROLL_MARGIN).min(cells.len());
    let right_marker = usize::from(end < cells.len());
    // A cursor past the last char needs a cell of its own.
    let after_end = usize::from(cursor == cells.len());
    let mut needed: usize = cells[..end].iter().sum::<usize>() + after_end;
    let mut skip = 0;
    while skip < cursor {
        let left_marker = usize::from(skip > 0);
        if needed + left_marker + right_marker <= width {
            break;
        }
        needed -= cells[skip];
        skip += 1;
        // Never start the row partway through a multi-char glyph.
        while skip < cursor && cells[skip] == 0 {
            skip += 1;
        }
    }
    skip
}

/// One row of editable text cut to `width` cells, starting `skip` chars in.
///
/// `◂` takes the first cell when text is hidden on the left and `▸` the last
/// cell when text is hidden on the right. Also returns the cell, counted from
/// the row's left edge, where the char at `cursor` sits (or where the next
/// char goes when `cursor` is the end of the line).
pub(super) fn clip_line(
    text: &str,
    skip: usize,
    width: usize,
    cursor: usize,
    style: Style,
) -> (Vec<Span<'static>>, usize) {
    let chars: Vec<char> = text.chars().collect();
    let cells = char_cells(text);
    let skip = skip.min(chars.len());
    let left_clipped = skip > 0 && !chars.is_empty();
    let room = width.saturating_sub(usize::from(left_clipped));
    let rest: usize = cells[skip..].iter().sum();
    let right_clipped = rest > room;
    let content_width = room.saturating_sub(usize::from(right_clipped));

    let mut used = 0;
    let mut slice = String::new();
    for (&c, &w) in chars[skip..].iter().zip(&cells[skip..]) {
        if used + w > content_width {
            break;
        }
        used += w;
        slice.push(c);
    }

    let mut spans = Vec::new();
    if left_clipped {
        spans.push(Span::styled("◂", Style::default().fg(Color::DarkGray)));
    }
    spans.push(Span::styled(slice, style));
    if right_clipped {
        spans.push(Span::styled("▸", Style::default().fg(Color::DarkGray)));
    }

    let cursor = cursor.clamp(skip, chars.len());
    let cell = usize::from(left_clipped) + cells[skip..cursor].iter().sum::<usize>();
    (spans, cell)
}
