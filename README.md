# oxidiris

A Rust TUI that leverages RSVP to stream docs to your focus point — zero eye movement required.

```
┌ BACKLOG.md ──────────────────────────┐┌ Text  ·  [Tab] to scroll ────────────┐
│                                      ││## How to Use This Document           │
│                                      ││                                      │
│                                      ││### Task Pick Rules                   │
│                                      ││                                      │
│                                      ││1. Only pick tasks where all          │
│                                      ││`Depends` are in the Done state. Do   │
│                                      ││not do tasks ahead of their           │
│                   ▼                  ││dependencies without permission.      │
│                  ahead               ││2. Tasks labeled BLOCKED-BY-HUMAN     │
│                   ▲                  ││require human decisions (see [Section │
│                                      ││D](#d-decisions-requiring-human-input)│
│                                      ││) — do not decide independently.      │
│                                      ││3. Tasks labeled Parallel-safe can be │
│                                      ││worked on concurrently with other     │
│                                      ││tasks in the same phase without file  │
│                                      ││conflicts.                            │
│                                      ││4. Update task status in the overview │
│                                      ││table when starting (`In progress`)   │
└──────────────────────────────────────┘└──────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────────────────────┐
│speed: 400 WPM (eff. 297)                                   word: 83/6047 (1%)│
│[Space] play   [K / Up] speed   [H / Left] seek   [?] help   [q] quit  ░░░░░░░│
└──────────────────────────────────────────────────────────────────────────────┘
```

The highlighted character is the **Optimal Recognition Point**, and it is drawn at the same
terminal column for every word in the document. That is the whole idea: your eye fixates once and
the text moves instead.

The panel on the right is the other half of the idea. RSVP takes the page away, and with it your
sense of where you are; the panel puts it back, scrolling itself and highlighting the word the
reader frame is showing. It appears whenever the window is at least 80 columns wide.

## Status

**v0.2.0** — reads `.md` and `.txt`, with the split view, outline sidebar and Review Mode. See
[`CHANGELOG.md`](CHANGELOG.md) for what this release contains and [`BACKLOG.md`](BACKLOG.md) for
what comes next.

**Unreleased** — PDFs read too, including two-column conference papers. Ligatures, hyphenated
line breaks and stray page numbers are cleaned up on the way in; figure labels are not, yet. How
that was decided, and measured, is in [ADR 003](docs/decisions/pdf-extraction.md).

## What it is not

RSVP removes *regressions*, the small backward eye movements that make up 10-15% of natural reading
and are how the brain repairs a misparse. Removing them buys speed and costs comprehension,
especially on dense material.

So Oxidiris is a **skim and triage** tool: work out whether a paper is worth your afternoon, get
through a changelog, re-read something you already know. It is not a replacement for reading a hard
paper carefully, and it does not advertise a WPM number as an achievement.

## Install

### Prebuilt binary

No Rust toolchain required. Every release carries a binary for the platforms below; the Linux
builds are statically linked against musl, so they run on any distribution regardless of its glibc
version. Each block is complete — paste it, and you have `oxidiris` on your `PATH`.

**Linux, x86_64**

```sh
curl -fsSL https://github.com/AlexanderSlokov/oxidiris/releases/latest/download/oxidiris-x86_64-unknown-linux-musl.tar.gz | tar -xz
sudo install -m755 oxidiris-x86_64-unknown-linux-musl/oxidiris /usr/local/bin/oxidiris
rm -rf oxidiris-x86_64-unknown-linux-musl
```

**Linux, aarch64**

```sh
curl -fsSL https://github.com/AlexanderSlokov/oxidiris/releases/latest/download/oxidiris-aarch64-unknown-linux-musl.tar.gz | tar -xz
sudo install -m755 oxidiris-aarch64-unknown-linux-musl/oxidiris /usr/local/bin/oxidiris
rm -rf oxidiris-aarch64-unknown-linux-musl
```

**macOS, Apple Silicon**

```sh
curl -fsSL https://github.com/AlexanderSlokov/oxidiris/releases/latest/download/oxidiris-aarch64-apple-darwin.tar.gz | tar -xz
sudo install -m755 oxidiris-aarch64-apple-darwin/oxidiris /usr/local/bin/oxidiris
rm -rf oxidiris-aarch64-apple-darwin
```

**macOS, Intel**

```sh
curl -fsSL https://github.com/AlexanderSlokov/oxidiris/releases/latest/download/oxidiris-x86_64-apple-darwin.tar.gz | tar -xz
sudo install -m755 oxidiris-x86_64-apple-darwin/oxidiris /usr/local/bin/oxidiris
rm -rf oxidiris-x86_64-apple-darwin
```

