//! Byte-to-text decoding with encoding detection.
//!
//! Implements OXD-015. See spec §4.3.
//!
//! Assuming UTF-8 is wrong often enough to matter: RFCs and man pages are frequently Latin-1, and
//! Windows-authored notes arrive as UTF-16 with a BOM. A panic on those files would be a poor
//! showing for a reading tool.

use thiserror::Error;

/// Why a byte stream could not be turned into text.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    /// The input looks like a binary file rather than a document.
    #[error("input appears to be binary (found {nul_bytes} NUL bytes in the first {scanned} bytes)")]
    Binary {
        /// How many NUL bytes were seen.
        nul_bytes: usize,
        /// How many bytes were inspected.
        scanned: usize,
    },
}

/// The encoding that was detected for an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    /// Valid UTF-8 without a byte order mark.
    Utf8,
    /// UTF-8 preceded by a byte order mark.
    Utf8Bom,
    /// UTF-16 little endian, identified by its byte order mark.
    Utf16Le,
    /// UTF-16 big endian, identified by its byte order mark.
    Utf16Be,
    /// Fallback for byte streams that are not valid UTF-8.
    Windows1252,
}

impl DetectedEncoding {
    /// Human-readable name, for the status bar and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            DetectedEncoding::Utf8 => "UTF-8",
            DetectedEncoding::Utf8Bom => "UTF-8 (BOM)",
            DetectedEncoding::Utf16Le => "UTF-16LE",
            DetectedEncoding::Utf16Be => "UTF-16BE",
            DetectedEncoding::Windows1252 => "Windows-1252",
        }
    }
}

/// Number of leading bytes inspected when deciding whether input is binary.
const BINARY_SCAN_LEN: usize = 8192;

/// Decode `bytes` into a `String`, detecting the encoding.
///
/// Returns the decoded text together with the encoding that was used. Malformed sequences are
/// replaced rather than rejected, so a partially corrupt file still reads.
pub fn decode(bytes: &[u8]) -> Result<(String, DetectedEncoding), DecodeError> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        let (text, _) = encoding_rs::UTF_8.decode_with_bom_removal(rest);
        return Ok((text.into_owned(), DetectedEncoding::Utf8Bom));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(rest);
        return Ok((text.into_owned(), DetectedEncoding::Utf16Le));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let (text, _, _) = encoding_rs::UTF_16BE.decode(rest);
        return Ok((text.into_owned(), DetectedEncoding::Utf16Be));
    }

    // No BOM. Reject obvious binary before guessing at a text encoding, otherwise an executable
    // would decode into thousands of replacement characters and "read" as a document.
    let scanned = bytes.len().min(BINARY_SCAN_LEN);
    let nul_bytes = bytes[..scanned].iter().filter(|b| **b == 0).count();
    if nul_bytes > 0 {
        return Err(DecodeError::Binary { nul_bytes, scanned });
    }

    match core::str::from_utf8(bytes) {
        Ok(text) => Ok((text.to_string(), DetectedEncoding::Utf8)),
        Err(_) => {
            let (text, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            Ok((text.into_owned(), DetectedEncoding::Windows1252))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_utf8_is_detected() {
        let (text, enc) = decode("tiếng Việt".as_bytes()).unwrap();
        assert_eq!(text, "tiếng Việt");
        assert_eq!(enc, DetectedEncoding::Utf8);
    }

    #[test]
    fn utf8_bom_is_stripped() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"hello");
        let (text, enc) = decode(&bytes).unwrap();
        assert_eq!(text, "hello");
        assert_eq!(enc, DetectedEncoding::Utf8Bom);
    }

    #[test]
    fn utf16_le_with_bom_round_trips() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "hi ế".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let (text, enc) = decode(&bytes).unwrap();
        assert_eq!(text, "hi ế");
        assert_eq!(enc, DetectedEncoding::Utf16Le);
    }

    #[test]
    fn utf16_be_with_bom_round_trips() {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in "hi".encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        let (text, enc) = decode(&bytes).unwrap();
        assert_eq!(text, "hi");
        assert_eq!(enc, DetectedEncoding::Utf16Be);
    }

    #[test]
    fn latin1_falls_back_without_error() {
        // 0xE9 is "é" in Windows-1252 but invalid on its own in UTF-8.
        let (text, enc) = decode(&[b'c', b'a', b'f', 0xE9]).unwrap();
        assert_eq!(text, "café");
        assert_eq!(enc, DetectedEncoding::Windows1252);
    }

    #[test]
    fn binary_input_is_rejected_rather_than_decoded() {
        let bytes = [0x7F, b'E', b'L', b'F', 0x00, 0x01, 0x00, 0x02];
        assert!(matches!(decode(&bytes), Err(DecodeError::Binary { .. })));
    }

    #[test]
    fn empty_input_decodes_to_empty_text() {
        let (text, _) = decode(b"").unwrap();
        assert!(text.is_empty());
    }
}