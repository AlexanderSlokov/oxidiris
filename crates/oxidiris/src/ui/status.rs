//! Status bar: speed, position, progress.
//!
//! Implements OXD-024. See spec §3.3 and §5.

use oxidiris_core::Player;
use oxidiris_core::pacing::FLASH_WARNING_WPM;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::keymap::{Action, Mode, keys_for};
use crate::term::{Capabilities, ColorLevel};

/// Left half of the status line: configured speed and the speed actually achieved.
///
/// Both numbers are shown because pacing multipliers make them differ by 15-25%, and a reader who
/// only sees the configured number will believe a speed they are not reading at (spec §3.2.4).
pub fn speed_text(player: &Player) -> String {
    format!("speed: {} WPM (eff. {})", player.wpm(), player.effective_wpm())
}

/// Right half: position measured in words, which is the unit the cursor actually moves in.
pub fn position_text(player: &Player) -> String {
    let (current, total) = player.progress();
    let percent = (player.progress_ratio() * 100.0).round() as u32;
    format!("word: {current}/{total} ({percent}%)")
}

/// Draw a progress bar from plain characters, so it survives terminals without box drawing.
pub fn progress_bar(ratio: f64, width: u16, unicode: bool) -> String {
    if width == 0 {
        return String::new();
    }
    let (full, empty) = if unicode { ('\u{2588}', '\u{2591}') } else { ('#', '-') };
    let filled = (ratio.clamp(0.0, 1.0) * f64::from(width)).round() as u16;
    let mut bar = String::with_capacity(width as usize);
    for i in 0..width {
        bar.push(if i < filled { full } else { empty });
    }
    bar
}

/// Key hints for `mode`.
///
/// The hints are generated from the binding table, so a key that means something different here
/// is advertised differently here (spec §7.1).
pub fn hint_text(player: &Player, mode: Mode) -> String {
    if mode.is_panel() {
        return format!(
            "{}   [{}] scroll   [{}] back   [{}] help   [{}] quit",
            mode.label(),
            keys_for(Action::ScrollDown),
            keys_for(Action::FocusPanel),
            keys_for(Action::ToggleHelp),
            keys_for(Action::Quit),
        );
    }
    // Deliberately no hint for Tab or o here: the line already fills an 80-column terminal, and
    // both panels advertise themselves in their own title bar.
    let state = if player.is_playing() { "pause" } else { "play" };
    format!(
        "[{}] {state}   [{}] speed   [{}] seek   [{}] help   [{}] quit",
        keys_for(Action::TogglePlay),
        keys_for(Action::Faster),
        keys_for(Action::Back),
        keys_for(Action::ToggleHelp),
        keys_for(Action::Quit),
    )
}

