//! Terminal cells per char, measured the way ratatui draws text.

use ratatui::style::Style;
use ratatui::text::Span;

/// The cells each char of `text` takes on screen, one entry per char.
///
/// Ratatui lays text out by grapheme, not by char. A cluster such as `❤️` (a
/// heart plus a variation selector) or `👍🏽` (an emoji plus a skin-tone
/// modifier) is drawn as one glyph, two cells wide, though its chars' own
/// widths sum to 1 and 4. So a cluster's full width goes on its first char
/// and the rest of its chars get 0. Summing any run of chars that starts and
/// ends on a cluster boundary then gives the cells ratatui draws for it.
///
/// A cluster holding a control char counts as 0, as a lone control char did
/// before, and so does a `\n`.
pub(super) fn char_cells(text: &str) -> Vec<usize> {
    let mut cells = Vec::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            cells.push(0);
        }
        // `styled_graphemes` drops `\n`, so it only ever sees one line.
        let span = Span::raw(line);
        for grapheme in span.styled_graphemes(Style::default()) {
            let symbol = grapheme.symbol;
            let width = if symbol.chars().any(char::is_control) {
                0
            } else {
                Span::raw(symbol).width()
            };
            let mut chars = symbol.chars();
            if chars.next().is_some() {
                cells.push(width);
            }
            cells.extend(chars.map(|_| 0));
        }
    }
    cells
}

/// Cells `text` takes on screen. Equal to summing [`char_cells`].
pub(super) fn str_cells(text: &str) -> usize {
    char_cells(text).iter().sum()
}
