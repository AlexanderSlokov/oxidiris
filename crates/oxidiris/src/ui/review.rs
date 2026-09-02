//! Review Mode: the paragraph just read, held still.
//!
//! Implements OXD-034. See spec §3.3.
//!
//! RSVP takes away the backward glance — the regression that repairs a misparse, and the reason
//! §0.2 admits comprehension costs. This is the controlled version of it: playback stops, the
//! paragraph is reconstructed verbatim from [`oxidiris_core::Block::byte_span`], and `Esc` puts
//! the reader back exactly where they were.

use core::ops::Range;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::ui::panel::wrap;

/// Fraction of the screen the popup occupies, in eighths.
const WIDTH_EIGHTHS: u16 = 6;
/// Fraction of the screen height the popup occupies, in eighths.
const HEIGHT_EIGHTHS: u16 = 5;

/// Centre a `width` x `height` rectangle inside `area`, shrinking to fit if needed.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

/// Render the paragraph at `span` as a static overlay over `area`.
pub fn render(frame: &mut Frame, area: Rect, source: &str, span: Range<usize>, scroll: u16) {
    let popup =
        centered(area, area.width * WIDTH_EIGHTHS / 8, (area.height * HEIGHT_EIGHTHS / 8).max(3));
    frame.render_widget(Clear, popup);

    let block = Block::bordered().title(Line::from(" Review  ·  [Esc] resume "));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // The source is shown as written, markup and all: Review Mode exists to answer "what did that
    // actually say", and a stripped copy is not the same answer (OXD-034 acceptance).
    let text = source.get(span).unwrap_or("");
    let rows = wrap(text, inner.width);
    let lines: Vec<Line> = rows
        .iter()
        .skip(usize::from(scroll))
        .take(usize::from(inner.height))
        .map(|r| Line::raw(&text[r.clone()]))
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Number of wrapped rows the paragraph needs, used to clamp scrolling.
pub fn row_count(source: &str, span: Range<usize>, width: u16) -> usize {
    source.get(span).map_or(0, |text| wrap(text, width).len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const SOURCE: &str = "\
# Heading

A paragraph with `inline code` and a [link](https://example.com) in it.

Another paragraph.
";

    fn render_to_string(width: u16, height: u16, span: Range<usize>, scroll: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, frame.area(), SOURCE, span.clone(), scroll)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..width).filter_map(|x| buffer.cell((x, y))).map(|c| c.symbol()).collect())
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// The point of Review Mode is the original text, including the markup the stream strips.
    #[test]
    fn the_paragraph_is_shown_exactly_as_written() {
        let start = SOURCE.find("A paragraph").unwrap();
        let end = SOURCE.find("in it.").unwrap() + "in it.".len();
        let out = render_to_string(80, 20, start..end, 0);
        assert!(out.contains("`inline code`"), "markup was stripped: {out}");
        assert!(out.contains("https://example.com"), "the link target was dropped: {out}");
    }

    #[test]
    fn a_long_paragraph_can_be_scrolled() {
        let start = SOURCE.find("A paragraph").unwrap();
        let end = SOURCE.find("in it.").unwrap() + "in it.".len();
        let top = render_to_string(30, 8, start..end, 0);
        let scrolled = render_to_string(30, 8, start..end, 2);
        assert_ne!(top, scrolled, "scrolling must move the text");
    }

    #[test]
    fn row_count_grows_as_the_popup_narrows() {
        let span = 0..SOURCE.len();
        assert!(row_count(SOURCE, span.clone(), 20) > row_count(SOURCE, span, 200));
    }

    #[test]
    fn an_out_of_range_span_renders_empty_rather_than_panicking() {
        let out = render_to_string(40, 10, 9_000..9_100, 0);
        assert!(out.contains("Review"), "the frame should still be drawn");
        assert_eq!(row_count(SOURCE, 9_000..9_100, 40), 0);
    }

    #[test]
    fn the_popup_stays_inside_a_small_frame() {
        for (w, h) in [(4u16, 3u16), (20, 6), (200, 60)] {
            render_to_string(w, h, 0..20, 0);
        }
        let popup = centered(Rect { x: 0, y: 0, width: 10, height: 4 }, 40, 30);
        assert_eq!(popup.width, 10);
        assert_eq!(popup.height, 4);
    }
}
