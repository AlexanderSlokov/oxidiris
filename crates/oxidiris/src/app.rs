//! Application state and the key dispatcher.

use core::ops::Range;

use oxidiris_core::player::{SEEK_WORDS, WPM_STEP_COARSE, WPM_STEP_FINE};
use oxidiris_core::{Document, PacingMode, PlayState, Player};

use crate::keymap::{Action, Mode};
use crate::term::{Capabilities, MIN_WIDTH, SizeClass, size_class};
use crate::ui;
use crate::ui::{outline, panel};

/// The full-text panel's wrapped view of the document.
///
/// Wrapping is cached rather than redone per frame: at 1 500 WPM the screen redraws 25 times a
/// second, and re-wrapping a 50 KB document that often is work nobody asked for.
#[derive(Debug, Default)]
pub struct PanelView {
    /// Byte ranges into [`Document::source`], one per display row.
    pub rows: Vec<Range<usize>>,
    /// Inner width the rows were wrapped to.
    pub width: u16,
    /// Inner height last seen, used to clamp scrolling.
    pub height: u16,
    /// First visible row while the reader is scrolling by hand.
    pub scroll: u16,
}

/// Move a scroll offset by `delta`, keeping at least one row of content on screen.
fn step(current: u16, delta: isize, rows: usize, viewport: u16) -> u16 {
    let last = rows.saturating_sub(usize::from(viewport.max(1)));
    let next = (current as isize + delta).clamp(0, u16::MAX as isize) as usize;
    next.min(last) as u16
}

/// Everything the renderer needs to draw a frame.
pub struct App {
    /// Playback cursor over the token stream.
    pub player: Player,
    /// The parsed document, kept for the panel, the outline and Review Mode.
    pub doc: Document,
    /// Title shown on the reader frame, normally the file name.
    pub title: String,
    /// What the terminal can render.
    pub caps: Capabilities,
    /// Which panel owns the keyboard (spec §7.1).
    pub mode: Mode,
    /// Whether the reader asked for the split layout at all (`--mode tui`).
    pub split: bool,
    /// Whether the outline sidebar is open.
    pub show_outline: bool,
    /// Whether Review Mode is open.
    pub show_review: bool,
    /// Whether the key reference is open.
    pub show_help: bool,
    /// Scroll offset inside the key reference.
    pub help_scroll: u16,
    /// Scroll offset inside Review Mode.
    pub review_scroll: u16,
    /// Highlighted entry in the outline.
    pub outline_selected: usize,
    /// Wrapped view of the document for the full-text panel.
    pub panel: PanelView,
    /// Transient note shown in place of the key hints.
    pub message: Option<String>,
    /// Set once the reader has asked to leave.
    pub should_quit: bool,
    /// Last known terminal size, from [`App::relayout`].
    size: (u16, u16),
    /// Whether playback was interrupted by a popup and should pick up again afterwards.
    resume_when_done: bool,
}

impl App {
    /// Build an application over a parsed document.
    pub fn new(doc: Document, title: String, wpm: u16, pacing: PacingMode) -> Self {
        App {
            player: Player::from_document(&doc, wpm, pacing),
            doc,
            title,
            caps: Capabilities::default(),
            mode: Mode::Reader,
            split: true,
            show_outline: false,
            show_review: false,
            show_help: false,
            help_scroll: 0,
            review_scroll: 0,
            outline_selected: 0,
            panel: PanelView::default(),
            message: None,
            should_quit: false,
            size: (0, 0),
            resume_when_done: false,
        }
    }

    /// Detect terminal capabilities from the environment.
    pub fn with_detected_capabilities(mut self) -> Self {
        self.caps = Capabilities::detect();
        self
    }

    /// Attach a note to show in the status bar.
    pub fn with_message(mut self, message: Option<String>) -> Self {
        self.message = message;
        self
    }

    /// Choose between the split layout and the bare reader frame (`--mode`).
    pub fn with_split(mut self, split: bool) -> Self {
        self.split = split;
        self
    }

