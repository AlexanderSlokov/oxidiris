//! Corpus-driven tests against the generated PDF fixtures.
//!
//! Implements the verification half of OXD-060. The fixtures are produced by
//! `testdata/make_pdf_fixtures.py`; see `testdata/README.md` for what each one pins down.

use std::path::{Path, PathBuf};

use oxidiris_pdf::PdfError;

/// Every generated fixture, in the order `testdata/README.md` lists them.
const FIXTURES: [&str; 3] = ["pdf_typography.pdf", "pdf_two_column.pdf", "pdf_no_text_layer.pdf"];

fn testdata(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(name)
}

fn extract(name: &str) -> Result<String, PdfError> {
    let bytes = std::fs::read(testdata(name)).unwrap_or_else(|e| panic!("cannot read {name}: {e}"));
    oxidiris_pdf::extract(&bytes)
}

#[test]
fn every_fixture_is_recognised_as_a_pdf() {
    for name in FIXTURES {
        let bytes = std::fs::read(testdata(name)).unwrap();
        assert!(oxidiris_pdf::looks_like_pdf(&bytes), "{name} lost its header");
    }
}

/// Regression guard for the `windows-latest` failure on PR #13.
///
/// These fixtures are almost all ASCII, so git's own heuristic calls them text and rewrites their
/// line endings on a Windows checkout. One added byte per line shifts every offset in the
/// cross-reference table, and the file stops parsing — reported as `InvalidTrailer`, which points
/// nowhere near the cause, on a file nobody touched. `*.pdf binary` in `.gitattributes` is what
/// prevents it; failing here says so out loud instead.
#[test]
fn fixtures_are_checked_out_byte_for_byte() {
    for name in FIXTURES {
        let bytes = std::fs::read(testdata(name)).unwrap();
        assert!(
            !bytes.windows(2).any(|pair| pair == b"\r\n"),
            "{name} has CRLF line endings: the checkout rewrote it. \
             `.gitattributes` must keep `*.pdf binary`."
        );
    }
}

/// The three artefacts that would otherwise reach the RSVP frame, on a real PDF rather than a
/// hand-written string.
#[test]
fn typography_artefacts_do_not_survive_extraction() {
    let text = extract("pdf_typography.pdf").expect("fixture should extract");

    assert!(text.contains("efficient"), "ligature not expanded: {text:?}");
    assert!(!text.contains('\u{FB01}'), "raw fi ligature left in: {text:?}");
    assert!(text.contains("admission control"), "hyphenated break not welded: {text:?}");
    assert!(!text.contains("con-"), "hyphen left behind: {text:?}");
    assert!(!text.contains("12"), "page number survived: {text:?}");
}

/// A hyphen the author typed is not a line-break hyphen and must survive.
#[test]
fn a_real_compound_keeps_its_hyphen() {
    let text = extract("pdf_typography.pdf").unwrap();
    assert!(text.contains("process-level"), "got {text:?}");
}

/// Spec §8.2: no column interleaving. The left column must be finished before the right starts.
#[test]
fn two_columns_are_read_one_after_the_other() {
    let text = extract("pdf_two_column.pdf").expect("fixture should extract");

    let left_end = text.find("bottom of the").expect("left column missing");
    let right_start = text.find("right column is read second").expect("right column missing");
    assert!(left_end < right_start, "columns interleaved: {text:?}");

    // Line breaks are still present here; rejoining them into paragraphs is the plain-text
    // parser's job, one layer further on.
    assert!(
        text.contains("The left column starts here and\ncontinues to the bottom of the\npage."),
        "left column lost its own lines: {text:?}"
    );
}

/// Spec §4.4: a scan must produce an explanation, not an empty reader or a crash.
#[test]
fn a_page_without_a_text_layer_is_reported_as_such() {
    let err = extract("pdf_no_text_layer.pdf").unwrap_err();
    assert_eq!(err, PdfError::NoTextLayer { pages: 1 });
    assert!(err.to_string().contains("OCR"), "message should say why: {err}");
}
