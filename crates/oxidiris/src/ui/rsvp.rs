//! The RSVP frame: one word, anchored so the eye never moves.
//!
//! Implements OXD-023. See spec §3.1.3 and §3.4.1.
//!
//! # The one invariant
//!
//! The ORP grapheme is drawn at the *same absolute column* for every word in the document. Not
//! approximately, not usually: exactly, or the tool has no reason to exist.

use oxidiris_core::orp;
use oxidiris_core::segment::{display_width, graphemes};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::term::{Capabilities, ColorLevel};

/// A word broken into the three pieces the renderer draws, plus its left padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fitted {
    /// Spaces to the left of the word.
    pub pad: u16,
    /// Graphemes before the anchor.
    pub prefix: String,
    /// The anchored grapheme.
    pub focus: String,
    /// Graphemes after the anchor.
    pub suffix: String,
}

impl Fitted {
    /// Column at which the focus grapheme starts.
    pub fn focus_column(&self) -> u16 {
        self.pad + display_width(&self.prefix)
    }
}

/// Keep the leftmost graphemes that fit in `max` columns, marking the cut with an ellipsis.
fn truncate_right(text: &str, max: u16) -> String {
    if display_width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "\u{2026}".to_string();
    }
    let budget = max - 1; // room for the ellipsis
    let mut out = String::new();
    let mut used = 0u16;
    for g in graphemes(text) {
        let w = display_width(g);
        if used + w > budget {
            break;
        }
        out.push_str(g);
        used += w;
    }
    out.push('\u{2026}');
    out
}

/// Keep the rightmost graphemes that fit in `max` columns.
fn truncate_left(text: &str, max: u16) -> String {
    if display_width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let budget = max - 1;
    let mut kept: Vec<&str> = Vec::new();
    let mut used = 0u16;
    for g in graphemes(text).into_iter().rev() {
        let w = display_width(g);
        if used + w > budget {
            break;
        }
        kept.push(g);
        used += w;
    }
    kept.reverse();
    format!("\u{2026}{}", kept.concat())
}

/// Lay out `word` so its ORP grapheme lands on column `anchor` inside a frame `width` wide.
///
/// The padding is computed from the *column width* of the prefix, never from its character count,
/// which is what keeps CJK and emoji from shifting the anchor (spec §3.1.1).
pub fn fit(word: &str, anchor: u16, width: u16) -> Fitted {
    let layout = orp::layout(word);
    let focus_width = display_width(&layout.focus);

    // A word whose prefix is wider than the anchor cannot be placed as-is; trimming from the left
    // keeps the anchor honest at the cost of the word's head, which is the lesser evil.
    let prefix = if layout.orp_offset > anchor {
        truncate_left(&layout.prefix, anchor)
    } else {
        layout.prefix
    };
    let prefix_width = display_width(&prefix);
    let pad = anchor.saturating_sub(prefix_width);

    let used = pad.saturating_add(prefix_width).saturating_add(focus_width);
    let right_budget = width.saturating_sub(used);
    let suffix = truncate_right(&layout.suffix, right_budget);

    Fitted { pad, prefix, focus: layout.focus, suffix }
}

/// Style for the anchored grapheme.
///
/// Bold and underline are applied unconditionally, so the anchor survives a monochrome terminal
/// and remains visible to a colour-blind reader. Colour is an *addition*, never the only signal
/// (WCAG SC 1.4.1, spec §3.4.1).
fn focus_style(caps: Capabilities) -> Style {
    let base = Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    match caps.color {
        ColorLevel::None => base,
        ColorLevel::Ansi16 => base.fg(Color::LightRed),
        ColorLevel::Ansi256 | ColorLevel::TrueColor => base.fg(Color::Rgb(0xFF, 0x5F, 0x5F)),
    }
}

