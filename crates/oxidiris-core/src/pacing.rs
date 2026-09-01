//! Per-token timing: how long each word stays on screen.
//!
//! Implements OXD-013. See spec §3.2.
//!
//! Showing every word for the same duration is the single easiest way to make RSVP exhausting:
//! the brain needs measurably different amounts of time for a three-letter function word and a
//! twenty-letter technical term.

use crate::sentence::{self, Break};
use crate::token::{Block, Document, Fragment, Token, TokenKind};
use crate::{orp, segment};

/// Word length beyond which the length penalty starts to apply, in graphemes.
const LENGTH_FREE_GRAPHEMES: usize = 6;
/// Extra multiplier per grapheme past [`LENGTH_FREE_GRAPHEMES`].
const LENGTH_STEP: f32 = 0.05;
/// Ceiling on the length multiplier, so a pathological token cannot stall the stream.
const LENGTH_CAP: f32 = 2.0;

/// Lower bound on reading speed, in words per minute.
pub const MIN_WPM: u16 = 50;
/// Upper bound on reading speed, in words per minute.
pub const MAX_WPM: u16 = 1500;
/// Default reading speed.
///
/// Deliberately conservative. High-speed flashing text is a photosensitivity risk under WCAG
/// SC 2.3.1, so the default is a safe speed rather than a marketing number (spec §3.4.2).
pub const DEFAULT_WPM: u16 = 300;
/// Speed above which the UI warns the reader once (spec §3.4.2).
pub const FLASH_WARNING_WPM: u16 = 700;

/// How aggressively to vary per-word timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PacingMode {
    /// Vary timing by word length, punctuation and structure.
    #[default]
    Natural,
    /// Give every token exactly the same duration.
    Linear,
}

/// Clamp a requested speed into the supported range.
pub const fn clamp_wpm(wpm: u16) -> u16 {
    if wpm < MIN_WPM {
        MIN_WPM
    } else if wpm > MAX_WPM {
        MAX_WPM
    } else {
        wpm
    }
}

/// Length multiplier for a word of `graphemes` clusters.
fn length_factor(graphemes: usize) -> f32 {
    let over = graphemes.saturating_sub(LENGTH_FREE_GRAPHEMES);
    (1.0 + over as f32 * LENGTH_STEP).min(LENGTH_CAP)
}

/// Turn a parsed [`Document`] into the RSVP token stream.
///
/// Blocks whose kind is not readable (tables) are skipped entirely, and headings contribute an
/// extra pause on the token that precedes them so the reader feels the section change.
pub fn tokenize(doc: &Document, mode: PacingMode) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();

    for block in &doc.blocks {
        if !block.kind.is_readable() {
            continue;
        }
        let first_of_block = tokens.len();
        for fragment in &block.fragments {
            push_fragment(&mut tokens, block, fragment, mode);
        }
        if tokens.len() == first_of_block {
            continue; // Block contributed nothing readable.
        }
        // Structural pause lands on the last token of the block.
        let pause = block.kind.trailing_pause_ms();
        if let Some(last) = tokens.last_mut() {
            last.pause_ms = last.pause_ms.max(pause);
        }
    }

    // A heading deserves a longer run-up than an ordinary paragraph break, so the token *before*
    // a heading gets upgraded (spec §3.2.1).
    apply_pre_heading_pauses(doc, &mut tokens);

    if mode == PacingMode::Linear {
        for t in &mut tokens {
            t.weight = 1.0;
            t.pause_ms = 0;
        }
    }

    resolve_sentence_breaks(&mut tokens, mode);
    tokens
}

/// Split one fragment into tokens and append them.
fn push_fragment(tokens: &mut Vec<Token>, block: &Block, fragment: &Fragment, _mode: PacingMode) {
    for word in segment::split_words(&fragment.text) {
        let graphemes = segment::grapheme_count(word.text);
        if graphemes == 0 {
            continue;
        }
        let layout = orp::layout(word.text);
        // Clamp into the fragment: Markdown escapes and entities make a fragment's text shorter
        // than the source range it came from, so a naive offset could point past the fragment.
        // Review Mode slices `Document::source` with these spans, so they must always be valid.
        let start = (fragment.byte_span.start + word.start).min(fragment.byte_span.end);
        let end = (fragment.byte_span.start + word.end).min(fragment.byte_span.end);

        tokens.push(Token {
            text: word.text.to_string(),
            orp_index: orp::orp_index_for_len(graphemes),
            display_width: layout.total_width,
            orp_offset: layout.orp_offset,
            weight: length_factor(graphemes) * fragment.kind.pacing_factor(),
            pause_ms: 0,
            kind: fragment.kind,
            block_id: block.id,
            byte_span: start..end,
        });
    }
}

