# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the major version is
`0`, the public API of `oxidiris-core` may change in any minor release.

## [Unreleased]

Phase 3 is not finished: `OXD-033` (`<n>%` jump) and `OXD-035` (ramp-up on resume) remain. Both are
small; see [`BACKLOG.md`](BACKLOG.md).

## [0.2.0] - 2026-09-02

Phase 3 lands the context features (`OXD-030`…`OXD-036`). They are not extras: RSVP works by
removing the reader's sense of place, and this is the half of the design that gives it back. See
[`BACKLOG.md`](BACKLOG.md).

### Added

- **Full-text panel.** The document beside the reader frame, scrolling itself to follow the
  cursor, with the word currently on screen highlighted. The highlight is driven by
  `Token::byte_span`, so it cannot drift out of step with what is being read. Appears whenever the
  window is at least 80 columns wide, and is the default layout now that it exists — `--mode
  focus` still gives the bare reader frame. `OXD-031`.
- **Outline sidebar** (`o`). The heading tree from the parser, with the section you are in marked
  and `Enter` to jump to any of it. On windows too narrow for a third column it is drawn over the
  reader instead. `OXD-032`.
- **Review Mode** (`v`). Pauses and shows the paragraph just read, verbatim and markup included,
  reconstructed from `Block::byte_span`. `Esc` closes it and resumes at the same word. This is the
  controlled version of the backward glance RSVP removes. `OXD-034`.
- **Modal keymap.** `Tab` moves focus to the text panel, `o` to the outline; while a panel has
  focus `J`/`K` scroll it rather than changing speed. The key reference lists the bindings that
  work where you currently are, and both the popup and the dispatcher still read one table.
  `OXD-030`, resolving the conflict spec §7.1 flagged.
- `Player::seek_block_id`, so a caller holding a `Heading` can jump to it without knowing token
  indices.
- Movement keys scroll an open popup instead of changing a speed the reader cannot see; the key
  reference has been scrollable in principle since v0.1.0 but had no key bound to it.
- **Golden-frame tests** (`OXD-036`): whole-screen snapshots at 80x24, 200x50 and 40x10, in every
  mode, plus a style grid that asserts the ORP stays bold *and* underlined and the panel highlight
  stays reversed with no colour emitted at all under `NO_COLOR`.

### Changed

- `--mode` now defaults to `tui`. It degrades to `focus` on its own below 80 columns, so the
  default is safe on any terminal, and `--mode tui` no longer reports itself as unimplemented.
- The status block spans the full window width rather than sitting inside the left column as spec
  §5 drew it, and panel visibility is decided on width alone rather than on a size class that also
  weighed height. Both changes are recorded in
  [`docs/decisions/split-view-layout.md`](docs/decisions/split-view-layout.md) (ADR 002) and
  back-annotated into the spec.

