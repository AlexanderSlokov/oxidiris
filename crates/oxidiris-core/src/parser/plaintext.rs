//! Plain-text parser.
//!
//! Implements OXD-016. See spec §8.5.
//!
//! Simple, but it is also the reference implementation for every later parser: it establishes
//! that a block's readable text is assembled from [`Fragment`]s that each map back to an exact
//! byte range in the source.

use crate::token::{Block, BlockKind, Document, DocumentMeta, Fragment, TokenKind};

/// Parse `text` into paragraphs separated by blank lines.
///
/// # Hard-wrapped input
///
/// RFCs and man pages wrap at column 72 and use blank lines for real paragraph breaks. Treating
/// every newline as a paragraph boundary would shatter such a file into dozens of one-line
/// pseudo-paragraphs, so lines inside a paragraph are rejoined (spec §8.5).
pub fn parse(text: &str) -> Document {
    let mut blocks = Vec::new();
    let mut current: Vec<(usize, usize)> = Vec::new(); // line ranges of the paragraph being built
    let mut offset = 0usize;

    let flush = |lines: &mut Vec<(usize, usize)>, blocks: &mut Vec<Block>| {
        if lines.is_empty() {
            return;
        }
        let id = blocks.len();
        let span_start = lines[0].0;
        let span_end = lines[lines.len() - 1].1;
        let fragments: Vec<Fragment> = lines
            .iter()
            .map(|(s, e)| Fragment {
                text: text[*s..*e].to_string(),
                byte_span: *s..*e,
                kind: TokenKind::Word,
            })
            .collect();
        let joined = fragments.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(" ");
        blocks.push(Block {
            id,
            kind: BlockKind::Paragraph,
            text: joined,
            byte_span: span_start..span_end,
            fragments,
        });
        lines.clear();
    };

    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let trimmed = line.trim_end_matches(['\n', '\r']);
        let content = trimmed.trim();

        if content.is_empty() {
            flush(&mut current, &mut blocks);
            continue;
        }
        // Record the trimmed span so byte offsets stay tight around the actual words.
        let lead = trimmed.len() - trimmed.trim_start().len();
        let trail = trimmed.len() - trimmed.trim_end().len();
        current.push((start + lead, start + trimmed.len() - trail));
    }
    flush(&mut current, &mut blocks);

    let title = blocks.first().map(|b| b.text.chars().take(80).collect::<String>());

    Document {
        source: text.to_string(),
        blocks,
        headings: Vec::new(),
        meta: DocumentMeta { title, format: Some("plaintext") },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::sanitize;

    #[test]
    fn blank_lines_separate_paragraphs() {
        let doc = parse("First para.\n\nSecond para.\n");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.blocks[0].text, "First para.");
        assert_eq!(doc.blocks[1].text, "Second para.");
    }

    /// The OXD-016 acceptance case: an RFC-style file must not explode into one block per line.
    #[test]
    fn hard_wrapped_lines_are_rejoined_into_one_paragraph() {
        let rfc = "This document specifies an Internet standards track\n\
                   protocol for the Internet community, and requests\n\
                   discussion and suggestions for improvements.\n\
                   \n\
                   Distribution of this memo is unlimited.\n";
        let doc = parse(rfc);
        assert_eq!(doc.blocks.len(), 2, "expected 2 paragraphs, got {}", doc.blocks.len());
        assert!(doc.blocks[0].text.contains("standards track protocol"));
    }

    #[test]
    fn multiple_blank_lines_do_not_create_empty_blocks() {
        let doc = parse("A.\n\n\n\n\nB.\n");
        assert_eq!(doc.blocks.len(), 2);
    }

    #[test]
    fn empty_input_yields_no_blocks() {
        let doc = parse("");
        assert!(doc.blocks.is_empty());
        assert!(doc.headings.is_empty());
    }

    /// The invariant every parser must uphold: a fragment's byte span must address exactly the
    /// text it claims, inside `Document::source`.
    #[test]
    fn fragment_spans_address_their_own_text() {
        let text = sanitize("Tiếng Việt có dấu.\n\nDòng thứ hai ở đây.\n");
        let doc = parse(&text);
        for block in &doc.blocks {
            for f in &block.fragments {
                assert_eq!(&doc.source[f.byte_span.clone()], f.text);
            }
        }
    }

    #[test]
    fn leading_whitespace_is_excluded_from_spans() {
        let doc = parse("    indented line\n");
        let f = &doc.blocks[0].fragments[0];
        assert_eq!(f.text, "indented line");
        assert_eq!(&doc.source[f.byte_span.clone()], "indented line");
    }
}