    /// Recompute the layout for a terminal of this size, re-wrapping the panel if it changed.
    ///
    /// Called before every draw. Cheap when nothing moved: the wrap is redone only when the
    /// panel's inner width actually differs.
    pub fn relayout(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        let panes = ui::panes(
            ratatui::layout::Rect { x: 0, y: 0, width, height },
            self.outline_docked(),
            self.panel_visible(),
        );
        let Some(area) = panes.panel else { return };
        let inner = (area.width.saturating_sub(2), area.height.saturating_sub(2));
        if inner.0 != self.panel.width || self.panel.rows.is_empty() {
            self.panel.rows = panel::wrap(&self.doc.source, inner.0);
            self.panel.width = inner.0;
        }
        self.panel.height = inner.1;
        self.panel.scroll =
            panel::clamp_scroll(self.panel.scroll, self.panel.height, self.panel.rows.len());
    }

    /// Whether the full-text panel is on screen.
    ///
    /// The test is on *width* alone: spec §3.4.4 drops the panel below 80 columns, and a short
    /// window is no reason to take it away — a 100x20 terminal has plenty of room beside the
    /// reader frame, just not much above it.
    pub fn panel_visible(&self) -> bool {
        self.split && self.has_room()
    }

    /// Whether the outline has room for its own column, rather than covering the reader.
    pub fn outline_docked(&self) -> bool {
        self.show_outline && self.has_room()
    }

    /// Whether the window is wide enough for a second column beside the reader.
    fn has_room(&self) -> bool {
        self.size.0 >= MIN_WIDTH && size_class(self.size.0, self.size.1) != SizeClass::TooSmall
    }

    /// Whether the outline has to be drawn over the reader instead of beside it.
    pub fn outline_overlaid(&self) -> bool {
        self.show_outline && !self.outline_docked()
    }

    /// The word currently on screen.
    pub fn current_word(&self) -> Option<&str> {
        self.player.current().map(|t| t.text.as_str())
    }

    /// Source range of the word currently on screen, for the panel highlight.
    pub fn current_span(&self) -> Option<Range<usize>> {
        self.player.current().map(|t| t.byte_span.clone())
    }

    /// Source range of the block the cursor is in, for Review Mode.
    pub fn current_block_span(&self) -> Option<Range<usize>> {
        let block_id = self.player.current()?.block_id;
        self.doc.blocks.iter().find(|b| b.id == block_id).map(|b| b.byte_span.clone())
    }

    /// Outline entry whose section contains the cursor.
    pub fn active_heading(&self) -> Option<usize> {
        let block_id = self.player.current()?.block_id;
        outline::active_index(&self.doc.headings, block_id)
    }

    /// First panel row to draw.
    ///
    /// While the panel has focus the reader owns the scroll; otherwise it follows the cursor,
    /// which is the behaviour that keeps them oriented (OXD-031 acceptance).
    pub fn panel_scroll(&self) -> u16 {
        if self.mode == Mode::Browser {
            return self.panel.scroll;
        }
        let Some(span) = self.current_span() else { return 0 };
        let Some(row) = panel::row_of(&self.panel.rows, span.start) else { return 0 };
        panel::auto_scroll(row, self.panel.height, self.panel.rows.len())
    }

