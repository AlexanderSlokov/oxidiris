//! Command line interface.
//!
//! Implements OXD-020. See spec §6.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use oxidiris_core::pacing::{DEFAULT_WPM, MAX_WPM, MIN_WPM};

/// Display mode for the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    /// Split screen: RSVP frame plus the full text panel.
    Tui,
    /// Minimal: only the RSVP frame, centred.
    Focus,
}

/// Pacing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Pacing {
    /// Vary timing by word length, punctuation and structure.
    Natural,
    /// Absolutely even timing.
    Linear,
}

impl From<Pacing> for oxidiris_core::PacingMode {
    fn from(value: Pacing) -> Self {
        match value {
            Pacing::Natural => oxidiris_core::PacingMode::Natural,
            Pacing::Linear => oxidiris_core::PacingMode::Linear,
        }
    }
}

/// Colour theme. Full theme support arrives in v0.3 (OXD-040).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Theme {
    Dark,
    Light,
    Solarized,
}

/// Read documents at speed without moving your eyes.
#[derive(Debug, Parser)]
#[command(
    name = "oxidiris",
    version,
    about,
    long_about = None,
    after_help = "\
EXAMPLES:
  oxidiris README.md                 Read a file at the default 300 WPM
  oxidiris paper.md -w 450           Start at 450 words per minute
  oxidiris notes.txt --pacing linear Disable punctuation-aware pauses
  oxidiris borg.pdf                  Read a PDF, two-column papers included
  oxidiris paper.md -m focus         Hide the text panel, RSVP frame only
  oxidiris BACKLOG.md --dump         Print clean plain text and exit
  oxidiris BACKLOG.md | less         Piping also produces plain text

KEYS:
  Space play/pause   J/K speed   H/L seek   [/] paragraph   Tab panel
  o outline   v review   ? help   q quit"
)]
pub struct Cli {
    /// Document to read. Use `-` to read from standard input.
    pub file: PathBuf,

    /// Starting reading speed in words per minute.
    #[arg(short = 'w', long, default_value_t = DEFAULT_WPM, value_parser = wpm_in_range)]
    pub wpm: u16,

    /// Display mode.
    ///
    /// `tui` is the default now that the full-text panel exists (OXD-031); it degrades to `focus`
    /// on its own below 80 columns, so the default is safe on any terminal.
    #[arg(short = 'm', long, value_enum, default_value_t = Mode::Tui)]
    pub mode: Mode,

    /// Pacing strategy.
    #[arg(long, value_enum, default_value_t = Pacing::Natural)]
    pub pacing: Pacing,

    /// Colour theme.
    #[arg(long, value_enum)]
    pub theme: Option<Theme>,

    /// Print clean plain text to stdout instead of opening the reader.
    #[arg(long)]
    pub dump: bool,

    /// Words shown per step. Not implemented yet (OXD-046).
    #[arg(long, value_name = "N")]
    pub chunk: Option<u16>,

    /// Start position, e.g. `50%`. Not implemented yet (OXD-033).
    #[arg(long, value_name = "POS")]
    pub start: Option<String>,

    /// Ignore any saved position. Not implemented yet (OXD-045).
    #[arg(long)]
    pub no_resume: bool,

    /// Alternate config file. Not implemented yet (OXD-041).
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

/// Reject speeds outside the supported band with a message that names the band.
fn wpm_in_range(s: &str) -> Result<u16, String> {
    let value: u16 = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
    if (MIN_WPM..=MAX_WPM).contains(&value) {
        Ok(value)
    } else {
        Err(format!("speed must be between {MIN_WPM} and {MAX_WPM} WPM (got {value})"))
    }
}

impl Cli {
    /// Whether the input is standard input rather than a file on disk.
    pub fn reads_stdin(&self) -> bool {
        self.file.as_os_str() == "-"
    }

    /// Flags that parse but are not wired up yet, so the reader can say so instead of pretending.
    pub fn unimplemented_flags(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.theme.is_some() {
            v.push("--theme (OXD-040)");
        }
        if self.chunk.is_some() {
            v.push("--chunk (OXD-046)");
        }
        if self.start.is_some() {
            v.push("--start (OXD-033)");
        }
        if self.no_resume {
            v.push("--no-resume (OXD-045)");
        }
        if self.config.is_some() {
            v.push("--config (OXD-041)");
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn defaults_are_the_safe_ones() {
        let cli = Cli::try_parse_from(["oxidiris", "a.md"]).unwrap();
        assert_eq!(cli.wpm, DEFAULT_WPM);
        assert_eq!(cli.mode, Mode::Tui, "the split view is the default layout since v0.2");
        assert_eq!(cli.pacing, Pacing::Natural);
        assert!(!cli.dump);
    }

    #[test]
    fn wpm_outside_the_supported_band_is_rejected() {
        for bad in ["10", "5000", "abc"] {
            let err = Cli::try_parse_from(["oxidiris", "a.md", "-w", bad]).unwrap_err();
            assert!(
                err.to_string().contains("between") || err.to_string().contains("not a number"),
                "unhelpful error for {bad}: {err}"
            );
        }
    }

    #[test]
    fn wpm_inside_the_band_is_accepted() {
        let cli = Cli::try_parse_from(["oxidiris", "a.md", "--wpm", "450"]).unwrap();
        assert_eq!(cli.wpm, 450);
    }

    #[test]
    fn dash_means_standard_input() {
        assert!(Cli::try_parse_from(["oxidiris", "-"]).unwrap().reads_stdin());
        assert!(!Cli::try_parse_from(["oxidiris", "a.md"]).unwrap().reads_stdin());
    }

    #[test]
    fn unimplemented_flags_are_reported_not_silently_ignored() {
        let cli = Cli::try_parse_from(["oxidiris", "a.md", "--chunk", "3", "--no-resume"]).unwrap();
        let flags = cli.unimplemented_flags();
        assert_eq!(flags.len(), 2);
        assert!(flags.iter().any(|f| f.contains("--chunk")));
    }

    #[test]
    fn a_fully_default_invocation_reports_nothing_unimplemented() {
        let cli = Cli::try_parse_from(["oxidiris", "a.md"]).unwrap();
        assert!(cli.unimplemented_flags().is_empty());
    }

    #[test]
    fn help_mentions_a_real_example() {
        let help = Cli::command().render_help().to_string();
        assert!(help.contains("oxidiris README.md"));
    }
}
