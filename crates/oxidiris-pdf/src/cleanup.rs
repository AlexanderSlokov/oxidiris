//! Turning raw extracted text into something worth reading.
//!
//! A PDF stores glyphs at coordinates, not sentences. What comes back out therefore carries three
//! artefacts that would each be visible in the RSVP frame, one word at a time, at 400 WPM:
//!
//! 1. **Typographic ligatures.** `efﬁcient` is stored as `e`, `f`, U+FB01, `c`… The frame would
//!    show a glyph most terminal fonts cannot draw, and a search for "efficient" would miss it.
//! 2. **Hyphenated line breaks.** Justified two-column text breaks `hun-` / `dreds` across lines.
//!    The plain-text parser rejoins wrapped lines with a space, which would yield `hun- dreds` —
//!    two nonsense display units instead of one word.
//! 3. **Stray numbers from page furniture and charts.** A bare `7` standing between two
//!    paragraphs becomes a paragraph of its own, so the reader gets a full pause on something
//!    that is not part of the text. Measured on five real papers, most of these turn out to be
//!    axis tick labels lifted out of figures rather than running page numbers; the rule catches
//!    both, because to a reader they are the same interruption.
//!
//! Every step here is a pure `&str -> String` function, so the whole pipeline is testable without
//! a PDF anywhere in sight.

/// Ligatures that must be expanded before the text reaches a terminal.
///
/// Deliberately a short explicit table rather than NFKC normalisation: NFKC would also rewrite
/// superscripts, fractions, Roman numeral characters and CJK compatibility forms, all of which
/// carry meaning in a technical paper and must survive untouched.
const LIGATURES: [(char, &str); 7] = [
    ('\u{FB00}', "ff"),
    ('\u{FB01}', "fi"),
    ('\u{FB02}', "fl"),
    ('\u{FB03}', "ffi"),
    ('\u{FB04}', "ffl"),
    ('\u{FB05}', "st"), // long s + t
    ('\u{FB06}', "st"),
];

/// Hyphen characters that a typesetter may use to break a word across two lines.
const BREAKING_HYPHENS: [char; 2] = ['-', '\u{2010}'];

/// Longest numeric line still treated as page furniture rather than content.
const STRAY_NUMBER_MAX_LEN: usize = 4;

/// Clean raw extracted text into readable prose.
///
/// ```
/// let raw = "hun-\ndreds of e\u{FB03}cient jobs\n\n7\n\nNext paragraph.\n";
/// let text = oxidiris_pdf::tidy(raw);
/// assert_eq!(text, "hundreds of efficient jobs\n\nNext paragraph.\n");
/// ```
pub fn tidy(raw: &str) -> String {
    let folded = expand_ligatures(raw);
    let welded = weld_hyphenated_breaks(&folded);
    let stripped = drop_isolated_numbers(&welded);
    collapse_blank_runs(&stripped)
}

/// Replace typographic ligature code points with the letters they stand for.
fn expand_ligatures(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match LIGATURES.iter().find(|(ligature, _)| *ligature == c) {
            Some((_, letters)) => out.push_str(letters),
            None => out.push(c),
        }
    }
    out
}

/// Join a line ending in a hyphen onto the next one, dropping the hyphen.
///
/// Every line is trimmed on both sides on the way through: a PDF's leading whitespace is an
/// artefact of glyph positions, never indentation the author typed.
fn weld_hyphenated_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let current = line.trim();
        let next = lines.peek().map(|l| l.trim());
        if let Some(stem) = word_split_across_lines(current, next) {
            out.push_str(stem);
            continue;
        }
        out.push_str(current);
        out.push('\n');
    }
    out
}

/// The line without its trailing hyphen, when that hyphen breaks a word across two lines.
///
/// Requires a letter on both sides of the break, so a dash used as punctuation (`text - `) and a
/// list bullet are left alone. A compound that genuinely contains a hyphen and happens to break at
/// it (`Google-` / `wide`) is welded into one word; distinguishing those needs a dictionary, and
/// every extractor in the field makes the same trade.
fn word_split_across_lines<'a>(current: &'a str, next: Option<&str>) -> Option<&'a str> {
    let next = next?;
    if !next.chars().next()?.is_lowercase() {
        return None;
    }
    let hyphen = *BREAKING_HYPHENS.iter().find(|h| current.ends_with(**h))?;
    let stem = &current[..current.len() - hyphen.len_utf8()];
    stem.chars().next_back()?.is_alphanumeric().then_some(stem)
}

