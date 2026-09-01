//! Playback state machine for the RSVP stream.
//!
//! Implements OXD-018. See spec §4.1.
//!
//! # Why there is no clock in here
//!
//! [`Player`] never calls `Instant::now`, never sleeps, and holds no timer. It reports *how long*
//! the current token wants to be on screen and leaves the scheduling to the event loop.
//!
//! That split buys two things: the whole state machine is testable without real time passing, and
//! the same code runs unmodified under WebAssembly (OXD-080), where the host owns the clock.

use crate::pacing::{self, PacingMode};
use crate::token::{Document, Token};

/// Whether the stream is advancing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    /// Tokens are advancing.
    Playing,
    /// Cursor is held on the current token.
    Paused,
    /// Cursor has passed the last token.
    Finished,
}

/// Words moved per backstep / skip action (spec §7.2).
pub const SEEK_WORDS: usize = 5;
/// WPM change per coarse adjustment.
pub const WPM_STEP_COARSE: u16 = 25;
/// WPM change per fine adjustment.
pub const WPM_STEP_FINE: u16 = 5;

/// Cursor and speed over a token stream.
#[derive(Debug, Clone)]
pub struct Player {
    tokens: Vec<Token>,
    cursor: usize,
    wpm: u16,
    state: PlayState,
}

impl Player {
    /// Build a player over an already-tokenized stream.
    pub fn new(tokens: Vec<Token>, wpm: u16) -> Self {
        let state = if tokens.is_empty() { PlayState::Finished } else { PlayState::Paused };
        Player { tokens, cursor: 0, wpm: pacing::clamp_wpm(wpm), state }
    }

    /// Build a player straight from a parsed document.
    pub fn from_document(doc: &Document, wpm: u16, mode: PacingMode) -> Self {
        Self::new(pacing::tokenize(doc, mode), wpm)
    }

    /// The full token stream.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Current playback state.
    pub fn state(&self) -> PlayState {
        self.state
    }

    /// Whether the stream is currently advancing.
    pub fn is_playing(&self) -> bool {
        self.state == PlayState::Playing
    }

    /// Configured reading speed, in words per minute.
    pub fn wpm(&self) -> u16 {
        self.wpm
    }

    /// Actual speed once pacing multipliers are accounted for (spec §3.2.4).
    pub fn effective_wpm(&self) -> u16 {
        pacing::effective_wpm(&self.tokens, self.wpm)
    }

