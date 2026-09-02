//! Outline sidebar: the document's heading tree.
//!
//! Implements OXD-032. See spec §3.3.
//!
//! The tree comes straight from [`oxidiris_core::Document::headings`], which the Markdown parser
//! fills in while it walks the AST (OXD-017). Nothing is re-derived from the text here, so an
//! outline entry always points at a block the player can actually reach.

use oxidiris_core::Heading;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::term::{Capabilities, ColorLevel};

/// Columns of indentation per heading level.
const INDENT: usize = 2;

/// Index of the heading whose section contains `block_id`.
///
/// Returns `None` while the cursor is still ahead of the first heading — a preamble, a licence
/// header — rather than pretending the first heading is active.
pub fn active_index(headings: &[Heading], block_id: usize) -> Option<usize> {
    headings.iter().rposition(|h| h.block_id <= block_id)
}

/// Scroll offset that keeps `selected` visible in a viewport `height` rows tall.
pub fn scroll_for(selected: usize, height: u16, total: usize) -> u16 {
    let h = usize::from(height.max(1));
    if selected < h {
        return 0;
    }
    let last = total.saturating_sub(h);
    (selected + 1 - h).min(last) as u16
}

/// One outline row: indent, marker, text.
///
/// The active section is marked with a glyph as well as a style, because a reader who cannot see
/// the style still needs to know where they are (WCAG SC 1.4.1, spec §3.4.1).
fn row<'a>(
    heading: &'a Heading,
    is_selected: bool,
    is_active: bool,
    focused: bool,
    caps: Capabilities,
) -> Line<'a> {
    let marker = match (is_active, caps.unicode) {
        (true, true) => "\u{25b8}",
        (true, false) => ">",
        (false, _) => " ",
    };
    let indent = " ".repeat(usize::from(heading.level.saturating_sub(1)) * INDENT);

    let mut style = Style::default();
    if is_active {
        style = style.add_modifier(Modifier::BOLD);
    }
    if is_selected && focused {
        style = style.add_modifier(Modifier::REVERSED);
        if caps.color != ColorLevel::None {
            style = style.fg(Color::Yellow);
        }
    }

    Line::from(vec![
        Span::raw(marker),
        Span::raw(" "),
        Span::styled(format!("{indent}{}", heading.text), style),
    ])
}

/// Render the sidebar into `area`.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    headings: &[Heading],
    selected: usize,
    active: Option<usize>,
    focused: bool,
    caps: Capabilities,
) {
    let title = if focused { " Outline  ·  [Enter] jump " } else { " Outline  ·  [o] close " };
    let mut block = Block::bordered().title(Line::from(title));
    if focused {
        block = block.border_style(Style::default().add_modifier(Modifier::BOLD));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // An empty panel would look like a bug. Say what happened instead (OXD-032 acceptance).
    if headings.is_empty() {
        let note = vec![
            Line::raw(""),
            Line::from("No headings in").centered(),
            Line::from("this document").centered(),
        ];
        frame.render_widget(
            Paragraph::new(note).style(Style::default().add_modifier(Modifier::DIM)),
            inner,
        );
        return;
    }

    let scroll = usize::from(scroll_for(selected, inner.height, headings.len()));
    let lines: Vec<Line> = headings
        .iter()
        .enumerate()
        .skip(scroll)
        .take(usize::from(inner.height))
        .map(|(i, h)| row(h, i == selected, Some(i) == active, focused, caps))
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidiris_core::{parser, segment};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const DOC: &str = "\
preamble text

# One

body of one

## One point one

more body

# Two

body of two
";

    fn headings() -> Vec<Heading> {
        parser::parse_markdown(&segment::sanitize(DOC)).headings
    }

    #[test]
    fn the_tree_comes_from_the_parser_with_its_levels_intact() {
        let h = headings();
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].text, "One");
        assert_eq!(h[1].level, 2);
        assert_eq!(h[2].text, "Two");
    }

    #[test]
    fn the_active_heading_is_the_last_one_at_or_before_the_cursor() {
        let h = headings();
        assert_eq!(active_index(&h, h[0].block_id), Some(0));
        assert_eq!(active_index(&h, h[1].block_id + 1), Some(1));
        assert_eq!(active_index(&h, 9_999), Some(2));
    }

    /// A document can open with text before its first heading; nothing is active there.
    #[test]
    fn nothing_is_active_before_the_first_heading() {
        let h = headings();
        assert_eq!(active_index(&h, 0), None);
        assert_eq!(active_index(&[], 5), None);
    }

    #[test]
    fn the_selection_scrolls_into_view_and_stops_at_the_end() {
        assert_eq!(scroll_for(0, 5, 20), 0);
        assert_eq!(scroll_for(4, 5, 20), 0);
        assert_eq!(scroll_for(5, 5, 20), 1);
        assert_eq!(scroll_for(19, 5, 20), 15);
        assert_eq!(scroll_for(3, 10, 4), 0);
    }

    fn render_to_string(width: u16, height: u16, headings: &[Heading], selected: usize) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    headings,
                    selected,
                    Some(0),
                    true,
                    Capabilities::default(),
                )
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..width).filter_map(|x| buffer.cell((x, y))).map(|c| c.symbol()).collect())
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// Byte offsets lie here: the border and the active marker are both multi-byte.
    fn column_of(line: &str, needle: &str) -> usize {
        let byte = line.find(needle).unwrap_or_else(|| panic!("{needle:?} not in {line:?}"));
        line[..byte].chars().count()
    }

    #[test]
    fn nested_levels_are_indented() {
        let out = render_to_string(30, 8, &headings(), 0);
        let child = out.lines().find(|l| l.contains("One point one")).unwrap();
        let parent = out.lines().find(|l| l.contains("One") && !l.contains("point")).unwrap();
        let child_col = column_of(child, "One point one");
        let parent_col = column_of(parent, "One");
        assert!(child_col > parent_col, "H2 must sit further right than H1\n{out}");
    }

    #[test]
    fn a_document_without_headings_says_so() {
        let out = render_to_string(30, 8, &[], 0);
        assert!(out.contains("No headings"), "got {out}");
    }

    #[test]
    fn the_active_section_is_marked_with_a_glyph_not_only_a_style() {
        let caps = Capabilities { color: ColorLevel::None, unicode: false };
        let h = headings();
        let line = row(&h[0], false, true, false, caps);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.starts_with('>'), "expected a marker, got {text:?}");
    }

    #[test]
    fn rendering_into_a_tiny_area_does_not_panic() {
        for (w, h) in [(3u16, 3u16), (10, 4), (24, 40)] {
            render_to_string(w, h, &headings(), 2);
        }
    }
}
