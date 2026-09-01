//! Optimal Recognition Point: where the eye should be anchored inside a word.
//!
//! Implements OXD-012. See spec §3.1.2 and §3.1.3.
//!
//! This is the single most load-bearing algorithm in the project. If the ORP column drifts even
//! by one cell between words, the eye has to move, and the entire premise of the tool collapses.

use unicode_segmentation::UnicodeSegmentation;

use crate::segment::display_width;

/// Lookup table mapping word length (in grapheme clusters) to an ORP index.
///
/// Read as: "a word of at most `max_len` graphemes anchors on grapheme `index`". Longer words
/// fall through to [`ORP_INDEX_CAP`].
///
/// Encoded as data rather than nested conditionals so that DEC-03 (empirical validation against
/// Vietnamese and CJK readers) can be applied by editing one array.
const ORP_TABLE: &[(usize, usize)] = &[(1, 0), (5, 1), (9, 2), (13, 3)];

/// Ceiling for the ORP index on very long words.
///
/// Pushing the anchor further right on a 20-character chemical term or function name would drag
/// the tail of the word out of foveal vision (roughly 2 degrees), which costs more than the
/// slightly off-centre anchor saves.
const ORP_INDEX_CAP: usize = 4;

/// ORP index for a word of `len` grapheme clusters.
pub const fn orp_index_for_len(len: usize) -> usize {
    let mut i = 0;
    while i < ORP_TABLE.len() {
        let (max_len, index) = ORP_TABLE[i];
        if len <= max_len {
            return index;
        }
        i += 1;
    }
    ORP_INDEX_CAP
}

/// ORP index for `word`, counted in grapheme clusters.
///
/// Guaranteed to be a valid index into the word's graphemes for any non-empty input.
pub fn orp_index(word: &str) -> usize {
    orp_index_for_len(word.graphemes(true).count())
}

/// Terminal column offset of the ORP grapheme from the start of `word`.
///
/// The renderer subtracts this from the fixed anchor column to find where to start drawing, which
/// is what pins the ORP grapheme to the same column for every word (spec §3.1.3).
pub fn orp_offset(word: &str) -> u16 {
    let idx = orp_index(word);
    word.graphemes(true).take(idx).map(display_width).sum()
}

/// Everything the renderer needs to draw one word: the split points and the widths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrpLayout {
    /// Graphemes before the ORP.
    pub prefix: String,
    /// The ORP grapheme itself. Empty only for empty input.
    pub focus: String,
    /// Graphemes after the ORP.
    pub suffix: String,
    /// Column offset of `focus` from the start of the word.
    pub orp_offset: u16,
    /// Total column width of the word.
    pub total_width: u16,
}

/// Compute the full render layout for `word`.
pub fn layout(word: &str) -> OrpLayout {
    let graphemes: Vec<&str> = word.graphemes(true).collect();
    if graphemes.is_empty() {
        return OrpLayout {
            prefix: String::new(),
            focus: String::new(),
            suffix: String::new(),
            orp_offset: 0,
            total_width: 0,
        };
    }
    let idx = orp_index_for_len(graphemes.len());
    let prefix: String = graphemes[..idx].concat();
    let focus = graphemes[idx].to_string();
    let suffix: String = graphemes[idx + 1..].concat();
    let orp_offset = display_width(&prefix);
    let total_width = display_width(word);

    OrpLayout { prefix, focus, suffix, orp_offset, total_width }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::sanitize;

    #[test]
    fn lookup_table_matches_the_specified_buckets() {
        assert_eq!(orp_index("a"), 0);
        assert_eq!(orp_index("at"), 1);
        assert_eq!(orp_index("hello"), 1);
        assert_eq!(orp_index("reading"), 2);
        assert_eq!(orp_index("javascript"), 3);
        assert_eq!(orp_index("internationalization"), ORP_INDEX_CAP);
    }

    #[test]
    fn single_grapheme_word_anchors_on_index_zero() {
        assert_eq!(orp_index("I"), 0);
        assert_eq!(orp_offset("I"), 0);
    }

    #[test]
    fn twenty_character_word_is_capped() {
        let long = "a".repeat(20);
        assert_eq!(orp_index(&long), ORP_INDEX_CAP);
    }

    #[test]
    fn nfc_and_nfd_give_the_same_orp() {
        let nfc = sanitize("ti\u{1EBF}ng");
        let nfd = sanitize("tie\u{302}\u{301}ng");
        assert_eq!(orp_index(&nfc), orp_index(&nfd));
        assert_eq!(orp_offset(&nfc), orp_offset(&nfd));
    }

    #[test]
    fn cjk_offset_is_measured_in_columns_not_characters() {
        // 4 graphemes -> index 1. One full-width grapheme precedes the ORP, so the offset is 2
        // columns, not 1 character.
        assert_eq!(orp_index("日本語文"), 1);
        assert_eq!(orp_offset("日本語文"), 2);
    }

    #[test]
    fn zwj_emoji_does_not_shift_the_index() {
        let family = sanitize("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}");
        assert_eq!(orp_index(&family), 0);
        assert_eq!(orp_offset(&family), 0);
    }

    #[test]
    fn layout_reassembles_the_original_word() {
        for word in ["a", "at", "hello", "javascript", "日本語文", "Oxidiris"] {
            let l = layout(word);
            assert_eq!(format!("{}{}{}", l.prefix, l.focus, l.suffix), word);
        }
    }

    #[test]
    fn empty_word_does_not_panic() {
        let l = layout("");
        assert_eq!(l.total_width, 0);
        assert!(l.focus.is_empty());
    }
}

/// Property tests for the invariant that keeps the anchor column stable.
#[cfg(test)]
mod properties {
    use super::*;
    use crate::segment::{display_width, grapheme_count, sanitize};
    use proptest::prelude::*;

    proptest! {
        /// The core invariant: for *any* Unicode input, the ORP must be a real grapheme of the
        /// word and must never sit further right than the word is wide. A violation here is
        /// exactly the "text jumps horizontally" bug.
        #[test]
        fn orp_is_always_inside_the_word(raw in ".{0,64}") {
            let word = sanitize(&raw);
            let n = grapheme_count(&word);
            prop_assume!(n > 0);

            prop_assert!(orp_index(&word) < n);
            prop_assert!(orp_offset(&word) <= display_width(&word));
        }

        /// The layout must be a lossless partition of the word.
        #[test]
        fn layout_is_lossless(raw in ".{0,64}") {
            let word = sanitize(&raw);
            prop_assume!(grapheme_count(&word) > 0);

            let l = layout(&word);
            prop_assert_eq!(format!("{}{}{}", l.prefix, l.focus, l.suffix), word.clone());
            prop_assert_eq!(l.orp_offset, display_width(&l.prefix));
            prop_assert_eq!(l.total_width, display_width(&word));
        }
    }
}