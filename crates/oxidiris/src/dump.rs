//! Plain-text output.
//!
//! Implements OXD-026. See spec §3.4.3 and §4.4.
//!
//! This is both the parser debugging tool and a genuine accessibility route: a screen reader
//! cannot follow text that rewrites itself several times a second, so there has to be a way to
//! get the cleaned document out as ordinary text.

use std::io::Write;

use anyhow::Result;
use oxidiris_core::Document;

/// Write the document as clean plain text.
pub fn write_plain(out: &mut impl Write, doc: &Document) -> Result<()> {
    write!(out, "{}", doc.to_plain_text())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidiris_core::{parser, segment};

    fn dump(md: &str) -> String {
        let doc = parser::parse_markdown(&segment::sanitize(md));
        let mut buf = Vec::new();
        write_plain(&mut buf, &doc).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn output_is_readable_text_with_no_escape_sequences() {
        let text = dump("# Title\n\nSome *emphasised* words and [a link](https://example.com).\n");
        assert!(!text.contains('\u{1b}'), "escape sequence leaked into plain output");
        assert!(text.contains("Title"));
        assert!(text.contains("emphasised"));
    }

    #[test]
    fn urls_do_not_survive_into_plain_output() {
        let text = dump("See [the docs](https://example.com/x) and <https://example.org>.\n");
        assert!(!text.contains("://"), "got {text:?}");
        assert!(text.contains("the docs"));
    }

    #[test]
    fn structure_is_preserved_as_simple_markers() {
        let text = dump("# H1\n\n## H2\n\n- item one\n- item two\n\n> quoted\n");
        assert!(text.contains("# H1"));
        assert!(text.contains("## H2"));
        assert!(text.contains("- item one"));
        assert!(text.contains("> quoted"));
    }

    #[test]
    fn an_empty_document_produces_empty_output() {
        assert_eq!(dump(""), "");
    }

    #[test]
    fn the_projects_own_backlog_dumps_cleanly() {
        let doc = oxidiris_core::load(include_bytes!("../../../BACKLOG.md"), None).unwrap();
        let mut buf = Vec::new();
        write_plain(&mut buf, &doc).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.len() > 5_000);
        assert!(!text.contains('\u{1b}'));
    }
}