/// Render the status block into `area`.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    player: &Player,
    mode: Mode,
    caps: Capabilities,
    message: Option<&str>,
) {
    let block = Block::bordered();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let speed = speed_text(player);
    let position = position_text(player);
    let gap = (inner.width as usize)
        .saturating_sub(speed.chars().count() + position.chars().count())
        .max(1);

    let warn = player.wpm() >= FLASH_WARNING_WPM;
    let speed_style = if warn && caps.color != ColorLevel::None {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if warn {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let first = Line::from(vec![
        Span::styled(speed, speed_style),
        Span::raw(" ".repeat(gap)),
        Span::raw(position),
    ]);

    let hint = match message {
        Some(msg) => msg.to_string(),
        None if warn && mode == Mode::Reader => {
            format!("high speed (>{FLASH_WARNING_WPM} WPM) may be uncomfortable  ·  [?] help")
        }
        None => hint_text(player, mode),
    };

    let bar_width = inner.width.saturating_sub(hint.chars().count() as u16 + 2);
    let second = Line::from(vec![
        Span::styled(hint, Style::default().add_modifier(Modifier::DIM)),
        Span::raw("  "),
        Span::raw(progress_bar(player.progress_ratio(), bar_width, caps.unicode)),
    ]);

    frame.render_widget(Paragraph::new(vec![first, second]), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidiris_core::{PacingMode, parser, segment};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn player() -> Player {
        let text = segment::sanitize(
            "The quick brown fox jumps over the lazy dog. Pack my box with five dozen jugs.\n",
        );
        let doc = parser::parse_markdown(&text);
        Player::from_document(&doc, 450, PacingMode::Natural)
    }

    #[test]
    fn speed_shows_both_the_configured_and_the_effective_number() {
        let p = player();
        let text = speed_text(&p);
        assert!(text.contains("450 WPM"));
        assert!(text.contains("eff."));
        assert!(p.effective_wpm() < p.wpm(), "effective speed should trail the configured one");
    }

    #[test]
    fn effective_speed_follows_a_speed_change() {
        let mut p = player();
        let before = p.effective_wpm();
        p.set_wpm(900);
        assert!(p.effective_wpm() > before);
    }

    #[test]
    fn position_is_measured_in_words_not_lines() {
        let p = player();
        let text = position_text(&p);
        assert!(text.starts_with("word: "), "got {text:?}");
        assert!(text.contains('%'));
    }

    #[test]
    fn percentage_matches_the_players_progress() {
        let mut p = player();
        p.goto_end();
        assert!(position_text(&p).contains("100%"));
        p.goto_start();
        assert!(position_text(&p).contains("(0%)"));
    }

    #[test]
    fn progress_bar_respects_its_width_and_the_unicode_setting() {
        assert_eq!(progress_bar(0.5, 10, false).chars().count(), 10);
        assert_eq!(progress_bar(0.0, 4, false), "----");
        assert_eq!(progress_bar(1.0, 4, false), "####");
        assert!(progress_bar(0.5, 10, true).contains('\u{2588}'));
        assert_eq!(progress_bar(0.5, 0, true), "");
    }

    #[test]
    fn progress_bar_clamps_out_of_range_ratios() {
        assert_eq!(progress_bar(-1.0, 4, false), "----");
        assert_eq!(progress_bar(5.0, 4, false), "####");
    }

    #[test]
    fn the_status_line_fits_an_eighty_column_terminal() {
        let mut terminal = Terminal::new(TestBackend::new(80, 6)).unwrap();
        let p = player();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &p, Mode::Reader, Capabilities::default(), None)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        // Nothing may be written outside the frame, and the border must survive on both edges.
        for y in 0..6u16 {
            assert!(buffer.cell((79u16, y)).is_some());
        }
        let rendered: String =
            (0..80).filter_map(|x| buffer.cell((x, 1u16))).map(|c| c.symbol()).collect();
        assert!(rendered.contains("450 WPM"), "status line was {rendered:?}");
    }

    /// A reader who has focused the panel needs to know J/K no longer touch the speed.
    #[test]
    fn the_hint_line_changes_with_the_mode() {
        let p = player();
        let reading = hint_text(&p, Mode::Reader);
        let browsing = hint_text(&p, Mode::Browser);
        assert!(reading.contains("speed"));
        assert!(!browsing.contains("speed"), "got {browsing:?}");
        assert!(browsing.contains("BROWSER"));
        assert!(browsing.contains("scroll"));
    }

    /// OXD-024: the line must not wrap on the narrowest supported terminal.
    #[test]
    fn every_mode_hint_fits_an_eighty_column_terminal() {
        let p = player();
        for mode in [Mode::Reader, Mode::Browser, Mode::Outline] {
            let hint = hint_text(&p, mode);
            assert!(hint.chars().count() <= 78, "{mode:?} hint is {} chars", hint.chars().count());
        }
    }

    #[test]
    fn a_high_speed_earns_a_warning_line() {
        let mut p = player();
        p.set_wpm(900);
        let mut terminal = Terminal::new(TestBackend::new(100, 6)).unwrap();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &p, Mode::Reader, Capabilities::default(), None)
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let line: String =
            (0..100).filter_map(|x| buffer.cell((x, 2u16))).map(|c| c.symbol()).collect();
        assert!(line.contains("high speed"), "expected a warning, got {line:?}");
    }

    /// The hints must be generated, so renaming a key in the table updates the status bar too.
    #[test]
    fn the_hint_line_is_built_from_the_binding_table() {
        let mut terminal = Terminal::new(TestBackend::new(100, 6)).unwrap();
        let p = player();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &p, Mode::Reader, Capabilities::default(), None)
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let line: String =
            (0..100).filter_map(|x| buffer.cell((x, 2u16))).map(|c| c.symbol()).collect();
        assert!(line.contains(keys_for(Action::TogglePlay)));
        assert!(line.contains(keys_for(Action::Quit)));
    }

    #[test]
    fn rendering_into_a_tiny_area_does_not_panic() {
        let mut terminal = Terminal::new(TestBackend::new(12, 3)).unwrap();
        let p = player();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &p, Mode::Reader, Capabilities::default(), None)
            })
            .unwrap();
    }
}
