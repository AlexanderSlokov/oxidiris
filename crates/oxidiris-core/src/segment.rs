//! Unicode-correct text sanitisation and word splitting.
//!
//! Implements OXD-011. See spec §3.1.1 and §8.6.
//!
//! # The three traps this module exists to avoid
//!
//! 1. **Grapheme != char != byte.** Vietnamese `ế` is one code point in NFC and three in NFD.
//!    Counting `char`s would give the same word two different ORP indices depending on which
//!    form the file happened to use. Everything is therefore normalized to NFC once, up front.
//! 2. **Display width != grapheme count.** CJK and most emoji occupy two terminal columns.
//!    Anchoring by grapheme count would make the text drift horizontally, destroying the exact
//!    spatial consistency the tool is built to provide.
//! 3. **Zero-width and control characters.** Bidi overrides and control codes have zero or
//!    undefined width, so they are stripped before anything else looks at the text.

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A word extracted from a text fragment, with its byte range inside that fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word<'a> {
    /// The word itself, punctuation included.
    pub text: &'a str,
    /// Byte offset of the word within the fragment it was split from.
    pub start: usize,
    /// Byte offset just past the word.
    pub end: usize,
}

/// Returns true for characters that must never reach the renderer.
///
/// Keeps `\n` and `\t` out of the "strip" set: they are meaningful to the parsers, which convert
/// them to structure before tokenization happens.
fn is_stripped(c: char) -> bool {
    match c {
        '\n' | '\t' | '\r' => false,
        // C0/C1 controls.
        c if c.is_control() => true,
        // Bidi embedding, override and isolate controls.
        '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => true,
        // Zero-width space / non-joiner. ZWJ (U+200D) is kept: it welds emoji sequences into a
        // single grapheme cluster, so removing it would split them.
        '\u{200B}' | '\u{200C}' | '\u{FEFF}' => true,
        _ => false,
    }
}

/// Normalize to NFC and remove characters that must not be rendered.
///
/// This is the single entry point that makes NFC and NFD inputs converge: run it once when the
/// document is loaded, and every downstream byte offset refers to the normalized string.
pub fn sanitize(input: &str) -> String {
    input.nfc().filter(|c| !is_stripped(*c)).collect()
}

/// Number of grapheme clusters in `text`.
///
/// Emoji joined with ZWJ (families, flags, skin-tone sequences) count as one.
pub fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}

/// Grapheme clusters of `text`, in order.
///
/// Exposed so that consumers (including the renderer) never have to reach for a Unicode crate of
/// their own and risk disagreeing with the engine about where a character begins.
pub fn graphemes(text: &str) -> Vec<&str> {
    text.graphemes(true).collect()
}

/// Terminal column width of `text` per UAX #11.
///
/// This is what the renderer must use for alignment: `"日本語"` is 3 graphemes but 6 columns.
pub fn display_width(text: &str) -> u16 {
    UnicodeWidthStr::width(text).min(u16::MAX as usize) as u16
}

/// Split a text fragment into RSVP display units.
///
/// Splitting is whitespace-based rather than UAX #29 word-boundary based, because the display
/// unit must keep its trailing punctuation attached: the pacing engine needs to see `"end."` as
/// one unit to know a sentence just closed. UAX #29 would hand back `"end"` and `"."` separately.
///
/// # Scripts without spaces
///
/// For CJK this yields one enormous "word" per run of text, which is not useful. That is a known
/// gap, deliberately left open pending DEC-02 rather than papered over with a half-working
/// heuristic (spec §8.6).
pub fn split_words(text: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    let mut start: Option<usize> = None;

    for (idx, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                words.push(Word { text: &text[s..idx], start: s, end: idx });
            }
        } else if start.is_none() {
            start = Some(idx);
        }
    }
    if let Some(s) = start {
        words.push(Word { text: &text[s..], start: s, end: text.len() });
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_and_nfd_converge_to_identical_text() {
        // "tiếng Việt" written in composed and decomposed form.
        let nfc = "ti\u{1EBF}ng Vi\u{1EC7}t";
        let nfd = "tie\u{302}\u{301}ng Vie\u{323}\u{302}t";
        assert_ne!(nfc, nfd, "inputs must genuinely differ before normalization");
        assert_eq!(sanitize(nfc), sanitize(nfd));
    }

    #[test]
    fn nfc_and_nfd_produce_identical_word_streams() {
        let nfc = sanitize("ti\u{1EBF}ng Vi\u{1EC7}t");
        let nfd = sanitize("tie\u{302}\u{301}ng Vie\u{323}\u{302}t");
        let a: Vec<_> = split_words(&nfc).iter().map(|w| w.text.to_string()).collect();
        let b: Vec<_> = split_words(&nfd).iter().map(|w| w.text.to_string()).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn cjk_is_three_graphemes_but_six_columns() {
        assert_eq!(grapheme_count("日本語"), 3);
        assert_eq!(display_width("日本語"), 6);
    }

    #[test]
    fn zwj_emoji_stays_one_grapheme() {
        // Family: man + ZWJ + woman + ZWJ + girl.
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        assert_eq!(grapheme_count(&sanitize(family)), 1);
    }

    #[test]
    fn regional_indicator_flag_stays_one_grapheme() {
        let flag = "\u{1F1FB}\u{1F1F3}"; // VN
        assert_eq!(grapheme_count(&sanitize(flag)), 1);
    }

    #[test]
    fn control_and_bidi_characters_are_stripped() {
        let dirty = "a\u{202E}b\u{0007}c\u{200B}d";
        assert_eq!(sanitize(dirty), "abcd");
    }

    #[test]
    fn empty_input_yields_no_words() {
        assert!(split_words("").is_empty());
        assert!(split_words("   \n\t ").is_empty());
        assert_eq!(sanitize(""), "");
    }

    #[test]
    fn word_ranges_index_back_into_the_input() {
        let text = "alpha beta  gamma";
        for w in split_words(text) {
            assert_eq!(&text[w.start..w.end], w.text);
        }
    }

    #[test]
    fn graphemes_agree_with_the_grapheme_count() {
        for text in ["", "hello", "日本語", "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"] {
            assert_eq!(graphemes(text).len(), grapheme_count(text));
            assert_eq!(graphemes(text).concat(), text);
        }
    }

    #[test]
    fn punctuation_stays_attached_to_its_word() {
        let words: Vec<_> = split_words("Hello, world.").iter().map(|w| w.text).collect();
        assert_eq!(words, vec!["Hello,", "world."]);
    }
}
