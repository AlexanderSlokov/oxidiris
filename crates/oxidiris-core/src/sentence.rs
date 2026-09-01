//! Sentence-boundary detection for pacing.
//!
//! Implements OXD-014. See spec §3.2.2.
//!
//! A period is not always a full stop. In technical and academic prose, `Fig. 3`, `et al.`,
//! `i.e.`, `v1.2.3` and `Dr.` would each earn a 2.25x pause under a naive rule, turning a smooth
//! reading rhythm into a stutter.

/// Abbreviations that end in a period without ending a sentence.
///
/// Matched case-insensitively against the word with surrounding punctuation trimmed.
const ABBREVIATIONS: &[&str] = &[
    "al.", "approx.", "ca.", "cf.", "dr.", "e.g.", "ed.", "eds.", "eq.", "esp.", "est.", "etc.",
    "et.", "fig.", "figs.", "i.e.", "inc.", "jr.", "ltd.", "mr.", "mrs.", "ms.", "no.", "nos.",
    "op.", "p.", "pp.", "prof.", "resp.", "sec.", "sr.", "st.", "vol.", "vs.", "viz.",
];

/// Closing characters that may trail a sentence-ending mark.
const CLOSERS: &[char] = &[')', ']', '}', '"', '\'', '\u{201D}', '\u{2019}', '\u{00BB}'];

/// Strength of the pause a word earns from its trailing punctuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Break {
    /// No punctuation break.
    None,
    /// Comma, semicolon or colon.
    Clause,
    /// Full stop, question mark or exclamation mark.
    Sentence,
}

impl Break {
    /// Multiplier applied to the base word duration (spec §3.2.1).
    pub const fn factor(self) -> f32 {
        match self {
            Break::None => 1.0,
            Break::Clause => 1.5,
            Break::Sentence => 2.25,
        }
    }
}

/// Classify the punctuation break after `word`, given the word that follows it.
///
/// `next` is required because the decisive signal for an ambiguous period is what comes after it:
/// a lowercase letter or a digit means the sentence is still running.
pub fn classify(word: &str, next: Option<&str>) -> Break {
    let trimmed = word.trim_end_matches(CLOSERS);
    let Some(last) = trimmed.chars().last() else {
        return Break::None;
    };

    match last {
        '!' | '?' | '\u{2026}' => Break::Sentence,
        ',' | ';' | ':' => Break::Clause,
        '.' => {
            if is_abbreviation(trimmed) || is_numeric_period(trimmed) || continues_after(next) {
                Break::None
            } else {
                Break::Sentence
            }
        }
        _ => Break::None,
    }
}

/// Whether `word` is a known abbreviation or a single-letter initial such as `K.`.
fn is_abbreviation(word: &str) -> bool {
    let lower = word.trim_start_matches(['(', '[', '{', '"', '\'']).to_lowercase();
    if ABBREVIATIONS.contains(&lower.as_str()) {
        return true;
    }
    // Single-letter initials: "K.", "J." in author lists.
    let mut chars = lower.chars();
    matches!((chars.next(), chars.next(), chars.next()), (Some(c), Some('.'), None) if c.is_alphabetic())
}

/// Whether the period belongs to a number or version string rather than to a sentence.
///
/// Catches `3.` inside enumerations and `v1.2.3.` at the end of a clause.
fn is_numeric_period(word: &str) -> bool {
    let body = word.trim_end_matches('.');
    !body.is_empty() && body.chars().any(|c| c.is_ascii_digit()) && body.contains('.')
}

/// Whether the following word signals that the sentence has not ended.
fn continues_after(next: Option<&str>) -> bool {
    let Some(next) = next else {
        // End of block. Treat as a genuine stop.
        return false;
    };
    let Some(first) = next.chars().find(|c| c.is_alphanumeric()) else {
        return false;
    };
    first.is_numeric() || first.is_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_full_stop_ends_a_sentence() {
        assert_eq!(classify("thúc.", Some("Câu")), Break::Sentence);
        assert_eq!(classify("done.", Some("Next")), Break::Sentence);
    }

    #[test]
    fn question_and_exclamation_end_a_sentence() {
        assert_eq!(classify("really?", Some("yes")), Break::Sentence);
        assert_eq!(classify("wow!", Some("that")), Break::Sentence);
    }

    #[test]
    fn commas_and_colons_are_clause_breaks() {
        assert_eq!(classify("first,", Some("second")), Break::Clause);
        assert_eq!(classify("note:", Some("this")), Break::Clause);
    }

    /// The acceptance case for OXD-014: none of these may produce a sentence pause.
    #[test]
    fn technical_abbreviations_do_not_end_sentences() {
        let cases = [
            ("Fig.", Some("3")),
            ("et", Some("al.")),
            ("al.", Some("showed")),
            ("i.e.", Some("the")),
            ("e.g.", Some("Rust")),
            ("vs.", Some("the")),
            ("No.", Some("5")),
            ("Dr.", Some("Rayner")),
            ("v1.2.3", Some("release")),
            ("std::fmt", Some("module")),
            ("3.14", Some("approx")),
        ];
        for (word, next) in cases {
            assert_eq!(classify(word, next), Break::None, "word {word:?} must not end a sentence");
        }
    }

    #[test]
    fn lowercase_follower_suppresses_the_break() {
        assert_eq!(classify("Approx.", Some("twenty")), Break::None);
    }

    #[test]
    fn closing_quote_after_a_stop_still_counts() {
        assert_eq!(classify("\"done.\"", Some("Then")), Break::Sentence);
        assert_eq!(classify("(done.)", Some("Then")), Break::Sentence);
    }

    #[test]
    fn empty_and_punctuation_only_words_are_safe() {
        assert_eq!(classify("", None), Break::None);
        assert_eq!(classify(")", None), Break::None);
    }
}