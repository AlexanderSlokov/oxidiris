//! Full-text panel: the document as written, with the word being read highlighted.
//!
//! Implements OXD-031. See spec §5.
//!
//! # Why the panel shows the source
//!
//! RSVP removes the page. The reader loses the sense of *where* they are, which is the failure
//! mode spec §0.2 admits to. The panel puts the page back, and the highlight is the thread between
//! the two: it is driven by [`oxidiris_core::Token::byte_span`], which every parser maintains
//! against [`oxidiris_core::Document::source`] for exactly this purpose.
//!
//! Because the spans index the source, the panel renders the source — markup and all, the way
//! `nano` would. Rendering the stripped block text instead would need a second mapping that could
//! disagree with the first, and a highlight that lands on the wrong word is worse than no panel.

use core::ops::Range;

use oxidiris_core::segment::{display_width, graphemes};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::term::{Capabilities, ColorLevel};

/// Soft-wrap `source` to `width` columns, as byte ranges into `source`.
///
/// Each returned range is a slice of `source` that is safe to render on one row: no newline, and
/// no wider than `width` columns unless a single grapheme is wider on its own.
///
/// Ranges are not contiguous. The whitespace a line was broken on belongs to neither side, which
/// is what keeps a wrapped line from starting with a stray space.
pub fn wrap(source: &str, width: u16) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for line in source.split_inclusive('\n') {
        let end = start + line.len();
        let text_end = end - usize::from(line.ends_with('\n'));
        wrap_one(source, start..text_end, width, &mut out);
        start = end;
    }
    if out.is_empty() {
        out.push(0..0);
    }
    out
}

/// Break a single hard line into as many display rows as it needs.
fn wrap_one(source: &str, span: Range<usize>, width: u16, out: &mut Vec<Range<usize>>) {
    let text = &source[span.clone()];
    if width == 0 || display_width(text) <= width {
        out.push(span);
        return;
    }

    let base = span.start;
    let mut row_start = 0usize; // byte offset of the current row, relative to `text`
    let mut used = 0u16;
    let mut pos = 0usize;
    // Byte offset just past the most recent whitespace run: the preferred place to break.
    let mut breakpoint: Option<usize> = None;

    for g in graphemes(text) {
        let w = display_width(g);
        if used + w > width && pos > row_start {
            // No whitespace to break on means a URL or a hash: cut mid-word rather than overflow.
            let cut = breakpoint.filter(|b| *b > row_start).unwrap_or(pos);
            out.push(base + row_start..base + trim_end(text, row_start, cut));
            row_start = cut;
            used = display_width(&text[row_start..pos]);
            breakpoint = None;
        }
        if g.chars().all(char::is_whitespace) {
            breakpoint = Some(pos + g.len());
        }
        used += w;
        pos += g.len();
    }
    out.push(base + row_start..span.end);
}

/// Drop trailing whitespace from `text[start..end]`, returning the shortened end offset.
fn trim_end(text: &str, start: usize, end: usize) -> usize {
    let slice = &text[start..end];
    start + slice.trim_end().len()
}

/// Index of the row containing `offset`, or the row just before it.
///
/// Returns `None` only when there are no rows. The fallback matters because [`wrap`] drops the
/// whitespace it breaks on, so an offset can land in the gap between two rows.
pub fn row_of(rows: &[Range<usize>], offset: usize) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    match rows.binary_search_by(|r| {
        if offset < r.start {
            core::cmp::Ordering::Greater
        } else if offset >= r.end {
            core::cmp::Ordering::Less
        } else {
            core::cmp::Ordering::Equal
        }
    }) {
        Ok(idx) => Some(idx),
        // `Err(i)` is the insertion point: the row before it is the last one that starts earlier.
        Err(i) => Some(i.saturating_sub(1).min(rows.len() - 1)),
    }
}

/// Scroll offset that keeps `row` on screen.
///
/// The cursor is parked a third of the way down rather than centred, so the reader sees more of
/// what is coming than of what has gone: the panel is for orientation, not for re-reading.
pub fn auto_scroll(row: usize, viewport: u16, total: usize) -> u16 {
    let height = usize::from(viewport.max(1));
    let last = total.saturating_sub(height);
    row.saturating_sub(height / 3).min(last) as u16
}

/// Clamp a manual scroll offset to the document.
pub fn clamp_scroll(scroll: u16, viewport: u16, total: usize) -> u16 {
    let last = total.saturating_sub(usize::from(viewport.max(1)));
    scroll.min(last as u16)
}

/// Style for the word currently being read.
///
/// Reversed video carries the highlight on monochrome terminals, so the panel keeps working under
/// `NO_COLOR` (WCAG SC 1.4.1, spec §3.4.1). Colour is added on top where available.
fn highlight_style(caps: Capabilities) -> Style {
    let base = Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD);
    match caps.color {
        ColorLevel::None => base,
        _ => base.fg(Color::Yellow),
    }
}