The macOS binaries are unsigned. Downloading with `curl` as above is fine, because Gatekeeper only
quarantines what a browser or Finder wrote. If you fetched the archive in a browser instead, clear
the flag once: `xattr -d com.apple.quarantine /usr/local/bin/oxidiris`.

**Windows, x86_64** (PowerShell)

```powershell
$dst = "$env:LOCALAPPDATA\Programs\oxidiris"
Invoke-WebRequest https://github.com/AlexanderSlokov/oxidiris/releases/latest/download/oxidiris-x86_64-pc-windows-msvc.zip -OutFile oxidiris.zip
Expand-Archive oxidiris.zip -DestinationPath $dst -Force
Move-Item "$dst\oxidiris-x86_64-pc-windows-msvc\oxidiris.exe" $dst -Force
Remove-Item oxidiris.zip, "$dst\oxidiris-x86_64-pc-windows-msvc" -Recurse
[Environment]::SetEnvironmentVariable("Path", "$([Environment]::GetEnvironmentVariable('Path','User'));$dst", "User")
```

Open a new terminal afterward so the `PATH` change takes effect.

**Check it worked**, on any platform:

```sh
oxidiris --version
```

A `SHA256SUMS` file is attached to every release if you want to verify the download.

### From source

Requires Rust 1.88 or newer.

```sh
cargo install --git https://github.com/AlexanderSlokov/oxidiris oxidiris --locked
```

From a clone of this repository:

```sh
cargo install --path crates/oxidiris --locked
```

A Homebrew tap and a crates.io publish are tracked as `OXD-077` in [`BACKLOG.md`](BACKLOG.md).

## Use

```sh
oxidiris README.md                  # read at the default 300 WPM
oxidiris paper.md -w 450            # start faster
oxidiris paper.md -m focus          # hide the text panel, reader frame only
oxidiris notes.txt --pacing linear  # even timing, no punctuation pauses
oxidiris borg.pdf                   # PDFs too, two columns included
oxidiris BACKLOG.md --dump          # clean plain text on stdout
oxidiris BACKLOG.md | less          # piping produces text, not a broken TUI
```

### Keys

The keymap is **modal**: `Tab` moves focus to the text panel and `o` to the outline, and while a
panel has focus `J`/`K` scroll it instead of changing speed. `Esc` always returns to reading; only
`q` leaves the program. `?` lists the keys that work where you currently are.

While reading:

| Key               | Action                    |
|-------------------|---------------------------|
| `Space`           | Play / pause              |
| `K` `↑` / `J` `↓` | Faster / slower (25 WPM)  |
| `+` / `-`         | Faster / slower (5 WPM)   |
| `H` `←` / `L` `→` | Back / forward 5 words    |
| `[` / `]`         | Previous / next paragraph |
| `g` / `G`         | Start / end               |
| `R`               | Restart                   |
| `Tab`             | Focus the text panel      |
| `o`               | Outline sidebar           |
| `v`               | Review the last paragraph |
| `?`               | Key reference             |
| `q`               | Quit                      |

In the text panel or the outline:

| Key               | Action                        |
|-------------------|-------------------------------|
| `K` `↑` / `J` `↓` | Scroll / move the selection   |
| `Enter`           | Jump to the selected heading  |
| `Tab` / `Esc`     | Back to reading               |

**Review Mode** (`v`) pauses and shows the paragraph you just read, verbatim, markup included.
RSVP works by removing the backward glance that repairs a misparse; this is that glance, on
purpose. `Esc` closes it and picks up exactly where you were.

## Accessibility

Oxidiris is assistive technology, so these are requirements rather than nice-to-haves:

- **The anchor never relies on color alone.** It is bold, underlined, and framed by `▼ ▲`, so it
  stays identifiable for color-blind readers and on monochrome terminals (WCAG SC 1.4.1).
- **The default speed is 300 WPM,** not a marketing number. Rapidly changing text is a
  photosensitivity risk, and the reader warns once past 700 WPM (WCAG SC 2.3.1).
- **`NO_COLOR` is honored,** and color degrades from truecolor through 256 to 16 to none.
- **Everything is keyboard-driven.** There is no mouse-only action.
- **The panel highlight is reversed video, not a colour.** Like the anchor, it survives a
  monochrome terminal; a snapshot test asserts that no colour at all is emitted under `NO_COLOR`.
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
that departed from it are recorded in [`docs/decisions/`](docs/decisions) and back-annotated into
the spec.

## Licence

GPL-3.0-or-later. See [DEC-01](BACKLOG.md#d-decisions-requiring-human-input) for an open question
about dual-licensing the engine crate.
