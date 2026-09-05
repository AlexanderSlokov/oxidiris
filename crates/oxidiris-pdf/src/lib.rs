//! PDF text extraction for Oxidiris.
//!
//! Implements OXD-060. See spec §8.2 and `docs/decisions/pdf-extraction.md`.
//!
//! # Why this is a crate of its own
//!
//! `oxidiris-core` must keep building for `wasm32-unknown-unknown`, and the PDF backend drags in
//! font parsers, cryptography and image codecs that have no business in that build. Keeping the
//! dependency here means the core stays small and the wasm job keeps passing.
//!
//! # Where this sits in the pipeline
//!
//! ```text
//! pdf bytes -> oxidiris_pdf::extract -> plain text -> oxidiris_core::load -> Document
//! ```
//!
//! The output is deliberately plain text rather than a `Document`: a PDF carries no reliable
//! block structure, so the existing plain-text parser — which already rejoins hard-wrapped lines
//! — is exactly the right consumer.
//!
//! # Example
//!
//! ```no_run
//! let bytes = std::fs::read("paper.pdf").unwrap();
//! let text = oxidiris_pdf::extract(&bytes).unwrap();
//! assert!(text.contains(' '));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cleanup;

use std::panic::{self, AssertUnwindSafe};

use pdf_extract::{Document, PlainTextOutput, output_doc};
use thiserror::Error;

pub use cleanup::tidy;

/// Why a PDF could not be turned into readable text.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PdfError {
    /// The bytes do not begin with a PDF header.
    #[error("not a PDF: expected the file to start with `%PDF-`, found {head:?}")]
    NotPdf {
        /// The first few bytes, rendered printably, so the message names the offending value.
        head: String,
    },

    /// The file is password protected.
    #[error("the PDF is encrypted; oxidiris cannot open password-protected files")]
    Encrypted,

    /// The backend could not make sense of the file structure.
    #[error("the PDF is damaged or uses an unsupported feature: {detail}")]
    Malformed {
        /// What the backend reported.
        detail: String,
    },

    /// The pages carry no text, only images.
    #[error(
        "the PDF has no text layer ({pages} {} scanned); it is probably a scan and needs OCR",
        if *pages == 1 { "page" } else { "pages" }
    )]
    NoTextLayer {
        /// How many pages were inspected.
        pages: usize,
    },
}

/// Number of leading bytes quoted back in a [`PdfError::NotPdf`] message.
const HEAD_LEN: usize = 8;

/// Whether `bytes` start with a PDF header.
///
/// Used to route a file to this crate before the core's text decoder rejects it as binary, and to
/// recognise a PDF arriving on standard input, where there is no extension to go by.
///
/// ```
/// assert!(oxidiris_pdf::looks_like_pdf(b"%PDF-1.7\n..."));
/// assert!(!oxidiris_pdf::looks_like_pdf(b"# Just markdown\n"));
/// ```
pub fn looks_like_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

/// Extract readable plain text from the bytes of a PDF.
///
/// The returned text is already tidied for reading: ligatures expanded, hyphenated line breaks
/// welded back together, isolated short numbers removed. Feed it straight to
/// `oxidiris_core::load` as plain text.
///
/// ```no_run
/// let bytes = std::fs::read("paper.pdf").unwrap();
/// match oxidiris_pdf::extract(&bytes) {
///     Ok(text) => println!("{} characters", text.len()),
///     Err(err) => eprintln!("{err}"),
/// }
/// ```
pub fn extract(bytes: &[u8]) -> Result<String, PdfError> {
    if !looks_like_pdf(bytes) {
        return Err(PdfError::NotPdf { head: printable_head(bytes) });
    }

    let document = load(bytes)?;
    if document.is_encrypted() {
        return Err(PdfError::Encrypted);
    }

    let pages = document.get_pages().len();
    let text = tidy(&render(&document)?);
    if !text.chars().any(char::is_alphanumeric) {
        return Err(PdfError::NoTextLayer { pages });
    }
    Ok(text)
}