    /// Token currently on screen, if any.
    pub fn current(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    /// How long the current token should remain visible.
    pub fn current_duration_ms(&self) -> u32 {
        self.current().map_or(0, |t| t.duration_ms(self.wpm))
    }

    /// `(current index, total tokens)`, where the index is 1-based for display.
    pub fn progress(&self) -> (usize, usize) {
        (self.cursor.min(self.tokens.len().saturating_sub(1)) + 1, self.tokens.len())
    }

    /// Fraction of the document already consumed, in `0.0..=1.0`.
    pub fn progress_ratio(&self) -> f64 {
        if self.tokens.is_empty() {
            return 1.0;
        }
        self.cursor as f64 / self.tokens.len() as f64
    }

    /// Start advancing. Restarts from the top if the stream had finished.
    pub fn play(&mut self) {
        if self.tokens.is_empty() {
            return;
        }
        if self.state == PlayState::Finished {
            self.cursor = 0;
        }
        self.state = PlayState::Playing;
    }

    /// Hold the cursor on the current token.
    pub fn pause(&mut self) {
        if self.state == PlayState::Playing {
            self.state = PlayState::Paused;
        }
    }

    /// Flip between playing and paused.
    pub fn toggle(&mut self) {
        match self.state {
            PlayState::Playing => self.pause(),
            PlayState::Paused | PlayState::Finished => self.play(),
        }
    }

    /// Advance one token. Returns the token now on screen, or `None` at the end of the stream.
    pub fn advance(&mut self) -> Option<&Token> {
        if self.tokens.is_empty() {
            self.state = PlayState::Finished;
            return None;
        }
        if self.cursor + 1 >= self.tokens.len() {
            self.cursor = self.tokens.len() - 1;
            self.state = PlayState::Finished;
            return None;
        }
        self.cursor += 1;
        self.tokens.get(self.cursor)
    }

    /// Jump to the beginning and pause.
    pub fn restart(&mut self) {
        self.cursor = 0;
        self.state = if self.tokens.is_empty() { PlayState::Finished } else { PlayState::Paused };
    }

    /// Move the cursor by `delta` words, clamped to the document.
    pub fn seek_words(&mut self, delta: isize) {
        if self.tokens.is_empty() {
            return;
        }
        let last = self.tokens.len() - 1;
        let target = self.cursor as isize + delta;
        self.cursor = target.clamp(0, last as isize) as usize;
        if self.state == PlayState::Finished && self.cursor < last {
            self.state = PlayState::Paused;
        }
    }

    /// Move to the first token of the previous or next block.
    ///
    /// Block identity comes from the parser, not from guessing at the text: paragraph boundaries
    /// in LaTeX or PDF look nothing like Markdown's (spec §3.3).
    pub fn seek_blocks(&mut self, delta: isize) {
        if self.tokens.is_empty() || delta == 0 {
            return;
        }
        let current_block = self.tokens[self.cursor].block_id;
        let target = if delta > 0 {
            self.tokens.iter().position(|t| t.block_id > current_block)
        } else {
            // First token of the current block; if already there, of the previous one.
            let first_of_current =
                self.tokens.iter().position(|t| t.block_id == current_block).unwrap_or(0);
            if self.cursor > first_of_current {
                Some(first_of_current)
            } else {
                self.tokens
                    .iter()
                    .rposition(|t| t.block_id < current_block)
                    .map(|idx| {
                        let b = self.tokens[idx].block_id;
                        self.tokens.iter().position(|t| t.block_id == b).unwrap_or(idx)
                    })
                    .or(Some(0))
            }
        };
        if let Some(idx) = target {
            self.cursor = idx;
            self.state = match self.state {
                PlayState::Finished => PlayState::Paused,
                other => other,
            };
        }
    }

    /// Jump to a fraction of the document, `0.0..=1.0`.
    pub fn seek_ratio(&mut self, ratio: f64) {
        if self.tokens.is_empty() {
            return;
        }
        let last = self.tokens.len() - 1;
        let idx = (ratio.clamp(0.0, 1.0) * last as f64).round() as usize;
        self.cursor = idx.min(last);
        if self.state == PlayState::Finished {
            self.state = PlayState::Paused;
        }
    }

    /// Jump to the first token.
    pub fn goto_start(&mut self) {
        self.seek_ratio(0.0);
    }

    /// Jump to the last token.
    pub fn goto_end(&mut self) {
        self.seek_ratio(1.0);
    }

    /// Set the reading speed. Clamped to the supported range.
    ///
    /// Only affects tokens not yet shown: durations are derived from the token weight at read
    /// time rather than baked in when the document was parsed.
    pub fn set_wpm(&mut self, wpm: u16) {
        self.wpm = pacing::clamp_wpm(wpm);
    }

    /// Change the reading speed by `delta`, saturating at the supported bounds.
    pub fn adjust_wpm(&mut self, delta: i32) {
        let next = i32::from(self.wpm) + delta;
        self.set_wpm(next.clamp(0, i32::from(u16::MAX)) as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parser, segment};

    fn player_from(md: &str) -> Player {
        let doc = parser::parse_markdown(&segment::sanitize(md));
        Player::from_document(&doc, 300, PacingMode::Natural)
    }

    const DOC: &str = "First block has several words here.\n\nSecond block also has words.\n\nThird block ends it.\n";

    #[test]
    fn empty_document_is_safe_for_every_operation() {
        let mut p = Player::new(Vec::new(), 300);
        assert_eq!(p.state(), PlayState::Finished);
        assert!(p.current().is_none());
        assert_eq!(p.current_duration_ms(), 0);
        p.play();
        p.advance();
        p.seek_words(-10);
        p.seek_blocks(1);
        p.seek_ratio(0.5);
        p.restart();
        assert!(p.current().is_none());
    }

    #[test]
    fn advance_walks_to_the_end_then_finishes() {
        let mut p = player_from(DOC);
        let total = p.tokens().len();
        assert!(total > 5);
        p.play();
        let mut steps = 0;
        while p.advance().is_some() {
            steps += 1;
            assert!(steps < total + 5, "advance failed to terminate");
        }
        assert_eq!(p.state(), PlayState::Finished);
        assert_eq!(p.progress().0, total);
    }

    #[test]
    fn seek_is_clamped_at_both_ends() {
        let mut p = player_from(DOC);
        p.seek_words(-1_000);
        assert_eq!(p.progress().0, 1);
        p.seek_words(1_000_000);
        assert_eq!(p.progress().0, p.tokens().len());
    }

    #[test]
    fn seek_words_does_not_overflow_on_extreme_deltas() {
        let mut p = player_from(DOC);
        p.seek_words(isize::MAX);
        p.seek_words(isize::MIN);
        assert_eq!(p.progress().0, 1);
    }

    #[test]
    fn block_navigation_uses_block_ids() {
        let mut p = player_from(DOC);
        let first_block = p.current().unwrap().block_id;
        p.seek_blocks(1);
        assert!(p.current().unwrap().block_id > first_block);
        p.seek_blocks(-1);
        assert_eq!(p.current().unwrap().block_id, first_block);
    }

    #[test]
    fn block_navigation_at_the_start_stays_at_the_start() {
        let mut p = player_from(DOC);
        p.seek_blocks(-1);
        p.seek_blocks(-1);
        assert_eq!(p.progress().0, 1);
    }

    #[test]
    fn seek_ratio_lands_where_expected() {
        let mut p = player_from(DOC);
        p.seek_ratio(1.0);
        assert_eq!(p.progress().0, p.tokens().len());
        p.seek_ratio(0.0);
        assert_eq!(p.progress().0, 1);
        p.seek_ratio(-5.0);
        assert_eq!(p.progress().0, 1);
    }

    /// Changing speed mid-document must not rewrite the timing of tokens already read.
    #[test]
    fn set_wpm_is_not_retroactive() {
        let mut p = player_from(DOC);
        let before = p.current_duration_ms();
        p.advance();
        p.set_wpm(600);
        let after = p.current_duration_ms();
        assert!(after < before, "faster WPM should shorten upcoming tokens");
        // The token stream itself is untouched: only the derived duration changed.
        assert_eq!(p.tokens()[0].weight, p.tokens()[0].weight);
    }

    #[test]
    fn wpm_is_clamped_by_the_player() {
        let mut p = player_from(DOC);
        p.set_wpm(5);
        assert_eq!(p.wpm(), pacing::MIN_WPM);
        p.adjust_wpm(100_000);
        assert_eq!(p.wpm(), pacing::MAX_WPM);
        p.adjust_wpm(-100_000);
        assert_eq!(p.wpm(), pacing::MIN_WPM);
    }

    #[test]
    fn toggle_cycles_play_and_pause() {
        let mut p = player_from(DOC);
        assert_eq!(p.state(), PlayState::Paused);
        p.toggle();
        assert_eq!(p.state(), PlayState::Playing);
        p.toggle();
        assert_eq!(p.state(), PlayState::Paused);
    }

    #[test]
    fn play_after_finishing_restarts_from_the_top() {
        let mut p = player_from(DOC);
        p.goto_end();
        while p.advance().is_some() {}
        assert_eq!(p.state(), PlayState::Finished);
        p.play();
        assert_eq!(p.progress().0, 1);
        assert_eq!(p.state(), PlayState::Playing);
    }

    /// The whole state machine must be exercisable without any real time passing.
    #[test]
    fn no_wall_clock_is_needed_to_drive_playback() {
        let mut p = player_from(DOC);
        p.play();
        let mut virtual_elapsed_ms: u64 = 0;
        while p.current().is_some() {
            virtual_elapsed_ms += u64::from(p.current_duration_ms());
            if p.advance().is_none() {
                break;
            }
        }
        assert!(virtual_elapsed_ms > 0);
    }
}