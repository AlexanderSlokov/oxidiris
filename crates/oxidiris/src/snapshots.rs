//! Golden frames for the whole screen.
//!
//! Implements OXD-036. See spec §9.1.
//!
//! Every other UI test asserts one property of one widget. These assert the *composition*: what a
//! reader actually sees at a given terminal size, in a given mode, with a given colour budget.
//! They are the tests that catch a layout regression nobody thought to write an assertion for.
//!
//! Two grids are captured per frame. The text grid is what the terminal prints. The style grid
//! records how each cell is emphasised, because the accessibility guarantees in §3.4.1 are about
//! attributes, not characters: an ORP that silently stopped being bold would pass a text-only
//! snapshot.
//!
//! Run `cargo insta review` after an intentional layout change; a diff here is a question, not a
//! failure.

use oxidiris_core::{PacingMode, parser, segment};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Modifier, Style};

use crate::app::App;
use crate::keymap::Action;
use crate::term::{Capabilities, ColorLevel};
use crate::ui;

/// A document with one of everything the layout has to place: headings at two levels, a paragraph
/// long enough to wrap, a list, and code.
const DOC: &str = "\
# Oxidiris

Reading at speed without moving your eyes, which needs a paragraph long enough to wrap.

## Keys

- Space plays and pauses
- Tab focuses the text panel

```rust
let anchor = width / 2;
```

## Limits

Skimming and triage, not a replacement for careful reading.
";

fn app(width: u16, height: u16, caps: Capabilities) -> App {
    let doc = parser::parse_markdown(&segment::sanitize(DOC));
    let mut a = App::new(doc, "oxidiris.md".into(), 300, PacingMode::Natural);
    a.caps = caps;
    a.player.seek_words(9);
    a.relayout(width, height);
    a
}

/// Colourless and glyph-less: the terminal a screen reader user or a `NO_COLOR` user has.
fn plain_caps() -> Capabilities {
    Capabilities { color: ColorLevel::None, unicode: false }
}

fn buffer_of(a: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, a)).unwrap();
    terminal.backend().buffer().clone()
}

