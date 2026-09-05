//! Oxidiris: read documents at speed without moving your eyes.

#![forbid(unsafe_code)]

mod app;
mod cli;
mod dump;
mod event;
mod keymap;
mod scheduler;
#[cfg(test)]
mod snapshots;
mod term;
mod ui;

use std::io::{IsTerminal, Read};
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser;
use oxidiris_core::parser::Format;

use crate::app::App;
use crate::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("oxidiris: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let (bytes, title, format) = read_input(&cli)?;

    let doc = oxidiris_core::load(&bytes, format)
        .with_context(|| format!("could not read {title} as text"))?;

    if doc.blocks.is_empty() {
        bail!("{title} contains no readable text");
    }

    // Plain text when asked for it, and whenever stdout is not a terminal: piping into `less`
    // should produce text, not a fight over the alternate screen.
    let piped = !std::io::stdout().is_terminal();
    if cli.dump || piped || cli.reads_stdin() {
        return dump::write_plain(&mut std::io::stdout().lock(), &doc);
    }

    let message = build_notice(&cli, &doc);
    let split = cli.mode == cli::Mode::Tui;
    let mut application = App::new(doc, title, cli.wpm, cli.pacing.into())
        .with_detected_capabilities()
        .with_split(split)
        .with_message(message);

    install_panic_hook();
    let mut terminal = ratatui::init();
    let result = event::run(&mut terminal, &mut application);
    ratatui::restore();
    result
}

/// Read the document bytes plus a display title and a format hint.
fn read_input(cli: &Cli) -> Result<(Vec<u8>, String, Option<Format>)> {
    if cli.reads_stdin() {
        let mut buf = Vec::new();
        std::io::stdin().lock().read_to_end(&mut buf).context("reading standard input")?;
        if buf.is_empty() {
            bail!("standard input was empty");
        }
        return unpack_pdf(buf, "stdin".to_string(), None);
    }

    let path = &cli.file;
    let bytes = std::fs::read(path).with_context(|| format!("cannot open {}", path.display()))?;
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let format = Some(Format::from_extension(path.extension().and_then(|e| e.to_str())));
    unpack_pdf(bytes, title, format)
}

/// Replace PDF bytes with the plain text extracted from them, leaving anything else alone.
///
/// Detection is by header rather than by extension so that `cat paper.pdf | oxidiris -` works,
/// and so that a PDF saved under the wrong name still reads. Without this the core's decoder
/// would reject the file as binary, which is true but unhelpful (OXD-060).
fn unpack_pdf(
    bytes: Vec<u8>,
    title: String,
    format: Option<Format>,
) -> Result<(Vec<u8>, String, Option<Format>)> {
    if !oxidiris_pdf::looks_like_pdf(&bytes) {
        return Ok((bytes, title, format));
    }
    let text = oxidiris_pdf::extract(&bytes).with_context(|| format!("cannot read {title}"))?;
    Ok((text.into_bytes(), title, Some(Format::PlainText)))
}

/// Compose the note shown in the status bar on the first frame.
///
/// Flags that parse but do nothing are called out rather than silently ignored: a reader who
/// typed `--chunk 3` deserves to know it had no effect.
fn build_notice(cli: &Cli, doc: &oxidiris_core::Document) -> Option<String> {
    let pending = cli.unimplemented_flags();
    if !pending.is_empty() {
        return Some(format!("not implemented yet: {}", pending.join(", ")));
    }
    let headings = doc.headings.len();
    if headings > 0 {
        Some(format!("{headings} headings  ·  [Space] to start  ·  [?] for keys"))
    } else {
        Some("[Space] to start  ·  [?] for keys".to_string())
    }
}

/// Restore the terminal before a panic prints, so the backtrace is readable.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        previous(info);
    }));
}

/// Helper kept for future path-based format overrides.
#[allow(dead_code)]
fn format_of(path: &Path) -> Format {
    Format::from_extension(path.extension().and_then(|e| e.to_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_files_are_recognised_by_extension() {
        assert_eq!(format_of(Path::new("a.md")), Format::Markdown);
        assert_eq!(format_of(Path::new("a.txt")), Format::PlainText);
        assert_eq!(format_of(Path::new("README")), Format::PlainText);
    }

    #[test]
    fn the_notice_names_unimplemented_flags() {
        let cli = Cli::try_parse_from(["oxidiris", "a.md", "--chunk", "2"]).unwrap();
        let doc = oxidiris_core::parser::parse_markdown("# T\n\nbody\n");
        let notice = build_notice(&cli, &doc).unwrap();
        assert!(notice.contains("--chunk"), "got {notice:?}");
    }

    fn testdata(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(name)
    }

    /// A PDF must arrive at the engine as plain text, whatever the extension claimed (OXD-060).
    #[test]
    fn pdf_bytes_are_replaced_by_their_text_and_marked_plain() {
        let bytes = std::fs::read(testdata("pdf_typography.pdf")).unwrap();
        let (out, title, format) = unpack_pdf(bytes, "paper.pdf".into(), None).unwrap();

        assert_eq!(format, Some(Format::PlainText));
        assert_eq!(title, "paper.pdf");
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("efficient"), "got {text:?}");
    }

    #[test]
    fn non_pdf_bytes_pass_through_untouched() {
        let markdown = b"# Title\n\nbody\n".to_vec();
        let (out, _, format) =
            unpack_pdf(markdown.clone(), "a.md".into(), Some(Format::Markdown)).unwrap();

        assert_eq!(out, markdown);
        assert_eq!(format, Some(Format::Markdown), "the format hint must survive");
    }

    /// The failure has to name the file, since the reader may have been handed a directory of them.
    #[test]
    fn an_unreadable_pdf_reports_which_file_failed() {
        let err =
            unpack_pdf(b"%PDF-1.7 truncated".to_vec(), "broken.pdf".into(), None).unwrap_err();
        assert!(format!("{err:#}").contains("broken.pdf"), "got {err:#}");
    }

    #[test]
    fn the_notice_falls_back_to_a_hint() {
        let cli = Cli::try_parse_from(["oxidiris", "a.md"]).unwrap();
        let doc = oxidiris_core::parser::parse_markdown("# T\n\nbody\n");
        let notice = build_notice(&cli, &doc).unwrap();
        assert!(notice.contains("[?]"));
    }
}