- **Minimum supported Rust version raised from 1.85 to 1.88.** Edition 2024 only needs 1.85, but
  `ratatui` 0.30 and its dependency tree require 1.88, so the workspace never actually built on
  the version v0.1.0 declared. Raising it changes the compatibility promise, which is why it
  lands in a minor release rather than a patch. `OXD-005`, issue
  [#4](https://github.com/AlexanderSlokov/oxidiris/issues/4).

### Fixed

- `cargo-deny` no longer rejects the project's own crates. The licence allow-list is meant to keep
  *dependencies* permissive, but cargo-deny evaluates workspace members too, so `oxidiris` and
  `oxidiris-core` failed their own GPL-3.0-or-later policy. They are now explicit exceptions.
- `make ci` gained the MSRV build and the licence audit, and a new `make msrv` target builds on
  the declared minimum, reading the version out of the manifest so the two cannot drift. The
  Makefile no longer claims that a green `make check` implies a green pipeline: it runs on
  whatever toolchain is active and never invoked `cargo deny`, which is exactly how both defects
  above reached a tagged release.

## [0.1.0] - 2026-09-01

First release. A complete vertical slice: bytes on disk to an anchored word on screen. Covers
Phases 0-2 of the backlog (`OXD-001`…`OXD-027`).

### Added

**Engine (`oxidiris-core`)**

- Unicode text pipeline: NFC normalization, control and bidi character stripping, grapheme
  cluster segmentation, and display-width measurement (UAX #15, #29, #11). Zero-width joiners are
  preserved so emoji sequences stay a single grapheme.
- Optimal Recognition Point algorithm. The anchor index comes from a table encoded as data, and
  the resulting offset is measured in display columns rather than characters, so double-width
  and combining text land on the same column as ASCII.
- Pacing engine. Each token carries a WPM-independent `weight` and a structural `pause_ms`, so a
  speed change never requires re-tokenizing. Word length, punctuation, and block kind all feed the
  weight. Two modes: `natural` and `linear`.
- Speed range 50-1500 WPM, default 300. The default is deliberately conservative because rapidly
  changing text is a photosensitivity risk (WCAG SC 2.3.1); the reader warns past 700 WPM.
- Sentence boundary heuristic with an abbreviation list, so `Fig. 3`, `et al.`, `v1.2.3` and
  `std::fmt` do not trigger a full sentence pause.
- Encoding detection with decoding fallback, so a non-UTF-8 file opens instead of failing.
- Plain text parser: blank-line paragraphs, hard-wrap rejoining.
- Markdown parser: heading outline, list items, code blocks, block quotes. Tables are parsed but
  kept out of the reading stream. URLs and autolinks are dropped while link labels survive.
- Player state machine with no wall-clock dependency, which is what makes it testable against
  simulated time and reusable in a browser build. Seek by word, by paragraph, and by ratio.
- The crate has no terminal dependency and builds for `wasm32-unknown-unknown`.

**Terminal application (`oxidiris`)**

- RSVP widget that holds the anchor at a fixed column for every word in the document, verified
  against a rendered buffer at widths 40, 80, 120 and 200.
- Deadline scheduler that anchors each deadline to the previous *deadline* rather than to the
  current instant, so timing does not accumulate drift, and resynchronizes rather than replaying
  a backlog after the machine suspends.
- Status bar showing configured WPM alongside the effective WPM, plus word position and progress.
- Help popup (`?`) generated from the binding table, so it cannot promise a key the dispatcher
  does not honour.
- Keys: play/pause, speed up and down (25 and 5 WPM steps), seek by word and by paragraph, jump to
  start and end, restart, help, quit.
- Terminal capability detection with colour degradation from truecolor to 256 to 16 to none, and
  `NO_COLOR` support.
- `--dump` for clean plain text on stdout. This is the screen-reader route, since a self-rewriting
  frame and a screen reader cannot cooperate. Piping the output produces text rather than a
  broken TUI.
- CLI flags `--wpm`, `--pacing`, `--dump`. Flags reserved for later phases (`--theme`, `--chunk`,
  `--start`, `--no-resume`, `--config`) parse and report themselves as unimplemented rather than
  failing silently.

**Repository**

- Cargo workspace, edition 2024, MSRV declared as 1.85.
- 175 tests: engine unit tests, terminal rendering tests against `TestBackend`, a fixture corpus,
  and property tests asserting the ORP invariants hold for arbitrary Unicode input.
- CI with six jobs, including a Windows target and a `wasm32-unknown-unknown` build that enforces
  the architectural rule that `oxidiris-core` never depends on a terminal crate.
- `cargo-deny` policy requiring permissive licences on dependencies.
- `Makefile` with a self-documenting target list; `make check` is the Definition of Done gate.
- `BACKLOG.md`: 56 tasks across 9 phases, each with machine-verifiable acceptance criteria.
- Design spec in `docs/informations/proposals.md` and decision records in `docs/decisions/`.

### Known limitations

- CJK text still splits on whitespace only. This is blocked on decision **DEC-02** and is tracked
  rather than papered over.
- Split view, outline sidebar and search arrive in Phases 3-4 (`OXD-031`, `OXD-032`, `OXD-043`).
- Reading from stdin (`-`) falls back to `--dump`, because the TUI cannot read keys while stdin is
  a pipe. Proper support needs `OXD-047`.
- RSVP removes the backward eye movements that repair a misparse, which costs comprehension on
  dense material. The tool is built for skim and triage, not for careful reading.
- **The declared MSRV of 1.85 is wrong.** The dependency tree requires 1.88, so this release does
  not build on the version it advertises. Use 1.88 or newer. Corrected in the next release; see
  issue [#4](https://github.com/AlexanderSlokov/oxidiris/issues/4).

[Unreleased]: https://github.com/AlexanderSlokov/oxidiris/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/AlexanderSlokov/oxidiris/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/AlexanderSlokov/oxidiris/releases/tag/v0.1.0