/// Add a longer pause to the token immediately preceding each heading.
fn apply_pre_heading_pauses(doc: &Document, tokens: &mut [Token]) {
    const PRE_HEADING_PAUSE_MS: u32 = 400;

    for heading in &doc.headings {
        // Find the last token belonging to a block before this heading.
        let idx = tokens.iter().rposition(|t| t.block_id < heading.block_id);
        if let Some(idx) = idx {
            tokens[idx].pause_ms = tokens[idx].pause_ms.max(PRE_HEADING_PAUSE_MS);
        }
    }
}

/// Apply punctuation multipliers, which need each token's successor to disambiguate periods.
fn resolve_sentence_breaks(tokens: &mut [Token], mode: PacingMode) {
    if mode == PacingMode::Linear {
        return;
    }
    for i in 0..tokens.len() {
        let next = tokens.get(i + 1).map(|t| t.text.as_str());
        let brk = match tokens[i].kind {
            // Code and math are not prose; their punctuation carries no rhythm.
            TokenKind::Code | TokenKind::Math => Break::None,
            _ => sentence::classify(&tokens[i].text, next),
        };
        tokens[i].weight *= brk.factor();
    }
}

/// Total time to read `tokens` end to end at `wpm`, in milliseconds.
pub fn total_duration_ms(tokens: &[Token], wpm: u16) -> u64 {
    tokens.iter().map(|t| u64::from(t.duration_ms(wpm))).sum()
}

/// Actual reading speed once every multiplier and pause is accounted for.
///
/// This is always lower than the configured WPM, typically by 15-25%. Both numbers are shown in
/// the status bar so the configured speed is not mistaken for the real one (spec §3.2.4).
pub fn effective_wpm(tokens: &[Token], wpm: u16) -> u16 {
    let total = total_duration_ms(tokens, wpm);
    if total == 0 || tokens.is_empty() {
        return wpm;
    }
    let wpm_f = tokens.len() as f64 * 60_000.0 / total as f64;
    wpm_f.round().clamp(1.0, f64::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn tokens_of(md: &str, mode: PacingMode) -> Vec<Token> {
        let doc = parser::parse_markdown(&segment::sanitize(md));
        tokenize(&doc, mode)
    }

    #[test]
    fn wpm_is_clamped_to_the_supported_range() {
        assert_eq!(clamp_wpm(0), MIN_WPM);
        assert_eq!(clamp_wpm(9_000), MAX_WPM);
        assert_eq!(clamp_wpm(400), 400);
    }

    #[test]
    fn length_factor_is_capped_at_two() {
        assert_eq!(length_factor(3), 1.0);
        assert_eq!(length_factor(6), 1.0);
        assert!((length_factor(10) - 1.2).abs() < 1e-6);
        assert_eq!(length_factor(1_000), LENGTH_CAP);
    }

    #[test]
    fn linear_mode_gives_every_token_the_same_duration() {
        let tokens = tokens_of("# Title\n\nA short one and a considerably longer one.\n", PacingMode::Linear);
        assert!(tokens.len() > 3);
        let first = tokens[0].duration_ms(300);
        assert!(tokens.iter().all(|t| t.duration_ms(300) == first));
    }

    #[test]
    fn sentence_end_costs_about_two_and_a_quarter_times_a_plain_word() {
        let tokens = tokens_of("alpha bravo. charlie delta\n", PacingMode::Natural);
        let plain = tokens.iter().find(|t| t.text == "alpha").unwrap();
        let stop = tokens.iter().find(|t| t.text == "bravo.").unwrap();
        // Same length bucket, so the ratio isolates the punctuation factor.
        let ratio = f32::from(u16::try_from(stop.duration_ms(300) - stop.pause_ms).unwrap())
            / f32::from(u16::try_from(plain.duration_ms(300)).unwrap());
        assert!((ratio - 2.25).abs() < 0.1, "ratio was {ratio}");
    }

    #[test]
    fn effective_wpm_never_exceeds_the_configured_speed() {
        let tokens = tokens_of(
            "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs.\n",
            PacingMode::Natural,
        );
        let eff = effective_wpm(&tokens, 300);
        assert!(eff <= 300, "effective {eff} must not exceed configured 300");
        assert!(eff >= 200, "effective {eff} unexpectedly far below configured 300");
    }

    #[test]
    fn effective_wpm_on_an_empty_document_is_the_configured_speed() {
        assert_eq!(effective_wpm(&[], 400), 400);
    }

    #[test]
    fn tables_contribute_no_tokens() {
        let md = "Intro text here.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let tokens = tokens_of(md, PacingMode::Natural);
        assert!(tokens.iter().all(|t| t.kind != TokenKind::Table));
        assert!(tokens.iter().all(|t| t.text != "1" && t.text != "2"));
    }

    #[test]
    fn heading_gets_a_run_up_pause_on_the_preceding_token() {
        let tokens = tokens_of("First paragraph here\n\n## Section\n\nBody\n", PacingMode::Natural);
        let last_before = tokens.iter().find(|t| t.text == "here").unwrap();
        assert!(last_before.pause_ms >= 400, "pause was {}", last_before.pause_ms);
    }
}