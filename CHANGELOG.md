# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the major version is
`0`, the public API of `oxidiris-core` may change in any minor release.

## [Unreleased]

Nothing yet. Next up is Phase 3 (`OXD-030`…`OXD-037`): modal keymap, full-text panel, outline
sidebar and Review Mode. See [`BACKLOG.md`](BACKLOG.md).

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

- Cargo workspace, edition 2024, MSRV 1.85.
- 175 tests: engine unit tests, terminal rendering tests against `TestBackend`, a fixture corpus,
  and property tests asserting the ORP invariants hold for arbitrary Unicode input.
- CI with six jobs, including a Windows target and a `wasm32-unknown-unknown` build that enforces
  the architectural rule that `oxidiris-core` never depends on a terminal crate.
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

[Unreleased]: https://github.com/AlexanderSlokov/oxidiris/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/AlexanderSlokov/oxidiris/releases/tag/v0.1.0