/// Render the reader frame into `area`.
pub fn render(frame: &mut Frame, area: Rect, word: Option<&str>, title: &str, caps: Capabilities) {
    let block = Block::bordered().title(Line::from(format!(" {title} ")));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let anchor = inner.width / 2;
    let (marker_top, marker_bottom) = caps.markers();
    let marker_line = |glyph: &str| {
        Line::from(vec![Span::raw(" ".repeat(anchor as usize)), Span::raw(glyph.to_string())])
    };

    let word_line = match word {
        Some(w) => {
            let f = fit(w, anchor, inner.width);
            // The reason this program exists, checked on every debug frame.
            debug_assert_eq!(f.focus_column(), anchor, "ORP left the anchor column");
            Line::from(vec![
                Span::raw(" ".repeat(f.pad as usize)),
                Span::raw(f.prefix),
                Span::styled(f.focus, focus_style(caps)),
                Span::raw(f.suffix),
            ])
        }
        None => Line::from(vec![
            Span::raw(" ".repeat(anchor.saturating_sub(3) as usize)),
            Span::styled("[end]", Style::default().add_modifier(Modifier::DIM)),
        ]),
    };

    // Centre the three-line group vertically, but never above the frame.
    let top_offset = inner.height.saturating_sub(3) / 2;
    let lines = vec![Line::raw(""); top_offset as usize]
        .into_iter()
        .chain([marker_line(marker_top), word_line, marker_line(marker_bottom)])
        .collect::<Vec<_>>();

    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidiris_core::segment::sanitize;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// The corpus that every alignment claim is checked against.
    const CORPUS: &[&str] = &[
        "a",
        "I",
        "at",
        "the",
        "hello",
        "reading",
        "javascript",
        "internationalization",
        "supercalifragilisticexpialidocious",
        "Oxidiris",
        "tiếng",
        "Việt",
        "nghiên",
        "日本語",
        "日本語文書",
        "3.14159",
        "std::fmt",
        "v1.2.3",
        "e.g.",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        "\u{1F1FB}\u{1F1F3}",
        "café",
    ];

    #[test]
    fn focus_lands_on_the_anchor_for_every_word_in_the_corpus() {
        for width in [40u16, 80, 120, 200] {
            let anchor = width / 2;
            for word in CORPUS {
                let f = fit(&sanitize(word), anchor, width);
                assert_eq!(
                    f.focus_column(),
                    anchor,
                    "word {word:?} at width {width} put the anchor on column {}",
                    f.focus_column()
                );
            }
        }
    }

    #[test]
    fn fitted_pieces_reassemble_the_word_when_nothing_is_truncated() {
        for word in CORPUS {
            let w = sanitize(word);
            let f = fit(&w, 60, 120);
            assert_eq!(format!("{}{}{}", f.prefix, f.focus, f.suffix), w);
        }
    }

    #[test]
    fn an_over_long_word_is_truncated_rather_than_overflowing() {
        let word = "a".repeat(200);
        let f = fit(&word, 20, 40);
        let drawn =
            f.pad + display_width(&f.prefix) + display_width(&f.focus) + display_width(&f.suffix);
        assert!(drawn <= 40, "drew {drawn} columns into a 40 column frame");
        assert!(f.suffix.ends_with('\u{2026}'), "truncation must be visible");
    }

    #[test]
    fn a_word_wider_than_the_anchor_keeps_the_anchor_and_trims_its_head() {
        // ORP index 4 on a wide-character word: the prefix alone is 8 columns.
        let word = "日本語文書館庫室";
        let f = fit(word, 3, 20);
        assert_eq!(f.focus_column(), 3);
        assert!(f.prefix.starts_with('\u{2026}'));
    }

    #[test]
    fn a_zero_width_frame_does_not_panic() {
        let f = fit("hello", 0, 0);
        assert_eq!(f.pad, 0);
    }

    /// End-to-end through a real ratatui buffer: the anchor cell must hold the focus grapheme.
    #[test]
    fn the_rendered_buffer_puts_the_focus_grapheme_on_the_anchor_column() {
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        let caps = Capabilities::default();

        for word in CORPUS {
            let w = sanitize(word);
            terminal.draw(|frame| render(frame, frame.area(), Some(&w), "test", caps)).unwrap();

            let buffer = terminal.backend().buffer();
            // Frame is 80 wide with a 1-column border, so inner width is 78 and the anchor is 39.
            let anchor_x = 1 + 78 / 2;
            let expected = orp::layout(&w).focus;

            let found = (0..12)
                .filter_map(|y| buffer.cell((anchor_x, y)))
                .any(|cell| cell.symbol() == expected);
            assert!(found, "focus grapheme {expected:?} of {word:?} was not on column {anchor_x}");
        }
    }

    #[test]
    fn the_anchor_column_is_identical_across_words_of_wildly_different_lengths() {
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        let caps = Capabilities::default();
        let mut columns = Vec::new();

        for word in ["a", "hello", "internationalization", "日本語文書"] {
            terminal.draw(|frame| render(frame, frame.area(), Some(word), "t", caps)).unwrap();
            let buffer = terminal.backend().buffer();
            let expected = orp::layout(word).focus;
            let col = (0..80u16)
                .find(|x| {
                    (0..12u16)
                        .filter_map(|y| buffer.cell((*x, y)))
                        .any(|c| c.symbol() == expected && *x > 0)
                })
                .unwrap();
            columns.push(col);
        }
        assert!(
            columns.windows(2).all(|w| w[0] == w[1]),
            "anchor column moved between words: {columns:?}"
        );
    }

    /// WCAG SC 1.4.1: with colour unavailable the anchor must still be identifiable.
    #[test]
    fn the_anchor_survives_a_monochrome_terminal() {
        let caps = Capabilities { color: ColorLevel::None, unicode: true };
        let style = focus_style(caps);
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
        assert!(style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(style.fg, None, "monochrome terminals must not be sent colour");
    }

    #[test]
    fn colour_terminals_get_colour_on_top_of_the_non_colour_signal() {
        let style = focus_style(Capabilities { color: ColorLevel::TrueColor, unicode: true });
        assert!(style.fg.is_some());
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn markers_are_drawn_on_the_anchor_column() {
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        let caps = Capabilities::default();
        terminal.draw(|frame| render(frame, frame.area(), Some("hello"), "t", caps)).unwrap();
        let buffer = terminal.backend().buffer();
        let anchor_x = 1 + 78 / 2;
        let symbols: Vec<String> =
            (0..12).filter_map(|y| buffer.cell((anchor_x, y))).map(|c| c.symbol().into()).collect();
        assert!(symbols.iter().any(|s| s == "▼"));
        assert!(symbols.iter().any(|s| s == "▲"));
    }

    #[test]
    fn rendering_at_many_sizes_never_panics() {
        let caps = Capabilities::default();
        for (w, h) in [(30u16, 10u16), (40, 12), (80, 24), (200, 50), (31, 10)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal
                .draw(|frame| render(frame, frame.area(), Some("internationalization"), "t", caps))
                .unwrap();
        }
    }

    #[test]
    fn the_end_of_stream_marker_renders_without_a_word() {
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), None, "t", Capabilities::default()))
            .unwrap();
    }
}
