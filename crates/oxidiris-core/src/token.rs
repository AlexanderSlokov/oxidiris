//! Core data types: [`Token`], [`Block`], [`Heading`] and [`Document`].
//!
//! Implements OXD-010. See spec §4.1.

use core::ops::Range;

/// Semantic class of a token, used to pick a pacing multiplier and a render style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// Ordinary prose word.
    Word,
    /// Inline code span or a line inside a fenced code block.
    Code,
    /// Math expression, kept whole rather than split into meaningless fragments.
    Math,
    /// Heading text, carrying its level (1..=6).
    Heading(u8),
    /// A word inside a list item.
    ListItem,
    /// A citation label such as `[12]` or a rendered `\cite{...}`.
    Citation,
    /// Table content. Excluded from the RSVP stream (see spec §8.1).
    Table,
}

impl TokenKind {
    /// Pacing multiplier applied on top of the base word duration.
    ///
    /// See spec §3.2.1 (`kind_factor`).
    pub const fn pacing_factor(self) -> f32 {
        match self {
            TokenKind::Word | TokenKind::ListItem => 1.0,
            TokenKind::Heading(_) => 1.15,
            TokenKind::Citation => 0.8,
            TokenKind::Code | TokenKind::Math => 1.5,
            TokenKind::Table => 1.0,
        }
    }
}

/// One display unit of the RSVP stream.
///
/// # Timing model
///
/// A token deliberately does **not** store a fixed `duration_ms`. It stores the
/// WPM-independent [`Token::weight`] and [`Token::pause_ms`], and the final duration is derived
/// on demand via [`Token::duration_ms`].
///
/// This is what makes a mid-document speed change non-retroactive: tokens already consumed keep
/// the timing they were shown with, and only upcoming tokens pick up the new WPM. Baking an
/// absolute duration into the token would force a full re-pace of the document on every keypress.
///
/// > Deviation from spec §4.1, which listed a plain `duration_ms` field. Recorded in
/// > `docs/decisions/token-timing.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// Text as displayed, already NFC-normalized and stripped of control characters.
    pub text: String,
    /// Index of the ORP grapheme within [`Token::text`], counted in grapheme clusters.
    pub orp_index: usize,
    /// Total terminal column width of [`Token::text`] (UAX #11), not the character count.
    pub display_width: u16,
    /// Column width of the graphemes *before* the ORP grapheme.
    ///
    /// The renderer left-pads by `anchor_column - orp_offset` so the ORP grapheme always starts
    /// at the same absolute column, no matter how long the word is (spec §3.1.3).
    pub orp_offset: u16,
    /// WPM-independent duration multiplier (length x punctuation x kind).
    pub weight: f32,
    /// Structural pause added *after* this token, in milliseconds. Not scaled by WPM.
    pub pause_ms: u32,
    /// Semantic class of this token.
    pub kind: TokenKind,
    /// Index of the [`Block`] this token belongs to. Drives paragraph navigation.
    pub block_id: usize,
    /// Byte range of this token inside [`Document::source`].
    ///
    /// This is the hook that lets the full-text panel highlight the word currently being shown
    /// (OXD-031) and lets Review Mode rebuild an entire paragraph verbatim (OXD-034).
    pub byte_span: Range<usize>,
}

impl Token {
    /// Duration this token should stay on screen at the given speed.
    ///
    /// `wpm` is expected to be pre-clamped by [`crate::pacing::clamp_wpm`].
    pub fn duration_ms(&self, wpm: u16) -> u32 {
        let base = 60_000.0 / f32::from(wpm.max(1));
        (base * self.weight) as u32 + self.pause_ms
    }
}

/// Structural class of a [`Block`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockKind {
    /// Ordinary prose paragraph.
    Paragraph,
    /// Section heading of the given level.
    Heading(u8),
    /// Fenced or indented code block.
    CodeBlock,
    /// A single item of a bulleted or numbered list.
    ListItem,
    /// Block quote.
    Quote,
    /// Tabular data, rendered whole rather than read word by word.
    Table,
}

impl BlockKind {
    /// Whether blocks of this kind contribute tokens to the RSVP stream.
    ///
    /// Tables are rendered whole in the side panel instead of being read word by word,
    /// because a table read sequentially is meaningless (spec §8.1).
    pub const fn is_readable(self) -> bool {
        !matches!(self, BlockKind::Table)
    }

