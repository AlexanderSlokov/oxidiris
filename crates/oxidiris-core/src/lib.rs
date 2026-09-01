//! Core text engine for Oxidiris: RSVP tokenization, Optimal Recognition Point, pacing and
//! document parsing.
//!
//! # Architectural constraint
//!
//! This crate must never depend on `ratatui`, `crossterm`, or any other terminal I/O crate. It is
//! the piece that gets reused for the WebAssembly build (OXD-080), a future GUI, and editor
//! plugins. CI enforces the rule by building this crate for `wasm32-unknown-unknown`; if a
//! terminal dependency sneaks in, that job fails.
//!
//! # Pipeline
//!
//! ```text
//! bytes -> encoding::decode -> segment::sanitize -> parser::parse -> Document
//!       -> pacing::tokenize -> Vec<Token> -> Player
//! ```
//!
//! Sanitisation happens exactly once, before parsing. Everything downstream then indexes into the
//! normalized [`Document::source`], which is what makes NFC and NFD inputs produce byte-identical
//! results.
//!
//! # Example
//!
//! ```
//! use oxidiris_core::{pacing::PacingMode, parser, player::Player, segment};
//!
//! let text = segment::sanitize("# Title\n\nHello there, reader.\n");
//! let doc = parser::parse_markdown(&text);
//! let mut player = Player::from_document(&doc, 300, PacingMode::Natural);
//!
//! player.play();
//! assert_eq!(player.current().unwrap().text, "Title");
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod encoding;
pub mod orp;
pub mod pacing;
pub mod parser;
pub mod player;
pub mod segment;
pub mod sentence;
pub mod token;

pub use pacing::{DEFAULT_WPM, MAX_WPM, MIN_WPM, PacingMode};
pub use player::{PlayState, Player};
pub use token::{Block, BlockKind, Document, Heading, Token, TokenKind};

/// Load a document from raw bytes: decode, sanitize, and parse.
///
/// `format` may be `None`, in which case the format is sniffed from the content.
pub fn load(
    bytes: &[u8],
    format: Option<parser::Format>,
) -> Result<Document, encoding::DecodeError> {
    let (raw, _encoding) = encoding::decode(bytes)?;
    let text = segment::sanitize(&raw);
    let format = format.unwrap_or_else(|| parser::Format::sniff(&text));
    Ok(parser::parse(&text, format))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_sniffs_markdown_and_builds_a_document() {
        let src = b"# Title\n\n- a\n- b\n\n> quote\n\nBody text.\n";
        let doc = load(src, None).unwrap();
        assert_eq!(doc.meta.format, Some("markdown"));
        assert_eq!(doc.headings.len(), 1);
    }

    #[test]
    fn load_rejects_binary_input() {
        assert!(load(&[0x00, 0x01, 0x02, 0x03], None).is_err());
    }

    /// End-to-end: the two Unicode normalization forms must be indistinguishable downstream.
    #[test]
    fn nfc_and_nfd_documents_are_byte_identical() {
        let nfc = "Ti\u{1EBF}ng Vi\u{1EC7}t r\u{1EA5}t hay.\n";
        let nfd = "Tie\u{302}\u{301}ng Vie\u{323}\u{302}t ra\u{301}\u{302}t hay.\n";
        let a = load(nfc.as_bytes(), Some(parser::Format::PlainText)).unwrap();
        let b = load(nfd.as_bytes(), Some(parser::Format::PlainText)).unwrap();
        assert_eq!(a.source, b.source);

        let ta = pacing::tokenize(&a, PacingMode::Natural);
        let tb = pacing::tokenize(&b, PacingMode::Natural);
        assert_eq!(ta, tb);
    }

    /// Every token span must be a valid slice of the document source, on real content.
    #[test]
    fn token_spans_are_always_valid_slices() {
        let doc = load(include_bytes!("../../../BACKLOG.md"), None).unwrap();
        let tokens = pacing::tokenize(&doc, PacingMode::Natural);
        assert!(tokens.len() > 500, "BACKLOG.md should yield a substantial stream");
        for t in &tokens {
            assert!(doc.source.get(t.byte_span.clone()).is_some(), "invalid span for {:?}", t.text);
        }
    }
}