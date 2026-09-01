# oxidiris

A Rust TUI that leverages RSVP to stream docs to your focus point — zero eye movement required.

```
┌ BACKLOG.md ──────────────────────────────────────────────────────────────────┐
│                                                                              │
│                                       ▼                                      │
│                                      Rules                                   │
│                                       ▲                                      │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────────────────────┐
│speed: 400 WPM (eff. 295)                                   word: 67/5494 (1%)│
│[Space] play   [K / Up] speed   [H / Left] seek   [?] help   [q] quit  ░░░░░░░│
└──────────────────────────────────────────────────────────────────────────────┘
```

The highlighted character is the **Optimal Recognition Point**, and it is drawn at the same
terminal column for every word in the document. That is the whole idea: your eye fixates once and
the text moves instead.

## Status

**v0.1** — reads `.md` and `.txt`. See [`BACKLOG.md`](BACKLOG.md) for what is built and what is next.

## What it is not

RSVP removes *regressions*, the small backward eye movements that make up 10-15% of natural reading
and are how the brain repairs a misparse. Removing them buys speed and costs comprehension,
especially on dense material.

So Oxidiris is a **skim and triage** tool: work out whether a paper is worth your afternoon, get
through a changelog, re-read something you already know. It is not a replacement for reading a hard
paper carefully, and it does not advertise a WPM number as an achievement.

## Install

```sh
cargo install --path crates/oxidiris
```

Requires Rust 1.85 or newer.

## Use

```sh
oxidiris README.md                  # read at the default 300 WPM
oxidiris paper.md -w 450            # start faster
oxidiris notes.txt --pacing linear  # even timing, no punctuation pauses
oxidiris BACKLOG.md --dump          # clean plain text on stdout
oxidiris BACKLOG.md | less          # piping produces text, not a broken TUI
```

### Keys

| Key               | Action                    |
|-------------------|---------------------------|
| `Space`           | Play / pause              |
| `K` `↑` / `J` `↓` | Faster / slower (25 WPM)  |
| `+` / `-`         | Faster / slower (5 WPM)   |
| `H` `←` / `L` `→` | Back / forward 5 words    |
| `[` / `]`         | Previous / next paragraph |
| `g` / `G`         | Start / end               |
| `R`               | Restart                   |
| `?`               | Key reference             |
| `q`               | Quit                      |

## Accessibility

Oxidiris is assistive technology, so these are requirements rather than nice-to-haves:

- **The anchor never relies on color alone.** It is bold, underlined, and framed by `▼ ▲`, so it
  stays identifiable for color-blind readers and on monochrome terminals (WCAG SC 1.4.1).
- **The default speed is 300 WPM,** not a marketing number. Rapidly changing text is a
  photosensitivity risk, and the reader warns once past 700 WPM (WCAG SC 2.3.1).
- **`NO_COLOR` is honored,** and color degrades from truecolor through 256 to 16 to none.
- **Everything is keyboard-driven.** There is no mouse-only action.
- **`--dump` is the screen-reader route.** A self-rewriting frame and a screen reader cannot
  cooperate, so there is a plain-text path out.

## Privacy

Oxidiris reads your documents and sends nothing anywhere. No telemetry, no network access. Nothing
is written outside the file you opened.

## Development

```sh
make            # list targets
make check      # fmt, clippy, tests, wasm constraint — the gate CI runs
make frame      # print one rendered frame without opening a terminal session
make demo       # read this project's own backlog
```

The workspace is two crates:

- **`oxidiris-core`** — the engine: Unicode segmentation, ORP, pacing, parsers, playback state.
  It has no terminal dependency and builds for `wasm32-unknown-unknown`; CI enforces this so the
  engine stays reusable for the web build and editor plugins.
- **`oxidiris`** — the terminal application.

Design notes live in [`docs/informations/proposals.md`](docs/informations/proposals.md); decisions
that departed from it are in [`docs/decisions/token-timing.md`](docs/decisions/token-timing.md).

## Licence

GPL-3.0-or-later. See [DEC-01](BACKLOG.md#d-decisions-requiring-human-input) for an open question
about dual-licensing the engine crate.