/// Remove short numeric lines that stand alone between blank lines.
///
/// Isolation is what makes this safe: a `7` or an `I` inside a paragraph keeps its neighbours and
/// is never touched, so only a number that already reads as its own paragraph can match.
fn drop_isolated_numbers(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::with_capacity(text.len());
    for (index, line) in lines.iter().enumerate() {
        let alone_above = index == 0 || lines[index - 1].trim().is_empty();
        let alone_below = index + 1 >= lines.len() || lines[index + 1].trim().is_empty();
        if alone_above && alone_below && is_stray_number(line) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Whether a line is nothing but a short number, in arabic or roman form.
fn is_stray_number(line: &str) -> bool {
    let text = line.trim();
    if text.is_empty() || text.len() > STRAY_NUMBER_MAX_LEN {
        return false;
    }
    text.chars().all(|c| c.is_ascii_digit())
        || text.chars().all(|c| matches!(c, 'i' | 'v' | 'x' | 'l' | 'I' | 'V' | 'X' | 'L'))
}

/// Reduce runs of blank lines to a single one and guarantee a trailing newline.
///
/// The plain-text parser reads one blank line as a paragraph break; more than one adds nothing,
/// and removing a stray number leaves a doubled gap behind every time.
fn collapse_blank_runs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_pending = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            blank_pending = !out.is_empty();
            continue;
        }
        if blank_pending {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
        blank_pending = false;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligatures_become_the_letters_they_stand_for() {
        assert_eq!(expand_ligatures("e\u{FB03}cient"), "efficient");
        assert_eq!(expand_ligatures("bene\u{FB01}ts and \u{FB02}ags"), "benefits and flags");
        assert_eq!(expand_ligatures("o\u{FB00}er"), "offer");
    }

    /// NFKC would fold these too; the explicit table must not.
    #[test]
    fn superscripts_and_fractions_survive_untouched() {
        assert_eq!(expand_ligatures("x\u{00B2} and \u{00BD}"), "x\u{00B2} and \u{00BD}");
    }

    #[test]
    fn a_word_broken_across_lines_is_welded_back_together() {
        assert_eq!(weld_hyphenated_breaks("hun-\ndreds\n"), "hundreds\n");
        assert_eq!(weld_hyphenated_breaks("e\u{2010}\nmail\n"), "email\n");
    }

    #[test]
    fn a_hyphen_before_a_capital_stays_a_hyphen() {
        assert_eq!(weld_hyphenated_breaks("Sub-\nSaharan\n"), "Sub-\nSaharan\n");
    }

    #[test]
    fn a_dash_used_as_punctuation_is_not_a_word_break() {
        assert_eq!(weld_hyphenated_breaks("the answer -\nmaybe\n"), "the answer -\nmaybe\n");
    }

    #[test]
    fn a_trailing_hyphen_on_the_last_line_is_kept() {
        assert_eq!(weld_hyphenated_breaks("dangling-\n"), "dangling-\n");
    }

    #[test]
    fn an_isolated_number_is_dropped() {
        assert_eq!(drop_isolated_numbers("End.\n\n7\n\nStart.\n"), "End.\n\n\nStart.\n");
        assert_eq!(drop_isolated_numbers("End.\n\nxiv\n\nStart.\n"), "End.\n\n\nStart.\n");
    }

    /// The isolation rule is the whole safety net: a number inside prose must survive.
    #[test]
    fn a_number_inside_a_paragraph_is_kept() {
        let text = "we ran\n7\nmachines\n";
        assert_eq!(drop_isolated_numbers(text), text);
    }

    #[test]
    fn a_long_number_is_content_not_furniture() {
        let text = "End.\n\n20155\n\nStart.\n";
        assert_eq!(drop_isolated_numbers(text), text);
    }

    #[test]
    fn blank_runs_collapse_to_one_paragraph_break() {
        assert_eq!(collapse_blank_runs("a\n\n\n\nb\n"), "a\n\nb\n");
        assert_eq!(collapse_blank_runs("\n\n\na\n"), "a\n");
        assert_eq!(collapse_blank_runs(""), "");
    }

    /// A hyphen the author typed must survive; only the one the typesetter added disappears.
    ///
    /// Both appear in this excerpt from the Borg paper: `task-packing` sits mid-line and is real,
    /// `con-` / `trol` spans a line break and is not.
    #[test]
    fn the_whole_pipeline_produces_readable_paragraphs() {
        let raw = "It achieves high utilization by combining admission con-\n\
                   trol, ef\u{FB01}cient task-packing, over-commitment, and machine\n\
                   sharing with isolation.\n\
                   \n\
                   \n\
                   12\n\
                   \n\
                   The next section explains how.\n";
        let text = tidy(raw);
        assert!(text.contains("admission control,"), "got {text:?}");
        assert!(text.contains("efficient task-packing,"), "got {text:?}");
        assert!(!text.contains("\n12\n"), "page number survived: {text:?}");
        assert_eq!(text.matches("\n\n").count(), 1, "one paragraph break expected: {text:?}");
    }

    /// Known limitation, recorded so a future change to the heuristic is a deliberate one: a real
    /// compound that happens to break at its own hyphen comes back welded shut.
    #[test]
    fn a_compound_broken_at_its_own_hyphen_loses_it() {
        assert_eq!(weld_hyphenated_breaks("over-\ncommitment\n"), "overcommitment\n");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(tidy(""), "");
    }
}