/// Parse the file structure.
fn load(bytes: &[u8]) -> Result<Document, PdfError> {
    let outcome = guard(|| Document::load_mem(bytes))?;
    outcome.map_err(|e| PdfError::Malformed { detail: e.to_string() })
}

/// Walk every page and collect the text in content-stream order.
fn render(document: &Document) -> Result<String, PdfError> {
    let mut text = String::new();
    let outcome = guard(|| {
        let mut sink = PlainTextOutput::new(&mut text);
        output_doc(document, &mut sink)
    })?;
    outcome.map_err(|e| PdfError::Malformed { detail: e.to_string() })?;
    Ok(text)
}

/// Run `job`, turning a panic inside the PDF backend into a [`PdfError::Malformed`].
///
/// `pdf-extract` panics instead of returning `Err` on several classes of broken file — unknown
/// font encodings, truncated cross-reference tables, unbalanced content streams. Spec §4.4 says a
/// reader must answer those with a clean message, not a crash, so the panic is caught here.
///
/// The default panic printer is silenced for the duration of the call, otherwise the backtrace
/// would still reach the terminal. That hook is process-wide: a panic on another thread during
/// this window would be swallowed too. Extraction happens once, before any threads or the
/// terminal exist, so the window is closed again long before that could matter.
fn guard<T>(job: impl FnOnce() -> T) -> Result<T, PdfError> {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let outcome = panic::catch_unwind(AssertUnwindSafe(job));
    panic::set_hook(previous);
    outcome.map_err(|payload| PdfError::Malformed { detail: panic_detail(&payload) })
}

/// Recover the message from a caught panic payload.
fn panic_detail(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "the PDF backend panicked".to_string()
}

/// Render the first few bytes printably, for the "this is not a PDF" message.
fn printable_head(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(HEAD_LEN)
        .map(|b| {
            if b.is_ascii_graphic() || *b == b' ' {
                (*b as char).to_string()
            } else {
                format!("\\x{b:02x}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pdf_header_is_what_identifies_a_pdf() {
        assert!(looks_like_pdf(b"%PDF-1.4\n"));
        assert!(!looks_like_pdf(b"%PDF"), "a truncated header is not a PDF");
        assert!(!looks_like_pdf(b""));
        assert!(!looks_like_pdf(b"# Markdown\n"));
    }

    #[test]
    fn non_pdf_input_is_rejected_with_the_offending_bytes_in_the_message() {
        let err = extract(b"\x00\x01not a pdf").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("\\x00"), "message should quote the head: {message}");
        assert!(message.contains("%PDF-"), "message should name the expected shape: {message}");
    }

    #[test]
    fn empty_input_is_rejected_rather_than_panicking() {
        assert!(matches!(extract(b""), Err(PdfError::NotPdf { .. })));
    }

    /// Spec §4.4: a damaged file must produce a message, never a crash.
    #[test]
    fn a_truncated_pdf_is_an_error_not_a_panic() {
        let err = extract(b"%PDF-1.7\nthis file stops right here").unwrap_err();
        assert!(matches!(err, PdfError::Malformed { .. }), "got {err:?}");
    }

    /// A one-page scan is the common case; "1 pages scanned" would look like a bug in the tool.
    #[test]
    fn the_no_text_message_counts_pages_in_the_right_number() {
        assert!(PdfError::NoTextLayer { pages: 1 }.to_string().contains("1 page scanned"));
        assert!(PdfError::NoTextLayer { pages: 14 }.to_string().contains("14 pages scanned"));
    }

    #[test]
    fn printable_head_escapes_non_printable_bytes() {
        assert_eq!(printable_head(b"%PDF-1.7extra"), "%PDF-1.7");
        assert_eq!(printable_head(&[0x00, 0xff]), "\\x00\\xff");
    }
}
