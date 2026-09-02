//! Key reference popup.
//!
//! Implements OXD-025. See spec §7.
//!
//! Every line is generated from [`crate::keymap::BINDINGS`], so the popup cannot promise a key
//! that the dispatcher does not honour — including the modal part: the popup lists the bindings
//! that work *here*, not every binding that exists (OXD-030).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::keymap::{Mode, bindings_for, groups_for};

/// Build the popup body from the bindings that apply in `mode`.
pub fn lines(mode: Mode) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for (i, group) in groups_for(mode).into_iter().enumerate() {
        if i > 0 {
            out.push(Line::raw(""));
        }
        out.push(Line::from(Span::styled(
            group,
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        for binding in bindings_for(mode).filter(|b| b.group == group) {
            out.push(Line::from(vec![
                Span::raw(format!("  {:<10}", binding.keys)),
                Span::raw(binding.description),
            ]));
        }
    }
    out
}

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

/// Render the popup over `area`.
///
/// `scroll` lets a short terminal reach the lower entries.
pub fn render(frame: &mut Frame, area: Rect, mode: Mode, scroll: u16) {
    let body = lines(mode);
    let popup = centered(area, 46, body.len() as u16 + 2);

    frame.render_widget(Clear, popup);
    let title = format!(" Keys · {} · [?] or [Esc] to close ", mode.label());
    let block = Block::bordered().title(Line::from(title));
    frame.render_widget(Paragraph::new(body).block(block).scroll((scroll, 0)), popup);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn rendered(mode: Mode) -> String {
        lines(mode)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The popup is generated, not hand-written: every binding must appear in it.
    #[test]
    fn every_binding_appears_in_the_popup_for_its_mode() {
        for mode in [Mode::Reader, Mode::Browser, Mode::Outline] {
            let text = rendered(mode);
            for binding in bindings_for(mode) {
                assert!(
                    text.contains(binding.keys),
                    "key {:?} missing from {mode:?} help",
                    binding.keys
                );
                assert!(
                    text.contains(binding.description),
                    "description {:?} missing from {mode:?} help",
                    binding.description
                );
            }
        }
    }

    /// A key that does something else here must not be advertised with its Reader meaning.
    #[test]
    fn the_popup_does_not_promise_reader_keys_while_a_panel_has_focus() {
        let browser = rendered(Mode::Browser);
        assert!(!browser.contains("Faster"), "speed keys leaked into Browser help: {browser}");
        assert!(browser.contains("Scroll up"));
    }

    #[test]
    fn every_group_gets_a_heading() {
        let text = rendered(Mode::Reader);
        for group in groups_for(Mode::Reader) {
            assert!(text.contains(group), "group {group:?} missing from help");
        }
    }

    #[test]
    fn the_popup_is_centred_and_stays_inside_the_frame() {
        let area = Rect { x: 0, y: 0, width: 100, height: 40 };
        let popup = centered(area, 46, 20);
        assert_eq!(popup.x, 27);
        assert_eq!(popup.y, 10);
        assert!(popup.right() <= area.right());
        assert!(popup.bottom() <= area.bottom());
    }

    #[test]
    fn the_popup_shrinks_rather_than_overflowing_a_small_frame() {
        let area = Rect { x: 0, y: 0, width: 30, height: 10 };
        let popup = centered(area, 46, 30);
        assert_eq!(popup.width, 30);
        assert_eq!(popup.height, 10);
    }

    #[test]
    fn rendering_the_popup_never_panics() {
        for (w, h) in [(30u16, 10u16), (80, 24), (200, 60)] {
            for mode in [Mode::Reader, Mode::Browser, Mode::Outline] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|frame| render(frame, frame.area(), mode, 0)).unwrap();
            }
        }
    }

    #[test]
    fn scrolling_reaches_the_lower_entries_on_a_short_terminal() {
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        terminal.draw(|frame| render(frame, frame.area(), Mode::Reader, 20)).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..12)
            .flat_map(|y| (0..60).map(move |x| (x, y)))
            .filter_map(|(x, y)| buffer.cell((x, y)))
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Quit"), "scrolled view should reach the System group");
    }
}