    /// Pause inserted after a block of this kind, in milliseconds (spec §3.2.1).
    pub const fn trailing_pause_ms(self) -> u32 {
        match self {
            BlockKind::Paragraph | BlockKind::Quote => 250,
            BlockKind::Heading(_) => 250,
            BlockKind::ListItem => 150,
            BlockKind::CodeBlock => 300,
            BlockKind::Table => 0,
        }
    }
}

/// A structural unit of the document: a paragraph, heading, list item, code block, ...
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Position of this block in [`Document::blocks`].
    pub id: usize,
    /// Structural class of this block.
    pub kind: BlockKind,
    /// Readable text of this block, with markup already stripped.
    pub text: String,
    /// Byte range covering this block inside [`Document::source`].
    pub byte_span: Range<usize>,
    /// Text fragments making up this block, each mapped back to its own source range.
    ///
    /// Markup removal makes block text non-contiguous in the source, so per-token byte spans are
    /// derived from these fragments rather than from [`Block::byte_span`].
    pub fragments: Vec<Fragment>,
}

/// A contiguous run of readable text mapped back to its exact source range.
#[derive(Debug, Clone, PartialEq)]
pub struct Fragment {
    /// Readable text of this fragment.
    pub text: String,
    /// Byte range of this fragment inside [`Document::source`].
    pub byte_span: Range<usize>,
    /// Semantic class inherited by every token produced from this fragment.
    pub kind: TokenKind,
}

/// An entry in the document outline (table of contents).
#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    /// Heading level, 1 through 6.
    pub level: u8,
    /// Heading text with markup removed.
    pub text: String,
    /// Index of the block this heading corresponds to.
    pub block_id: usize,
}

/// Document-level metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocumentMeta {
    /// Display title, usually the first H1 or the file name.
    pub title: Option<String>,
    /// Which parser produced this document.
    pub format: Option<&'static str>,
}

/// A parsed document: normalized source text plus its structure.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// Canonical text: decoded, NFC-normalized, control characters stripped.
    ///
    /// Every [`Token::byte_span`] and [`Block::byte_span`] indexes into *this* string, not into
    /// the raw bytes on disk. Normalizing once up front is what makes NFC and NFD inputs produce
    /// byte-identical documents (spec §3.1.1).
    pub source: String,
    /// Structural blocks in reading order.
    pub blocks: Vec<Block>,
    /// Flat outline used for the table of contents.
    pub headings: Vec<Heading>,
    /// Document-level metadata.
    pub meta: DocumentMeta,
}

impl Document {
    /// Plain-text rendering of the document, used by `--dump` (OXD-026).
    pub fn to_plain_text(&self) -> String {
        let mut out = String::with_capacity(self.source.len());
        for block in &self.blocks {
            match block.kind {
                BlockKind::Heading(level) => {
                    out.push_str(&"#".repeat(level as usize));
                    out.push(' ');
                }
                BlockKind::ListItem => out.push_str("- "),
                BlockKind::Quote => out.push_str("> "),
                _ => {}
            }
            out.push_str(&block.text);
            out.push_str("\n\n");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(weight: f32, pause_ms: u32) -> Token {
        Token {
            text: "x".into(),
            orp_index: 0,
            display_width: 1,
            orp_offset: 0,
            weight,
            pause_ms,
            kind: TokenKind::Word,
            block_id: 0,
            byte_span: 0..1,
        }
    }

    #[test]
    fn duration_scales_inversely_with_wpm() {
        let t = token(1.0, 0);
        assert_eq!(t.duration_ms(300), 200);
        assert_eq!(t.duration_ms(600), 100);
    }

    #[test]
    fn structural_pause_is_not_scaled_by_wpm() {
        let t = token(1.0, 250);
        assert_eq!(t.duration_ms(300), 450);
        assert_eq!(t.duration_ms(600), 350);
    }

    #[test]
    fn duration_never_divides_by_zero() {
        assert!(token(1.0, 0).duration_ms(0) > 0);
    }

    #[test]
    fn tables_are_excluded_from_the_rsvp_stream() {
        assert!(!BlockKind::Table.is_readable());
        assert!(BlockKind::Paragraph.is_readable());
    }
}
