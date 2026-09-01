//! Screen composition.

pub mod help;
pub mod rsvp;
pub mod status;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::term::{ABS_MIN_HEIGHT, ABS_MIN_WIDTH, SizeClass, size_class};

/// Rows reserved for the status block, including its border.
const STATUS_HEIGHT: u16 = 4;

/// Draw the whole screen.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if size_class(area.width, area.height) == SizeClass::TooSmall {
        render_too_small(frame, area);
        return;
    }

    let [reader_area, status_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(STATUS_HEIGHT)]).areas(area);

    rsvp::render(frame, reader_area, app.current_word(), &app.title, app.caps);
    status::render(frame, status_area, &app.player, app.caps, app.message.as_deref());

    if app.show_help {
        help::render(frame, area, app.help_scroll);
    }
}

/// Ask for a bigger window instead of drawing a broken layout (spec §3.4.4).
fn render_too_small(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("Terminal too small").centered(),
        Line::from(format!("need at least {ABS_MIN_WIDTH}x{ABS_MIN_HEIGHT}")).centered(),
        Line::from(format!("current: {}x{}", area.width, area.height)).centered(),
        Line::from("").centered(),
        Line::from("[q] quit").centered(),
    ];
    let top = area.height.saturating_sub(text.len() as u16) / 2;
    let mut lines = vec![Line::raw(""); top as usize];
    lines.extend(text);
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().add_modifier(Modifier::BOLD)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use oxidiris_core::{PacingMode, parser, segment};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn app() -> App {
        let text = segment::sanitize("# Title\n\nSome words to read here quickly.\n");
        let doc = parser::parse_markdown(&text);
        App::new(&doc, "test.md".into(), 300, PacingMode::Natural)
    }

    #[test]
    fn the_full_screen_renders_at_every_supported_size() {
        for (w, h) in [(30u16, 10u16), (40, 12), (80, 24), (120, 40), (200, 60)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| render(frame, &app())).unwrap();
        }
    }

    #[test]
    fn a_window_below_the_absolute_minimum_asks_to_be_enlarged() {
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();
        terminal.draw(|frame| render(frame, &app())).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..6u16)
            .flat_map(|y| (0..20u16).map(move |x| (x, y)))
            .filter_map(|(x, y)| buffer.cell((x, y)))
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("too small"), "expected a size prompt, got {text:?}");
    }

    #[test]
    fn the_help_overlay_draws_on_top_of_the_reader() {
        let mut a = app();
        a.show_help = true;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &a)).unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..24u16)
            .flat_map(|y| (0..80u16).map(move |x| (x, y)))
            .filter_map(|(x, y)| buffer.cell((x, y)))
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Play / pause"));
    }

    /// Render one real frame of this project's own backlog and print it.
    ///
    /// Ignored by default because its value is visual, not assertive. Run it with
    /// `make frame` to eyeball the layout without opening a terminal session.
    #[test]
    #[ignore = "visual check, run via `make frame`"]
    fn render_frame_of_the_backlog() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../BACKLOG.md");
        let doc = oxidiris_core::load(&std::fs::read(path).unwrap(), None).unwrap();
        let mut a = App::new(&doc, "BACKLOG.md".into(), 400, PacingMode::Natural);
        a.player.seek_ratio(0.012);

        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal.draw(|frame| render(frame, &a)).unwrap();
        let buffer = terminal.backend().buffer();

        println!();
        for y in 0..16u16 {
            let line: String =
                (0..80u16).filter_map(|x| buffer.cell((x, y))).map(|c| c.symbol()).collect();
            println!("{}", line.trim_end());
        }
        println!("\ncurrent word: {:?}", a.current_word());
        println!("tokens: {}", a.player.tokens().len());
        println!("headings: {}", doc.headings.len());
    }

    #[test]
    fn resizing_repeatedly_keeps_the_layout_intact() {
        let a = app();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        for (w, h) in [(80u16, 24u16), (31, 10), (200, 50), (40, 12), (80, 24)] {
            terminal.backend_mut().resize(w, h);
            terminal.draw(|frame| render(frame, &a)).unwrap();
        }
    }
}
