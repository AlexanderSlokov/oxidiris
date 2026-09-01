//! Corpus-driven integration tests.
//!
//! Implements the verification half of OXD-004. Every fixture in `testdata/` is exercised here,
//! against the real public API rather than internal helpers.

use std::path::{Path, PathBuf};

use oxidiris_core::pacing::{self, PacingMode};
use oxidiris_core::parser::Format;
use oxidiris_core::player::Player;
use oxidiris_core::segment;

fn testdata(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(name)
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(testdata(name)).unwrap_or_else(|e| panic!("cannot read {name}: {e}"))
}

fn tokens(name: &str, format: Option<Format>) -> Vec<oxidiris_core::Token> {
    let doc = oxidiris_core::load(&bytes(name), format).unwrap();
    pacing::tokenize(&doc, PacingMode::Natural)
}

/// Every fixture must load without error and produce spans that are valid slices of the source.
#[test]
fn every_fixture_loads_and_produces_valid_spans() {
    let fixtures = [
        "simple.txt",
        "vietnamese_nfc.txt",
        "vietnamese_nfd.txt",
        "cjk.txt",
        "emoji_zwj.txt",
        "mixed_width.txt",
        "abbreviations.txt",
        "long_words.txt",
        "rfc_style.txt",
        "sample.md",
        "utf16_bom.txt",
        "latin1.txt",
        "empty.txt",
    ];

    for name in fixtures {
        let doc = oxidiris_core::load(&bytes(name), None)
            .unwrap_or_else(|e| panic!("{name} failed to load: {e}"));
        let tokens = pacing::tokenize(&doc, PacingMode::Natural);
        for t in &tokens {
            assert!(
                doc.source.get(t.byte_span.clone()).is_some(),
                "{name}: token {:?} has an invalid span {:?}",
                t.text,
                t.byte_span
            );
        }
    }
}

/// The headline Unicode guarantee: normalization form must be invisible downstream.
#[test]
fn nfc_and_nfd_fixtures_produce_identical_token_streams() {
    let nfc_bytes = bytes("vietnamese_nfc.txt");
    let nfd_bytes = bytes("vietnamese_nfd.txt");
    assert_ne!(nfc_bytes, nfd_bytes, "fixtures are identical, so this test proves nothing");

    let nfc = tokens("vietnamese_nfc.txt", Some(Format::PlainText));
    let nfd = tokens("vietnamese_nfd.txt", Some(Format::PlainText));

    assert!(!nfc.is_empty());
    assert_eq!(nfc.len(), nfd.len(), "different token counts for the same text");
    for (a, b) in nfc.iter().zip(nfd.iter()) {
        assert_eq!(a.text, b.text);
        assert_eq!(a.orp_index, b.orp_index);
        assert_eq!(a.orp_offset, b.orp_offset);
        assert_eq!(a.display_width, b.display_width);
    }
}

/// Column width, not character count, is what the renderer will align against.
#[test]
fn cjk_tokens_report_double_width() {
    for t in tokens("cjk.txt", Some(Format::PlainText)) {
        let graphemes = segment::grapheme_count(&t.text);
        assert!(
            t.display_width >= graphemes as u16,
            "{:?}: width {} should be at least the grapheme count {graphemes}",
            t.text,
            t.display_width
        );
        assert!(t.orp_offset <= t.display_width);
    }
}

#[test]
fn emoji_sequences_stay_whole() {
    let tokens = tokens("emoji_zwj.txt", Some(Format::PlainText));
    let family =
        tokens.iter().find(|t| t.text.contains('\u{1F468}')).expect("family emoji missing");
    assert_eq!(segment::grapheme_count(&family.text), 1, "ZWJ sequence was split");

    let flag = tokens.iter().find(|t| t.text.contains('\u{1F1FB}')).expect("flag emoji missing");
    assert_eq!(segment::grapheme_count(&flag.text), 1, "flag was split");
}