    /// Apply an action.
    pub fn handle(&mut self, action: Action) {
        // The message is a response to the last thing that happened, so any new input clears it.
        if !matches!(action, Action::ToggleHelp | Action::Escape) {
            self.message = None;
        }

        // An open popup owns the movement keys while it is up: `J`/`K` scroll it rather than
        // changing a speed the reader cannot currently see the effect of.
        if self.show_help || self.show_review {
            match action {
                Action::Faster | Action::ScrollUp => return self.scroll_popup(-1),
                Action::Slower | Action::ScrollDown => return self.scroll_popup(1),
                _ => {}
            }
        }

        match action {
            Action::TogglePlay => self.toggle_play(),
            Action::Restart => self.player.restart(),
            Action::Faster => self.player.adjust_wpm(i32::from(WPM_STEP_COARSE)),
            Action::Slower => self.player.adjust_wpm(-i32::from(WPM_STEP_COARSE)),
            Action::FasterFine => self.player.adjust_wpm(i32::from(WPM_STEP_FINE)),
            Action::SlowerFine => self.player.adjust_wpm(-i32::from(WPM_STEP_FINE)),
            Action::Back => self.player.seek_words(-(SEEK_WORDS as isize)),
            Action::Forward => self.player.seek_words(SEEK_WORDS as isize),
            Action::PrevBlock => self.player.seek_blocks(-1),
            Action::NextBlock => self.player.seek_blocks(1),
            Action::GotoStart => self.player.goto_start(),
            Action::GotoEnd => self.player.goto_end(),
            Action::FocusPanel => self.focus_panel(),
            Action::ToggleOutline => self.toggle_outline(),
            Action::ToggleReview => self.toggle_review(),
            Action::ScrollUp => self.scroll(-1),
            Action::ScrollDown => self.scroll(1),
            Action::Select => self.select(),
            Action::ToggleHelp => self.toggle_help(),
            Action::Escape => self.escape(),
            Action::Quit => self.should_quit = true,
        }
    }

    /// Space always means "get back to reading", wherever it is pressed.
    fn toggle_play(&mut self) {
        self.show_help = false;
        self.show_review = false;
        self.mode = Mode::Reader;
        self.resume_when_done = false;
        self.player.toggle();
    }

    fn focus_panel(&mut self) {
        if self.mode.is_panel() {
            self.mode = Mode::Reader;
            self.resume();
            return;
        }
        if !self.panel_visible() {
            self.message = Some("the text panel needs an 80-column window".into());
            return;
        }
        // Seed the manual scroll from where the panel already is, so focusing it does not jump.
        self.panel.scroll = self.panel_scroll();
        self.mode = Mode::Browser;
        self.suspend();
    }

    fn toggle_outline(&mut self) {
        if self.show_outline {
            self.show_outline = false;
            if self.mode == Mode::Outline {
                self.mode = Mode::Reader;
                self.resume();
            }
            return;
        }
        self.show_outline = true;
        self.mode = Mode::Outline;
        self.outline_selected = self.active_heading().unwrap_or(0);
        self.suspend();
    }

    fn toggle_review(&mut self) {
        if self.show_review {
            self.show_review = false;
            self.resume();
            return;
        }
        if self.current_block_span().is_none() {
            self.message = Some("nothing to review yet".into());
            return;
        }
        self.show_review = true;
        self.review_scroll = 0;
        self.suspend();
    }

    fn scroll(&mut self, delta: isize) {
        match self.mode {
            Mode::Browser => {
                let next = (self.panel.scroll as isize + delta).clamp(0, u16::MAX as isize);
                self.panel.scroll =
                    panel::clamp_scroll(next as u16, self.panel.height, self.panel.rows.len());
            }
            Mode::Outline => {
                let last = self.doc.headings.len().saturating_sub(1);
                let next = self.outline_selected as isize + delta;
                self.outline_selected = next.clamp(0, last as isize) as usize;
            }
            Mode::Reader => {}
        }
    }

    /// Move the open popup's viewport, clamped to its content.
    fn scroll_popup(&mut self, delta: isize) {
        if self.show_help {
            let rows = crate::ui::help::lines(self.mode).len();
            self.help_scroll = step(self.help_scroll, delta, rows, self.size.1.saturating_sub(2));
            return;
        }
        let Some(span) = self.current_block_span() else { return };
        // Mirrors the popup geometry in `ui::review`; both derive from the terminal size.
        let width = (self.size.0 * 6 / 8).saturating_sub(2);
        let height = (self.size.1 * 5 / 8).max(3).saturating_sub(2);
        let rows = crate::ui::review::row_count(&self.doc.source, span, width);
        self.review_scroll = step(self.review_scroll, delta, rows, height);
    }

