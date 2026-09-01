//! Application state and the key dispatcher.

use oxidiris_core::player::{SEEK_WORDS, WPM_STEP_COARSE, WPM_STEP_FINE};
use oxidiris_core::{Document, PacingMode, Player};

use crate::keymap::Action;
use crate::term::Capabilities;

/// Everything the renderer needs to draw a frame.
pub struct App {
    /// Playback cursor over the token stream.
    pub player: Player,
    /// Title shown on the reader frame, normally the file name.
    pub title: String,
    /// What the terminal can render.
    pub caps: Capabilities,
    /// Whether the key reference is open.
    pub show_help: bool,
    /// Scroll offset inside the key reference.
    pub help_scroll: u16,
    /// Transient note shown in place of the key hints.
    pub message: Option<String>,
    /// Set once the reader has asked to leave.
    pub should_quit: bool,
}

impl App {
    /// Build an application over a parsed document.
    pub fn new(doc: &Document, title: String, wpm: u16, pacing: PacingMode) -> Self {
        App {
            player: Player::from_document(doc, wpm, pacing),
            title,
            caps: Capabilities::default(),
            show_help: false,
            help_scroll: 0,
            message: None,
            should_quit: false,
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

    /// The word currently on screen.
    pub fn current_word(&self) -> Option<&str> {
        self.player.current().map(|t| t.text.as_str())
    }

    /// Apply an action.
    pub fn handle(&mut self, action: Action) {
        // The message is a response to the last thing that happened, so any new input clears it.
        if !matches!(action, Action::ToggleHelp | Action::Escape) {
            self.message = None;
        }

        match action {
            Action::TogglePlay => {
                self.show_help = false;
                self.player.toggle();
            }
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
            Action::ToggleHelp => {
                self.show_help = !self.show_help;
                self.help_scroll = 0;
                // Opening the reference stops the stream; reading and reading about reading do not
                // mix.
                if self.show_help {
                    self.player.pause();
                }
            }
            Action::Escape => {
                if self.show_help {
                    self.show_help = false;
                }
            }
            Action::Quit => self.should_quit = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidiris_core::{PlayState, parser, segment};

    const DOC: &str = "\
# Heading one

First paragraph with a handful of words in it.

Second paragraph, also with several words.
";

    fn app() -> App {
        let doc = parser::parse_markdown(&segment::sanitize(DOC));
        App::new(&doc, "test.md".into(), 300, PacingMode::Natural)
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

    /// Esc leaves the popup; only q leaves the program (spec §7.2).
    #[test]
    fn escape_closes_the_popup_without_quitting() {
        let mut a = app();
        a.handle(Action::ToggleHelp);
        a.handle(Action::Escape);
        assert!(!a.show_help);
        assert!(!a.should_quit);
    }

    #[test]
    fn escape_with_no_popup_open_does_nothing() {
        let mut a = app();
        a.handle(Action::Escape);
        assert!(!a.should_quit);
    }

    #[test]
    fn quit_sets_the_quit_flag() {
        let mut a = app();
        a.handle(Action::Quit);
        assert!(a.should_quit);
    }

    #[test]
    fn any_navigation_clears_a_pending_message() {
        let mut a = app().with_message(Some("note".into()));
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
        let mut a = App::new(&doc, "empty.md".into(), 300, PacingMode::Natural);
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
        ] {
            a.handle(action);
        }
        assert!(a.current_word().is_none());
    }
}
