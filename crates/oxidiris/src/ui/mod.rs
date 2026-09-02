//! Screen composition.

pub mod help;
pub mod outline;
pub mod panel;
pub mod review;
pub mod rsvp;
pub mod status;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph};

use crate::app::App;
use crate::term::{ABS_MIN_HEIGHT, ABS_MIN_WIDTH, SizeClass, size_class};

/// Rows reserved for the status block, including its border.
const STATUS_HEIGHT: u16 = 4;
/// Narrowest useful outline sidebar.
const OUTLINE_MIN_WIDTH: u16 = 16;
/// Widest the outline may grow; past this it starves the reader frame.
const OUTLINE_MAX_WIDTH: u16 = 24;

/// Where each part of the screen goes.
///
/// Computed as a pure function so the event loop can ask for the panel's width *before* drawing,
/// and re-wrap the document only when that width actually changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panes {
    /// Outline sidebar, when it has a column of its own.
    pub outline: Option<Rect>,
    /// The RSVP frame. Always present.
    pub reader: Rect,
    /// Full-text panel, when the window is wide enough for it.
    pub panel: Option<Rect>,
    /// Status and progress block, always full width.
    pub status: Rect,
}

/// Split `area` into panes.
///
/// > **Deviation from spec §5,** which drew the status block inside the left column. It is kept
/// > full width instead; at 80 columns a half-width status line cannot hold both the speed and the
/// > position without clipping one of them. Recorded in `docs/decisions/split-view-layout.md`.
pub fn panes(area: Rect, outline: bool, panel: bool) -> Panes {
    let [top, status] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(STATUS_HEIGHT)]).areas(area);

    let (outline_area, rest) = if outline {
        let width = (top.width / 5).clamp(OUTLINE_MIN_WIDTH, OUTLINE_MAX_WIDTH);
        let [o, r] = Layout::horizontal([Constraint::Length(width), Constraint::Min(0)]).areas(top);
        (Some(o), r)
    } else {
        (None, top)
    };

    let (reader, panel_area) = if panel {
        let [l, r] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(rest);
        (l, Some(r))
    } else {
        (rest, None)
    };

    Panes { outline: outline_area, reader, panel: panel_area, status }
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

