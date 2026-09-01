//! Format-specific parsers producing a [`Document`].
//!
//! Implements OXD-016 and OXD-017. See spec §8.

pub mod markdown;
pub mod plaintext;

use crate::token::Document;

pub use markdown::parse as parse_markdown;
pub use plaintext::parse as parse_plaintext;

/// Document formats the engine can currently read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// CommonMark and its common extensions.
    Markdown,
    /// Unstructured text, paragraphs separated by blank lines.
    PlainText,
}

impl Format {
    /// Guess a format from a file extension.
    pub fn from_extension(ext: Option<&str>) -> Format {
        match ext.map(str::to_ascii_lowercase).as_deref() {
            Some("md" | "markdown" | "mdx") => Format::Markdown,
            _ => Format::PlainText,
        }
    }

    /// Guess a format from the content itself, for stdin and extensionless files.
    pub fn sniff(text: &str) -> Format {
        let markdownish = text
            .lines()
            .take(200)
            .filter(|l| {
                l.starts_with('#')
                    || l.starts_with("- ")
                    || l.starts_with("* ")
                    || l.starts_with("```")
                    || l.starts_with("> ")
                    || l.starts_with('|')
            })
            .count();
        if markdownish >= 3 { Format::Markdown } else { Format::PlainText }
    }

    /// Short identifier stored in [`crate::token::DocumentMeta::format`].
    pub const fn name(self) -> &'static str {
        match self {
            Format::Markdown => "markdown",
            Format::PlainText => "plaintext",
        }
    }
}

/// Parse `text` (already sanitized) using the given format.
pub fn parse(text: &str, format: Format) -> Document {
    match format {
        Format::Markdown => markdown::parse(text),
        Format::PlainText => plaintext::parse(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_drives_format_selection() {
        assert_eq!(Format::from_extension(Some("md")), Format::Markdown);
        assert_eq!(Format::from_extension(Some("MD")), Format::Markdown);
        assert_eq!(Format::from_extension(Some("txt")), Format::PlainText);
        assert_eq!(Format::from_extension(None), Format::PlainText);
    }

    #[test]
    fn sniffing_recognises_markdown_structure() {
        let md = "# Title\n\n- one\n- two\n\n```rust\nfn main() {}\n```\n";
        assert_eq!(Format::sniff(md), Format::Markdown);
        assert_eq!(Format::sniff("Just a plain sentence.\n\nAnother one.\n"), Format::PlainText);
    }
}