/// The characters on screen, one row per line.
fn text_grid(a: &App, width: u16, height: u16) -> String {
    let buffer = buffer_of(a, width, height);
    (0..height)
        .map(|y| {
            let row: String =
                (0..width).filter_map(|x| buffer.cell((x, y))).map(|c| c.symbol()).collect();
            row.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How each cell is emphasised: `r`eversed, `b`old, `u`nderlined, `c`oloured, `.` plain.
///
/// Reversed wins over bold and bold over underline, so one character can stand for a cell.
fn style_grid(a: &App, width: u16, height: u16) -> String {
    let buffer = buffer_of(a, width, height);
    (0..height)
        .map(|y| {
            let row: String = (0..width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|cell| classify(cell.style()))
                .collect();
            row.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn classify(style: Style) -> char {
    let m = style.add_modifier;
    if m.contains(Modifier::REVERSED) {
        'r'
    } else if m.contains(Modifier::BOLD) && m.contains(Modifier::UNDERLINED) {
        'A' // the ORP anchor: bold *and* underlined, never one alone (§3.4.1)
    } else if m.contains(Modifier::BOLD) {
        'b'
    } else if m.contains(Modifier::UNDERLINED) {
        'u'
    } else if has_colour(style) {
        'c'
    } else {
        '.'
    }
}

/// Whether a style asks for an actual colour. `Reset` is the terminal's own foreground, which
/// every untouched cell carries, so it does not count.
fn has_colour(style: Style) -> bool {
    matches!(style.fg, Some(c) if c != ratatui::style::Color::Reset)
        || matches!(style.bg, Some(c) if c != ratatui::style::Color::Reset)
}

#[test]
fn split_view_at_eighty_by_twenty_four() {
    let a = app(80, 24, Capabilities::default());
    insta::assert_snapshot!(text_grid(&a, 80, 24));
}

#[test]
fn split_view_at_two_hundred_by_fifty() {
    let a = app(200, 50, Capabilities::default());
    insta::assert_snapshot!(text_grid(&a, 200, 50));
}

/// Forty columns is below the split threshold: the panel goes away rather than being squeezed.
#[test]
fn focus_mode_at_forty_by_ten() {
    let a = app(40, 10, Capabilities::default());
    insta::assert_snapshot!(text_grid(&a, 40, 10));
}

#[test]
fn focus_mode_when_the_reader_asked_for_it() {
    let mut a = app(80, 24, Capabilities::default());
    a.split = false;
    a.relayout(80, 24);
    insta::assert_snapshot!(text_grid(&a, 80, 24));
}

#[test]
fn the_help_popup() {
    let mut a = app(80, 24, Capabilities::default());
    a.handle(Action::ToggleHelp);
    insta::assert_snapshot!(text_grid(&a, 80, 24));
}

#[test]
fn the_help_popup_lists_panel_keys_while_browsing() {
    let mut a = app(80, 24, Capabilities::default());
    a.handle(Action::FocusPanel);
    a.handle(Action::ToggleHelp);
    insta::assert_snapshot!(text_grid(&a, 80, 24));
}

#[test]
fn the_outline_sidebar() {
    let mut a = app(100, 24, Capabilities::default());
    a.handle(Action::ToggleOutline);
    insta::assert_snapshot!(text_grid(&a, 100, 24));
}

#[test]
fn the_outline_over_a_narrow_window() {
    let mut a = app(60, 20, Capabilities::default());
    a.handle(Action::ToggleOutline);
    insta::assert_snapshot!(text_grid(&a, 60, 20));
}

#[test]
fn review_mode() {
    let mut a = app(80, 24, Capabilities::default());
    a.handle(Action::ToggleReview);
    insta::assert_snapshot!(text_grid(&a, 80, 24));
}

/// Under `NO_COLOR` the frame must lose colour and box glyphs, and nothing else.
#[test]
fn a_monochrome_terminal_still_shows_everything() {
    let a = app(80, 24, plain_caps());
    insta::assert_snapshot!(text_grid(&a, 80, 24));
}

/// The accessibility contract, made visible: `A` marks a cell that is bold *and* underlined, `r` a
/// reversed one. The anchor and the panel highlight must both survive with no colour at all.
#[test]
fn emphasis_survives_a_monochrome_terminal() {
    let a = app(80, 24, plain_caps());
    insta::assert_snapshot!(style_grid(&a, 80, 24));
}

#[test]
fn emphasis_with_colour_available() {
    let a = app(80, 24, Capabilities::default());
    insta::assert_snapshot!(style_grid(&a, 80, 24));
}

/// Nothing may reach the terminal as colour when the reader asked for none (spec §3.4.4).
#[test]
fn no_colour_is_emitted_under_no_color() {
    let a = app(80, 24, plain_caps());
    let buffer = buffer_of(&a, 80, 24);
    for y in 0..24 {
        for x in 0..80 {
            let cell = buffer.cell((x, y)).unwrap();
            assert!(!has_colour(cell.style()), "cell ({x},{y}) is coloured: {:?}", cell.style());
        }
    }
}

/// The one invariant: the ORP column does not move, whatever the word is (spec §3.1.3).
///
/// The rows are printed one under another so a drifting anchor is visible as a staircase.
#[test]
fn the_anchor_column_holds_across_word_lengths() {
    let mut a = app(80, 24, Capabilities::default());
    let mut frames = String::new();
    for _ in 0..8 {
        let word = a.current_word().unwrap_or("").to_string();
        let grid = text_grid(&a, 80, 24);
        let lines: Vec<&str> = grid.lines().collect();
        let marker = lines.iter().position(|l| l.contains('\u{25bc}')).expect("no anchor marker");
        // Only the reader pane; the panel beside it changes for unrelated reasons.
        let row: String = lines[marker + 1].chars().take(40).collect();
        frames.push_str(&format!("{word:>12} |{}|\n", row.trim_end()));
        a.player.advance();
    }
    insta::assert_snapshot!(frames);
}