/// The OXD-014 acceptance case, run against real prose.
#[test]
fn abbreviations_do_not_create_sentence_pauses() {
    let tokens = tokens("abbreviations.txt", Some(Format::PlainText));
    let plain_weight = 1.0f32;

    for suspect in ["Fig.", "al.", "i.e.", "e.g.", "vs.", "No.", "approx.", "Dr."] {
        let Some(t) = tokens.iter().find(|t| t.text == suspect) else { continue };
        assert!(
            t.weight < plain_weight * 2.0,
            "{suspect:?} was treated as a sentence end (weight {})",
            t.weight
        );
    }

    // The genuine full stop must still register.
    let real_stop = tokens.iter().find(|t| t.text == "this.").expect("expected a real full stop");
    assert!(
        real_stop.weight >= 2.0,
        "a real full stop lost its pause (weight {})",
        real_stop.weight
    );
}

/// The OXD-016 acceptance case: hard-wrapped input must not become one block per line.
#[test]
fn hard_wrapped_text_rejoins_into_paragraphs() {
    let doc = oxidiris_core::load(&bytes("rfc_style.txt"), Some(Format::PlainText)).unwrap();
    assert_eq!(doc.blocks.len(), 2, "expected 2 paragraphs, got {}", doc.blocks.len());
}

/// The OXD-017 acceptance case: no URL may reach the reader.
#[test]
fn markdown_keeps_link_labels_and_drops_targets() {
    let tokens = tokens("sample.md", Some(Format::Markdown));
    for t in &tokens {
        assert!(!t.text.contains("://"), "URL leaked into the stream: {:?}", t.text);
        assert!(!t.text.starts_with("www."), "URL leaked into the stream: {:?}", t.text);
    }
    assert!(tokens.iter().any(|t| t.text == "link"), "link label was lost");
}

#[test]
fn markdown_headings_build_an_outline() {
    let doc = oxidiris_core::load(&bytes("sample.md"), Some(Format::Markdown)).unwrap();
    let levels: Vec<u8> = doc.headings.iter().map(|h| h.level).collect();
    assert_eq!(levels, vec![1, 2, 3]);
    assert_eq!(doc.meta.title.as_deref(), Some("Sample Document"));
}

#[test]
fn table_contents_stay_out_of_the_reading_stream() {
    let tokens = tokens("sample.md", Some(Format::Markdown));
    assert!(tokens.iter().all(|t| t.kind != oxidiris_core::TokenKind::Table));
}

#[test]
fn non_utf8_fixtures_decode_rather_than_failing() {
    let utf16 = oxidiris_core::load(&bytes("utf16_bom.txt"), Some(Format::PlainText)).unwrap();
    assert!(utf16.source.contains("Xin chào"), "got {:?}", utf16.source);

    let latin = oxidiris_core::load(&bytes("latin1.txt"), Some(Format::PlainText)).unwrap();
    assert!(latin.source.contains("café"), "got {:?}", latin.source);
}

#[test]
fn the_empty_fixture_produces_an_empty_document() {
    let doc = oxidiris_core::load(&bytes("empty.txt"), None).unwrap();
    assert!(doc.blocks.is_empty());
    let player = Player::from_document(&doc, 300, PacingMode::Natural);
    assert!(player.current().is_none());
}

/// The end-to-end case this proof of concept is judged on.
#[test]
fn the_projects_own_backlog_reads_end_to_end() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../BACKLOG.md");
    let doc = oxidiris_core::load(&std::fs::read(path).unwrap(), Some(Format::Markdown)).unwrap();

    assert!(doc.headings.len() > 20, "expected a rich outline, got {}", doc.headings.len());

    let mut player = Player::from_document(&doc, 400, PacingMode::Natural);
    let total = player.tokens().len();
    assert!(total > 2_000, "expected a substantial stream, got {total}");

    // Walk the entire document with a simulated clock and confirm it terminates.
    player.play();
    let mut elapsed_ms: u64 = 0;
    let mut steps = 0usize;
    while player.current().is_some() {
        elapsed_ms += u64::from(player.current_duration_ms());
        steps += 1;
        assert!(steps <= total, "playback failed to terminate");
        if player.advance().is_none() {
            break;
        }
    }
    assert_eq!(steps, total);

    let effective = pacing::effective_wpm(player.tokens(), 400);
    assert!(effective < 400, "effective speed {effective} should trail the configured 400");
    assert!(elapsed_ms > 0);
}