/// Split one row into styled spans, marking the part covered by `hit`.
fn row_spans<'a>(
    source: &'a str,
    row: &Range<usize>,
    hit: Option<&Range<usize>>,
    caps: Capabilities,
) -> Vec<Span<'a>> {
    let text = &source[row.clone()];
    let Some(hit) = hit.filter(|h| h.start < row.end && h.end > row.start) else {
        return vec![Span::raw(text)];
    };

    let from = hit.start.max(row.start) - row.start;
    let to = (hit.end.min(row.end) - row.start).max(from);
    let mut spans = Vec::with_capacity(3);
    if from > 0 {
        spans.push(Span::raw(&text[..from]));
    }
    spans.push(Span::styled(&text[from..to], highlight_style(caps)));
    if to < text.len() {
        spans.push(Span::raw(&text[to..]));
    }
    spans
}

/// Everything the panel needs to draw itself.
pub struct View<'a> {
    /// The document text the rows index into.
    pub source: &'a str,
    /// Wrapped rows, from [`wrap`] at this panel's inner width.
    pub rows: &'a [Range<usize>],
    /// Source range of the word being read, if any.
    pub highlight: Option<Range<usize>>,
    /// First visible row.
    pub scroll: u16,
    /// Whether the panel currently owns the keyboard.
    pub focused: bool,
    /// What the terminal can render.
    pub caps: Capabilities,
}