/// Draw the whole screen.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if size_class(area.width, area.height) == SizeClass::TooSmall {
        render_too_small(frame, area);
        return;
    }

    let panes = panes(area, app.outline_docked(), app.panel_visible());

    if let Some(outline_area) = panes.outline {
        outline::render(
            frame,
            outline_area,
            &app.doc.headings,
            app.outline_selected,
            app.active_heading(),
            app.mode == crate::keymap::Mode::Outline,
            app.caps,
        );
    }

    rsvp::render(frame, panes.reader, app.current_word(), &app.title, app.caps);

    if let Some(panel_area) = panes.panel {
        let view = panel::View {
            source: &app.doc.source,
            rows: &app.panel.rows,
            highlight: app.current_span(),
            scroll: app.panel_scroll(),
            focused: app.mode == crate::keymap::Mode::Browser,
            caps: app.caps,
        };
        panel::render(frame, panel_area, &view);
    }

    status::render(frame, panes.status, &app.player, app.mode, app.caps, app.message.as_deref());

    // Too narrow for a sidebar column, so the outline takes over the reader instead. Navigating is
    // a moment of its own; playback is paused while it is open.
    if app.outline_overlaid() {
        let popup = centered(area, area.width * 3 / 4, area.height.saturating_sub(4));
        frame.render_widget(Clear, popup);
        outline::render(
            frame,
            popup,
            &app.doc.headings,
            app.outline_selected,
            app.active_heading(),
            true,
            app.caps,
        );
    }

    let reviewing = if app.show_review { app.current_block_span() } else { None };
    if let Some(span) = reviewing {
        review::render(frame, area, &app.doc.source, span, app.review_scroll);
    }

    if app.show_help {
        help::render(frame, area, app.mode, app.help_scroll);
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
    use crate::keymap::Action;
    use oxidiris_core::{PacingMode, parser, segment};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const DOC: &str = "\
# Title

Some words to read here quickly, enough of them to wrap a narrow panel.

## Second section

More words follow in a second paragraph.
";

    fn app(width: u16, height: u16) -> App {
        let doc = parser::parse_markdown(&segment::sanitize(DOC));
        let mut a = App::new(doc, "test.md".into(), 300, PacingMode::Natural);
        a.relayout(width, height);
        a
    }

    fn draw(width: u16, height: u16, a: &App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, a)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..width).filter_map(|x| buffer.cell((x, y))).map(|c| c.symbol()).collect())
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn the_full_screen_renders_at_every_supported_size() {
        for (w, h) in [(30u16, 10u16), (40, 12), (80, 24), (120, 40), (200, 60)] {
            draw(w, h, &app(w, h));
        }
    }

    #[test]
    fn a_window_below_the_absolute_minimum_asks_to_be_enlarged() {
        let out = draw(20, 6, &app(20, 6));
        assert!(out.contains("too small"), "expected a size prompt, got {out}");
    }

    #[test]
    fn a_wide_window_shows_the_reader_and_the_text_panel_side_by_side() {
        let out = draw(100, 24, &app(100, 24));
        assert!(out.contains("Title"), "the document text is missing: {out}");
        assert!(out.contains("Text"), "the panel frame is missing: {out}");
    }

    /// Below 80 columns the panel is dropped rather than squeezed (spec §3.4.4).
    #[test]
    fn a_narrow_window_drops_the_panel() {
        let out = draw(60, 20, &app(60, 20));
        assert!(!out.contains("Text  ·"), "the panel should be hidden at 60 columns: {out}");
    }

    #[test]
    fn the_outline_takes_a_column_on_a_wide_window() {
        let mut a = app(120, 30);
        a.handle(Action::ToggleOutline);
        let out = draw(120, 30, &a);
        assert!(out.contains("Outline"), "got {out}");
        assert!(out.contains("Second section"), "the headings are missing: {out}");
    }

    #[test]
    fn the_outline_covers_the_reader_on_a_narrow_window() {
        let mut a = app(60, 20);
        a.handle(Action::ToggleOutline);
        let out = draw(60, 20, &a);
        assert!(out.contains("Outline"), "got {out}");
    }

    #[test]
    fn review_mode_draws_over_everything_else() {
        let mut a = app(100, 24);
        a.handle(Action::ToggleReview);
        let out = draw(100, 24, &a);
        assert!(out.contains("Review"), "got {out}");
    }

    #[test]
    fn the_help_overlay_draws_on_top_of_the_reader() {
        let mut a = app(80, 24);
        a.handle(Action::ToggleHelp);
        let out = draw(80, 24, &a);
        assert!(out.contains("Play / pause"));
    }

    #[test]
    fn panes_leave_no_gap_and_never_overlap() {
        let area = Rect { x: 0, y: 0, width: 120, height: 40 };
        let p = panes(area, true, true);
        let outline = p.outline.unwrap();
        let panel = p.panel.unwrap();
        assert_eq!(outline.x, 0);
        assert_eq!(outline.right(), p.reader.x);
        assert_eq!(p.reader.right(), panel.x);
        assert_eq!(panel.right(), area.right());
        assert_eq!(p.status.width, area.width);
        assert_eq!(p.reader.bottom(), p.status.y);
    }

    #[test]
    fn the_outline_column_never_starves_the_reader() {
        for width in [80u16, 100, 200, 400] {
            let p = panes(Rect { x: 0, y: 0, width, height: 40 }, true, true);
            let outline = p.outline.unwrap();
            assert!(outline.width >= OUTLINE_MIN_WIDTH);
            assert!(outline.width <= OUTLINE_MAX_WIDTH);
            assert!(p.reader.width > outline.width / 2, "reader squeezed at {width} columns");
        }
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
        let headings = doc.headings.len();
        let mut a = App::new(doc, "BACKLOG.md".into(), 400, PacingMode::Natural);
        a.player.seek_ratio(0.0135);
        a.relayout(80, 24);

        println!("\n{}\n", draw(80, 24, &a));
        println!("current word: {:?}", a.current_word());
        println!("tokens: {}", a.player.tokens().len());
        println!("headings: {headings}");
    }

    #[test]
    fn resizing_repeatedly_keeps_the_layout_intact() {
        let mut a = app(80, 24);
        for (w, h) in [(80u16, 24u16), (31, 10), (200, 50), (40, 12), (80, 24)] {
            a.relayout(w, h);
            draw(w, h, &a);
        }
    }
}
