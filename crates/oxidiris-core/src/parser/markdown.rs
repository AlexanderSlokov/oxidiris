//! Markdown parser built on `pulldown-cmark`.
//!
//! Implements OXD-017. See spec §8.1.
//!
//! The job is to flatten the event stream into readable blocks while throwing away everything
//! that would pollute the reading rhythm: URLs, list bullets, fence markers.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::token::{Block, BlockKind, Document, DocumentMeta, Fragment, Heading, TokenKind};

/// A block under construction.
struct OpenBlock {
    kind: BlockKind,
    start: usize,
    end: usize,
    fragments: Vec<Fragment>,
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    headings: Vec<Heading>,
    open: Option<OpenBlock>,
    quote_depth: usize,
    in_code_block: bool,
    in_table: bool,
    link_depth: usize,
}

impl Builder {
    fn open_block(&mut self, kind: BlockKind, start: usize) {
        self.close_block();
        self.open = Some(OpenBlock { kind, start, end: start, fragments: Vec::new() });
    }

    fn close_block(&mut self) {
        let Some(open) = self.open.take() else { return };
        if open.fragments.is_empty() {
            return;
        }
        let text = open
            .fragments
            .iter()
            .map(|f| f.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if text.is_empty() {
            return;
        }
        let id = self.blocks.len();
        if let BlockKind::Heading(level) = open.kind {
            self.headings.push(Heading { level, text: text.clone(), block_id: id });
        }
        self.blocks.push(Block {
            id,
            kind: open.kind,
            text,
            byte_span: open.start..open.end,
            fragments: open.fragments,
        });
    }

    fn push_text(&mut self, text: &str, span: core::ops::Range<usize>, kind: TokenKind) {
        if text.trim().is_empty() {
            return;
        }
        // Drop bare URLs and autolinks. The link *label* is kept; the target never is, because a
        // URL read aloud one token at a time is pure noise (spec §8.1).
        if looks_like_url(text) {
            return;
        }
        let Some(open) = self.open.as_mut() else { return };
        open.end = open.end.max(span.end);
        open.fragments.push(Fragment { text: text.to_string(), byte_span: span, kind });
    }
}

/// Whether a piece of text is a URL rather than prose.
fn looks_like_url(text: &str) -> bool {
    let t = text.trim();
    t.contains("://") || t.starts_with("www.") || t.starts_with("mailto:")
}

const fn level_of(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Parse Markdown `text` (already sanitized) into a [`Document`].
pub fn parse(text: &str) -> Document {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut b = Builder::default();

    for (event, span) in Parser::new_ext(text, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                b.open_block(BlockKind::Heading(level_of(level)), span.start);
            }
            Event::End(TagEnd::Heading(_)) => b.close_block(),

            Event::Start(Tag::CodeBlock(kind)) => {
                b.in_code_block = true;
                let _ = matches!(kind, CodeBlockKind::Fenced(_));
                b.open_block(BlockKind::CodeBlock, span.start);
            }
            Event::End(TagEnd::CodeBlock) => {
                b.in_code_block = false;
                b.close_block();
            }

            Event::Start(Tag::BlockQuote(_)) => b.quote_depth += 1,
            Event::End(TagEnd::BlockQuote(_)) => b.quote_depth = b.quote_depth.saturating_sub(1),

            Event::Start(Tag::Item) => b.open_block(BlockKind::ListItem, span.start),
            Event::End(TagEnd::Item) => b.close_block(),

            Event::Start(Tag::Table(_)) => {
                b.in_table = true;
                b.open_block(BlockKind::Table, span.start);
            }
            Event::End(TagEnd::Table) => {
                b.in_table = false;
                b.close_block();
            }

            // Inside a list item or table cell the surrounding block already exists; a loose list
            // would otherwise split each item into a separate paragraph block.
            Event::Start(Tag::Paragraph) if b.open.is_none() => {
                let kind =
                    if b.quote_depth > 0 { BlockKind::Quote } else { BlockKind::Paragraph };
                b.open_block(kind, span.start);
            }
            Event::End(TagEnd::Paragraph) => {
                let inside_container = matches!(
                    b.open.as_ref().map(|o| o.kind),
                    Some(BlockKind::ListItem | BlockKind::Table)
                );
                if !inside_container {
                    b.close_block();
                }
            }

            Event::Start(Tag::Link { .. }) => b.link_depth += 1,
            Event::End(TagEnd::Link) => b.link_depth = b.link_depth.saturating_sub(1),

            Event::Text(t) => {
                let kind = if b.in_code_block {
                    TokenKind::Code
                } else if b.in_table {
                    TokenKind::Table
                } else {
                    TokenKind::Word
                };
                b.push_text(&t, span, kind);
            }
            Event::Code(t) => {
                let kind = if b.in_table { TokenKind::Table } else { TokenKind::Code };
                b.push_text(&t, span, kind);
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => {
                b.push_text(&t, span, TokenKind::Math);
            }
            _ => {}
        }
    }
    b.close_block();

    let title = b
        .headings
        .iter()
        .find(|h| h.level == 1)
        .map(|h| h.text.clone())
        .or_else(|| b.headings.first().map(|h| h.text.clone()));

    Document {
        source: text.to_string(),
        blocks: b.blocks,
        headings: b.headings,
        meta: DocumentMeta { title, format: Some("markdown") },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::sanitize;

    const SAMPLE: &str = "\
# Oxidiris

A tool for reading [the docs](https://example.com/docs) fast.

## Features

- Blazing fast
- No eye movement

```rust
fn main() { println!(\"hi\"); }
```

| Column | Value |
|---|---|
| a | 1 |

> A quoted remark.
";

    #[test]
    fn heading_tree_is_extracted_with_levels() {
        let doc = parse(SAMPLE);
        let levels: Vec<u8> = doc.headings.iter().map(|h| h.level).collect();
        let texts: Vec<&str> = doc.headings.iter().map(|h| h.text.as_str()).collect();
        assert_eq!(levels, vec![1, 2]);
        assert_eq!(texts, vec!["Oxidiris", "Features"]);
        assert_eq!(doc.meta.title.as_deref(), Some("Oxidiris"));
    }

    /// OXD-017 acceptance: no URL may reach the token stream.
    #[test]
    fn urls_are_stripped_but_link_labels_survive() {
        let doc = parse(SAMPLE);
        let all: String = doc.blocks.iter().map(|b| b.text.clone()).collect::<Vec<_>>().join(" ");
        assert!(!all.contains("://"), "a URL leaked into the token stream: {all}");
        assert!(!all.contains("example.com"));
        assert!(all.contains("the docs"), "link label was lost");
    }

    #[test]
    fn autolinks_are_dropped_entirely() {
        let doc = parse("See <https://example.com/page> for details.\n");
        let all: String = doc.blocks.iter().map(|b| b.text.clone()).collect();
        assert!(!all.contains("://"));
        assert!(all.contains("for details"));
    }

    #[test]
    fn code_block_is_kept_whole_and_marked_as_code() {
        let doc = parse(SAMPLE);
        let code = doc.blocks.iter().find(|b| b.kind == BlockKind::CodeBlock).unwrap();
        assert!(code.text.contains("fn main()"));
        assert!(code.fragments.iter().all(|f| f.kind == TokenKind::Code));
    }

    #[test]
    fn list_items_become_separate_blocks() {
        let doc = parse(SAMPLE);
        let items: Vec<&str> = doc
            .blocks
            .iter()
            .filter(|b| b.kind == BlockKind::ListItem)
            .map(|b| b.text.as_str())
            .collect();
        assert_eq!(items, vec!["Blazing fast", "No eye movement"]);
    }

    #[test]
    fn tables_are_parsed_but_marked_unreadable() {
        let doc = parse(SAMPLE);
        let table = doc.blocks.iter().find(|b| b.kind == BlockKind::Table).unwrap();
        assert!(!table.kind.is_readable());
        assert!(table.text.contains("Column"));
    }

    #[test]
    fn block_quotes_are_recognised() {
        let doc = parse(SAMPLE);
        assert!(doc.blocks.iter().any(|b| b.kind == BlockKind::Quote));
    }

    #[test]
    fn fragment_spans_stay_within_the_source() {
        let text = sanitize(SAMPLE);
        let doc = parse(&text);
        for block in &doc.blocks {
            assert!(block.byte_span.end <= doc.source.len());
            for f in &block.fragments {
                assert!(f.byte_span.end <= doc.source.len(), "fragment span out of bounds");
                assert!(f.byte_span.start <= f.byte_span.end);
            }
        }
    }

    #[test]
    fn plain_text_fragments_address_their_own_bytes() {
        // Without escapes or entities, a Text event maps verbatim onto the source.
        let text = sanitize("Một đoạn tiếng Việt bình thường.\n");
        let doc = parse(&text);
        let f = &doc.blocks[0].fragments[0];
        assert_eq!(&doc.source[f.byte_span.clone()], f.text);
    }

    #[test]
    fn empty_document_is_handled() {
        let doc = parse("");
        assert!(doc.blocks.is_empty());
        assert!(doc.headings.is_empty());
        assert!(doc.meta.title.is_none());
    }
}