/// Render the panel into `area`.
pub fn render(frame: &mut Frame, area: Rect, view: &View<'_>) {
    let focused = view.focused;
    let title = if focused { " Text  ·  [Tab] back " } else { " Text  ·  [Tab] to scroll " };
    let mut block = Block::bordered().title(Line::from(title));
    if focused {
        block = block.border_style(Style::default().add_modifier(Modifier::BOLD));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let lines: Vec<Line> = view
        .rows
        .iter()
        .skip(usize::from(view.scroll))
        .take(usize::from(inner.height))
        .map(|row| Line::from(row_spans(view.source, row, view.highlight.as_ref(), view.caps)))
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidiris_core::{PacingMode, Player, parser, segment};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Every row must be a valid slice of the source it was wrapped from.
    fn assert_rows_are_slices(source: &str, rows: &[Range<usize>]) {
        for row in rows {
            assert!(source.get(row.clone()).is_some(), "row {row:?} is not a valid slice");
            assert!(!source[row.clone()].contains('\n'), "row {row:?} contains a newline");
        }
    }

    #[test]
    fn short_lines_are_returned_whole() {
        let source = "one\ntwo\n";
        let rows = wrap(source, 40);
        assert_eq!(rows.len(), 2);
        assert_eq!(&source[rows[0].clone()], "one");
        assert_eq!(&source[rows[1].clone()], "two");
    }

    #[test]
    fn long_lines_break_on_whitespace_and_stay_within_the_width() {
        let source = "alpha beta gamma delta epsilon zeta\n";
        let rows = wrap(source, 12);
        assert_rows_are_slices(source, &rows);
        assert!(rows.len() > 1);
        for row in &rows {
            assert!(
                display_width(&source[row.clone()]) <= 12,
                "row {:?} overflows: {:?}",
                row,
                &source[row.clone()]
            );
            assert!(!source[row.clone()].starts_with(' '), "row starts with a stray space");
            assert!(!source[row.clone()].ends_with(' '), "row keeps trailing space");
        }
    }

    /// A 40-character git hash has nowhere to break; it must be cut, not allowed to overflow.
    #[test]
    fn a_word_with_no_break_opportunity_is_cut_mid_word() {
        let source = "https://example.com/a/very/long/path/that/never/breaks";
        let rows = wrap(source, 10);
        assert_rows_are_slices(source, &rows);
        for row in &rows {
            assert!(display_width(&source[row.clone()]) <= 10);
        }
        let rejoined: String = rows.iter().map(|r| &source[r.clone()]).collect();
        assert_eq!(rejoined, source, "cutting mid-word must not lose characters");
    }

    #[test]
    fn wide_graphemes_are_measured_in_columns_not_characters() {
        let source = "日本語のテキストです";
        let rows = wrap(source, 8);
        assert_rows_are_slices(source, &rows);
        for row in &rows {
            assert!(display_width(&source[row.clone()]) <= 8);
        }
    }

    #[test]
    fn an_empty_document_still_produces_one_row() {
        assert_eq!(wrap("", 40), vec![0..0]);
    }

    #[test]
    fn blank_lines_are_preserved_so_paragraphs_stay_apart() {
        let source = "one\n\ntwo\n";
        let rows = wrap(source, 40);
        assert_eq!(rows.len(), 3);
        assert_eq!(&source[rows[1].clone()], "");
    }

    #[test]
    fn zero_width_does_not_loop_forever() {
        let rows = wrap("some text here\n", 0);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn row_lookup_finds_the_row_holding_an_offset() {
        let source = "alpha beta gamma delta epsilon zeta\n";
        let rows = wrap(source, 12);
        let offset = source.find("delta").unwrap();
        let row = row_of(&rows, offset).unwrap();
        assert!(
            source[rows[row].clone()].contains("delta"),
            "got {:?}",
            &source[rows[row].clone()]
        );
    }

    /// Offsets landing on the whitespace a row was broken at belong to no row; do not panic.
    #[test]
    fn row_lookup_falls_back_to_the_preceding_row() {
        let rows = vec![0..5, 6..11];
        assert_eq!(row_of(&rows, 5), Some(0));
        assert_eq!(row_of(&rows, 100), Some(1));
        assert_eq!(row_of(&[], 0), None);
    }

    #[test]
    fn auto_scroll_keeps_the_cursor_inside_the_viewport() {
        // Near the top there is nothing to scroll to.
        assert_eq!(auto_scroll(0, 10, 100), 0);
        assert_eq!(auto_scroll(2, 10, 100), 0);
        // Mid-document the row sits a third of the way down.
        assert_eq!(auto_scroll(50, 12, 100), 46);
        // At the end the last screenful is pinned.
        assert_eq!(auto_scroll(99, 10, 100), 90);
        // A document shorter than the viewport never scrolls.
        assert_eq!(auto_scroll(3, 40, 5), 0);
    }

    #[test]
    fn manual_scroll_cannot_run_past_the_end() {
        assert_eq!(clamp_scroll(500, 10, 100), 90);
        assert_eq!(clamp_scroll(5, 10, 100), 5);
        assert_eq!(clamp_scroll(5, 10, 3), 0);
    }

    fn spans_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn the_highlight_covers_exactly_the_token_and_nothing_else() {
        let source = "alpha beta gamma";
        let row = 0..source.len();
        let hit = 6..10; // "beta"
        let spans = row_spans(source, &row, Some(&hit), Capabilities::default());
        assert_eq!(spans_text(&spans), source, "highlighting must not alter the text");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content.as_ref(), "beta");
        assert!(spans[1].style.add_modifier.contains(Modifier::REVERSED));
        assert!(!spans[0].style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn a_row_without_the_token_is_left_unstyled() {
        let source = "alpha beta gamma";
        let spans = row_spans(source, &(0..5), Some(&(6..10)), Capabilities::default());
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].style.add_modifier.contains(Modifier::REVERSED));
    }

    /// A token spanning a wrap point must be highlighted on both rows, clipped to each.
    #[test]
    fn a_token_straddling_a_row_boundary_is_clipped_to_each_row() {
        let source = "alpha beta gamma";
        let first = row_spans(source, &(0..8), Some(&(6..10)), Capabilities::default());
        let second = row_spans(source, &(8..16), Some(&(6..10)), Capabilities::default());
        assert_eq!(spans_text(&first), "alpha be");
        assert_eq!(spans_text(&second), "ta gamma");
    }

    /// Under NO_COLOR the highlight has to survive on attributes alone (WCAG SC 1.4.1).
    #[test]
    fn the_highlight_survives_a_monochrome_terminal() {
        let caps = Capabilities { color: ColorLevel::None, unicode: false };
        let spans = row_spans("alpha beta", &(0..10), Some(&(0..5)), caps);
        assert!(spans[0].style.add_modifier.contains(Modifier::REVERSED));
        assert_eq!(spans[0].style.fg, None, "no colour may be emitted under NO_COLOR");
    }

    fn render_to_string(
        width: u16,
        height: u16,
        source: &str,
        hit: Option<Range<usize>>,
    ) -> String {
        let rows = wrap(source, width - 2);
        let view = View {
            source,
            rows: &rows,
            highlight: hit,
            scroll: 0,
            focused: false,
            caps: Capabilities::default(),
        };
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, frame.area(), &view)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..width).filter_map(|x| buffer.cell((x, y))).map(|c| c.symbol()).collect())
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn the_panel_draws_the_document_text() {
        let out = render_to_string(40, 8, "# Title\n\nSome body text here.\n", None);
        assert!(out.contains("# Title"), "got {out}");
        assert!(out.contains("Some body text"));
    }

    #[test]
    fn the_panel_survives_every_size_it_can_be_given() {
        for (w, h) in [(3u16, 3u16), (10, 4), (40, 24), (200, 60)] {
            render_to_string(w, h, "a b c\n\nd e f\n", Some(0..1));
        }
    }

    /// The highlight must track the player, which is the whole point of the panel.
    #[test]
    fn the_highlighted_row_follows_the_reading_cursor() {
        let text = segment::sanitize(
            "First paragraph with several words.\n\nSecond paragraph, further down the page.\n",
        );
        let doc = parser::parse_markdown(&text);
        let rows = wrap(&doc.source, 30);
        let mut player = Player::from_document(&doc, 300, PacingMode::Natural);

        let start_row = row_of(&rows, player.current().unwrap().byte_span.start).unwrap();
        player.goto_end();
        let end_row = row_of(&rows, player.current().unwrap().byte_span.start).unwrap();
        assert!(end_row > start_row, "cursor moved but the panel row did not");
    }
}