    fn select(&mut self) {
        let Some(heading) = self.doc.headings.get(self.outline_selected) else {
            self.message = Some("this document has no headings".into());
            return;
        };
        if !self.player.seek_block_id(heading.block_id) {
            self.message = Some(format!("nothing readable under {:?}", heading.text));
            return;
        }
        self.mode = Mode::Reader;
        self.resume();
    }

    fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        self.help_scroll = 0;
        // Opening the reference stops the stream; reading and reading about reading do not mix.
        if self.show_help {
            self.suspend();
        } else {
            self.resume();
        }
    }

    /// Esc peels off one layer at a time: popup, then sub-mode, then nothing (spec §7.2).
    fn escape(&mut self) {
        if self.show_help {
            self.show_help = false;
        } else if self.show_review {
            self.show_review = false;
        } else if self.mode.is_panel() {
            self.mode = Mode::Reader;
        } else {
            return;
        }
        self.resume();
    }

    /// Hold playback while something else has the reader's attention.
    fn suspend(&mut self) {
        if self.player.is_playing() {
            self.resume_when_done = true;
            self.player.pause();
        }
    }

    /// Pick playback up again, but only once every distraction is gone.
    fn resume(&mut self) {
        let busy = self.show_help || self.show_review || self.mode.is_panel();
        if !self.resume_when_done || busy {
            return;
        }
        self.resume_when_done = false;
        // Finished means the cursor is parked on the last token; playing again would silently
        // restart the document, which is not what closing a popup should do.
        if self.player.state() == PlayState::Paused {
            self.player.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidiris_core::{parser, segment};

    const DOC: &str = "\
# Heading one

First paragraph with a handful of words in it.

## Heading two

Second paragraph, also with several words.
";

    fn app() -> App {
        let doc = parser::parse_markdown(&segment::sanitize(DOC));
        let mut a = App::new(doc, "test.md".into(), 300, PacingMode::Natural);
        a.relayout(120, 40);
        a
    }

    fn narrow_app() -> App {
        let doc = parser::parse_markdown(&segment::sanitize(DOC));
        let mut a = App::new(doc, "test.md".into(), 300, PacingMode::Natural);
        a.relayout(60, 20);
        a
    }

    #[test]
    fn play_and_pause_toggle() {
        let mut a = app();
        assert_eq!(a.player.state(), PlayState::Paused);
        a.handle(Action::TogglePlay);
        assert_eq!(a.player.state(), PlayState::Playing);
        a.handle(Action::TogglePlay);
        assert_eq!(a.player.state(), PlayState::Paused);
    }

    #[test]
    fn speed_actions_move_by_the_documented_steps() {
        let mut a = app();
        a.handle(Action::Faster);
        assert_eq!(a.player.wpm(), 300 + WPM_STEP_COARSE);
        a.handle(Action::Slower);
        assert_eq!(a.player.wpm(), 300);
        a.handle(Action::FasterFine);
        assert_eq!(a.player.wpm(), 300 + WPM_STEP_FINE);
    }

    #[test]
    fn navigation_actions_move_the_cursor() {
        let mut a = app();
        a.handle(Action::Forward);
        assert_eq!(a.player.progress().0, 1 + SEEK_WORDS);
        a.handle(Action::Back);
        assert_eq!(a.player.progress().0, 1);
        a.handle(Action::GotoEnd);
        assert_eq!(a.player.progress().0, a.player.tokens().len());
    }

    #[test]
    fn paragraph_navigation_crosses_blocks() {
        let mut a = app();
        let first = a.player.current().unwrap().block_id;
        a.handle(Action::NextBlock);
        assert!(a.player.current().unwrap().block_id > first);
    }

    // --- modes (OXD-030) ----------------------------------------------------

    #[test]
    fn tab_moves_focus_to_the_panel_and_back() {
        let mut a = app();
        assert_eq!(a.mode, Mode::Reader);
        a.handle(Action::FocusPanel);
        assert_eq!(a.mode, Mode::Browser);
        a.handle(Action::FocusPanel);
        assert_eq!(a.mode, Mode::Reader);
    }

    /// Esc leaves the sub-mode; only q leaves the program (spec §7.2).
    #[test]
    fn escape_returns_to_the_reader_without_quitting() {
        let mut a = app();
        a.handle(Action::FocusPanel);
        a.handle(Action::Escape);
        assert_eq!(a.mode, Mode::Reader);
        assert!(!a.should_quit);
    }

    #[test]
    fn focusing_a_hidden_panel_says_why_instead_of_doing_nothing() {
        let mut a = narrow_app();
        a.handle(Action::FocusPanel);
        assert_eq!(a.mode, Mode::Reader);
        assert!(a.message.as_deref().unwrap().contains("80-column"), "got {:?}", a.message);
    }

    #[test]
    fn browsing_pauses_playback_and_leaving_resumes_it() {
        let mut a = app();
        a.handle(Action::TogglePlay);
        assert!(a.player.is_playing());
        a.handle(Action::FocusPanel);
        assert!(!a.player.is_playing(), "browsing while the stream runs is unreadable");
        a.handle(Action::FocusPanel);
        assert!(a.player.is_playing(), "leaving the panel should pick reading back up");
    }

    #[test]
    fn a_reader_who_was_paused_stays_paused_after_browsing() {
        let mut a = app();
        a.handle(Action::FocusPanel);
        a.handle(Action::Escape);
        assert!(!a.player.is_playing());
    }

    #[test]
    fn space_returns_to_reading_from_anywhere() {
        let mut a = app();
        a.handle(Action::ToggleOutline);
        a.handle(Action::TogglePlay);
        assert_eq!(a.mode, Mode::Reader);
        assert!(a.player.is_playing());
    }

    // --- panel (OXD-031) ----------------------------------------------------

    #[test]
    fn the_panel_is_wrapped_to_its_own_width() {
        let a = app();
        assert!(!a.panel.rows.is_empty());
        assert!(a.panel.width > 0);
        for row in &a.panel.rows {
            assert!(a.doc.source.get(row.clone()).is_some());
        }
    }

    #[test]
    fn a_narrow_window_hides_the_panel_entirely() {
        let a = narrow_app();
        assert!(!a.panel_visible());
    }

    /// Enough paragraphs that the panel has to scroll at all.
    fn long_app() -> App {
        let text: String =
            (0..40).map(|i| format!("Paragraph number {i} with a few words in it.\n\n")).collect();
        let doc = parser::parse_markdown(&segment::sanitize(&text));
        let mut a = App::new(doc, "long.md".into(), 300, PacingMode::Natural);
        a.relayout(120, 40);
        a
    }

    #[test]
    fn the_panel_follows_the_cursor_until_the_reader_takes_over() {
        let mut a = long_app();
        a.handle(Action::GotoEnd);
        let followed = a.panel_scroll();
        assert!(followed > 0, "the panel should have scrolled to the end of the document");

        a.handle(Action::FocusPanel);
        assert_eq!(a.panel_scroll(), followed, "focusing the panel must not make it jump");
        a.handle(Action::ScrollUp);
        assert!(a.panel_scroll() < followed, "manual scrolling must win while browsing");
    }

    #[test]
    fn scrolling_is_clamped_at_both_ends() {
        let mut a = app();
        a.handle(Action::FocusPanel);
        for _ in 0..50 {
            a.handle(Action::ScrollUp);
        }
        assert_eq!(a.panel.scroll, 0);
        for _ in 0..500 {
            a.handle(Action::ScrollDown);
        }
        assert!(a.panel.scroll as usize <= a.panel.rows.len());
    }

    #[test]
    fn the_highlight_span_points_at_the_word_on_screen() {
        let a = app();
        let span = a.current_span().unwrap();
        assert_eq!(&a.doc.source[span], a.current_word().unwrap());
    }

    // --- outline (OXD-032) --------------------------------------------------

    #[test]
    fn opening_the_outline_selects_the_section_being_read() {
        let mut a = app();
        a.handle(Action::GotoEnd);
        a.handle(Action::ToggleOutline);
        assert_eq!(a.mode, Mode::Outline);
        assert_eq!(a.outline_selected, a.doc.headings.len() - 1);
    }

    #[test]
    fn selecting_a_heading_jumps_there_and_returns_to_the_reader() {
        let mut a = app();
        a.handle(Action::GotoEnd);
        a.handle(Action::ToggleOutline);
        a.handle(Action::ScrollUp);
        a.handle(Action::Select);
        assert_eq!(a.mode, Mode::Reader);
        assert_eq!(a.player.current().unwrap().text, "Heading");
    }

    #[test]
    fn the_outline_selection_is_clamped_to_the_headings() {
        let mut a = app();
        a.handle(Action::ToggleOutline);
        for _ in 0..20 {
            a.handle(Action::ScrollDown);
        }
        assert_eq!(a.outline_selected, a.doc.headings.len() - 1);
        for _ in 0..20 {
            a.handle(Action::ScrollUp);
        }
        assert_eq!(a.outline_selected, 0);
    }

    #[test]
    fn a_document_without_headings_reports_it_rather_than_jumping() {
        let doc = parser::parse_markdown("just a paragraph, no headings at all\n");
        let mut a = App::new(doc, "flat.md".into(), 300, PacingMode::Natural);
        a.relayout(120, 40);
        a.handle(Action::ToggleOutline);
        a.handle(Action::Select);
        assert!(a.message.as_deref().unwrap().contains("no headings"), "got {:?}", a.message);
    }

    #[test]
    fn a_narrow_window_puts_the_outline_over_the_reader() {
        let mut a = narrow_app();
        a.handle(Action::ToggleOutline);
        assert!(a.outline_overlaid());
        assert!(!a.outline_docked());
    }

    // --- review mode (OXD-034) ----------------------------------------------

    #[test]
    fn review_shows_the_block_under_the_cursor_and_pauses() {
        let mut a = app();
        a.handle(Action::TogglePlay);
        a.handle(Action::ToggleReview);
        assert!(a.show_review);
        assert!(!a.player.is_playing());

        let span = a.current_block_span().unwrap();
        assert!(a.doc.source[span].contains("Heading one"));
    }

    #[test]
    fn closing_review_resumes_at_the_same_position() {
        let mut a = app();
        a.handle(Action::Forward);
        let before = a.player.progress().0;
        a.handle(Action::TogglePlay);
        a.handle(Action::ToggleReview);
        a.handle(Action::Escape);
        assert!(!a.show_review);
        assert!(a.player.is_playing());
        assert_eq!(a.player.progress().0, before);
    }

    #[test]
    fn a_finished_document_is_not_restarted_by_closing_a_popup() {
        let mut a = app();
        a.handle(Action::TogglePlay);
        a.handle(Action::GotoEnd);
        while a.player.advance().is_some() {}
        assert_eq!(a.player.state(), PlayState::Finished);
        a.handle(Action::ToggleReview);
        a.handle(Action::Escape);
        assert_eq!(a.player.progress().0, a.player.tokens().len(), "the cursor jumped to the top");
    }

    // --- popups and messages ------------------------------------------------

    /// Reading and reading about reading do not mix.
    #[test]
    fn opening_help_pauses_playback() {
        let mut a = app();
        a.handle(Action::TogglePlay);
        assert!(a.player.is_playing());
        a.handle(Action::ToggleHelp);
        assert!(a.show_help);
        assert!(!a.player.is_playing());
    }

    /// While a popup is up, `J`/`K` move the popup — changing a speed you cannot see is useless.
    #[test]
    fn movement_keys_scroll_an_open_popup_instead_of_changing_speed() {
        let mut a = app();
        a.relayout(80, 12); // short enough that the key reference actually overflows
        let wpm = a.player.wpm();
        a.handle(Action::ToggleHelp);
        a.handle(Action::Slower);
        assert_eq!(a.player.wpm(), wpm, "the speed must not move behind a popup");
        assert_eq!(a.help_scroll, 1);
        a.handle(Action::Faster);
        assert_eq!(a.help_scroll, 0);
        a.handle(Action::Faster);
        assert_eq!(a.help_scroll, 0, "scrolling must stop at the top");
    }

    #[test]
    fn review_scrolls_within_its_paragraph_and_stops_at_the_end() {
        let text: String =
            (0..30).map(|i| format!("sentence {i} of one very long paragraph ")).collect();
        let doc = parser::parse_markdown(&segment::sanitize(&text));
        let mut a = App::new(doc, "long.md".into(), 300, PacingMode::Natural);
        a.relayout(80, 24);
        a.handle(Action::ToggleReview);
        a.handle(Action::ScrollDown);
        assert_eq!(a.review_scroll, 1);
        for _ in 0..200 {
            a.handle(Action::ScrollDown);
        }
        let bottom = a.review_scroll;
        a.handle(Action::ScrollDown);
        assert_eq!(a.review_scroll, bottom, "scrolling must stop at the last row");
    }

    #[test]
    fn escape_closes_the_popup_without_quitting() {
        let mut a = app();
        a.handle(Action::ToggleHelp);
        a.handle(Action::Escape);
        assert!(!a.show_help);
        assert!(!a.should_quit);
    }

    #[test]
    fn escape_with_nothing_open_does_nothing() {
        let mut a = app();
        a.handle(Action::Escape);
        assert!(!a.should_quit);
        assert_eq!(a.mode, Mode::Reader);
    }

    #[test]
    fn quit_sets_the_quit_flag() {
        let mut a = app();
        a.handle(Action::Quit);
        assert!(a.should_quit);
    }

    #[test]
    fn any_navigation_clears_a_pending_message() {
        let mut a = app();
        a.message = Some("note".into());
        a.handle(Action::Forward);
        assert!(a.message.is_none());
    }

    #[test]
    fn opening_help_keeps_the_message_visible() {
        let mut a = app().with_message(Some("note".into()));
        a.handle(Action::ToggleHelp);
        assert!(a.message.is_some());
    }

    #[test]
    fn an_empty_document_survives_every_action() {
        let doc = parser::parse_markdown("");
        let mut a = App::new(doc, "empty.md".into(), 300, PacingMode::Natural);
        a.relayout(120, 40);
        for action in [
            Action::TogglePlay,
            Action::Forward,
            Action::Back,
            Action::NextBlock,
            Action::PrevBlock,
            Action::GotoEnd,
            Action::GotoStart,
            Action::Restart,
            Action::Faster,
            Action::FocusPanel,
            Action::ScrollUp,
            Action::ScrollDown,
            Action::ToggleOutline,
            Action::Select,
            Action::ToggleReview,
            Action::Escape,
        ] {
            a.handle(action);
        }
        assert!(a.current_word().is_none());
        assert_eq!(a.panel_scroll(), 0);
    }

    /// Resizing must not leave the panel wrapped to a width that no longer exists.
    #[test]
    fn resizing_rewraps_the_panel() {
        let mut a = long_app();
        let at_120 = a.panel.rows.len();
        a.relayout(80, 24);
        assert!(a.panel.rows.len() > at_120, "a narrower panel needs more rows");
        a.relayout(120, 40);
        assert_eq!(a.panel.rows.len(), at_120, "going back to a known width must reuse its wrap");
    }
}
