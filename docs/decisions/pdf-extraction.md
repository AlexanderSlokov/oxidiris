# ADR 003 — PDF text comes from `pdf-extract`, in content-stream order

**Status:** Accepted
**Date:** 2026-09-05
**Task:** OXD-060
**Amends:** `docs/informations/proposals.md` §8.2

## Context

Spec §8.2 warns that de-columnizing PDFs is hard enough to consume the whole project, and OXD-060
was written on the assumption that shelling out to Poppler's `pdftotext -layout` would probably be
the pragmatic answer. This is the measurement that assumption asked for.

**Corpus.** Five real two-column papers, all publicly downloadable: Borg (EuroSys'15, 18 pages),
Omega (EuroSys'13, 14 pages, 4.3 MB), MapReduce (OSDI'04), Bigtable (OSDI'06), Spanner (OSDI'12).
They are deliberately *not* committed to `testdata/` — they are under publisher copyright and a
fixture has to be redistributable. See "Fixtures" below for what was committed instead.

**Candidates.** `pdf-extract` 0.12 (pure Rust, wraps `lopdf`), `lopdf` used directly, and Poppler's
`pdftotext` in both its plain and `-layout` modes.

`lopdf` on its own was dropped before benchmarking: it hands back content streams and font
dictionaries, so using it directly means writing the glyph-to-Unicode mapping, the font encoding
tables and the text-positioning maths ourselves. That is precisely the work `pdf-extract` has
already done on top of it, and it is not work this project wants to own.

## What the measurements said

**Reading order.** There is no ground truth to diff against, so the two extractors were compared
to each other: word sequences normalised (NFKC, de-hyphenated, letters only) and matched in order
with a longest-common-subsequence ratio. Agreement between two independent implementations is
weak evidence that both are right; disagreement says where to look by hand.

| Paper | Ordered overlap |
|---|---|
| Borg | 0.953 |
| Bigtable | 0.954 |
| Spanner | 0.959 |
| MapReduce | 0.925 |
| Omega | 0.699 |

Omega is the interesting one, and it is **Poppler that is wrong there**: it lifts the figure
labels out of Figure 1 (`Monolithic`, `Two-level`, `scheduling logic`, `cluster state
information`) and drops them into the middle of the abstract. `pdf-extract` keeps the body text
running clean. The remaining 5% on the other four papers is the same class of difference —
figure and table furniture, ordered differently — not columns interleaving.

`pdftotext -layout` was rejected outright: preserving the visual layout puts the two columns
side by side on every line, which is the one output shape a reader must never be handed.

**Speed**, best of three, whole pipeline (`oxidiris <paper> --dump`: extract, tidy, parse,
tokenize, print) against `pdftotext <paper> /dev/null` which only extracts:

| Paper | Pages | Size | oxidiris | pdftotext |
|---|---|---|---|---|
| Borg | 18 | 0.9 MB | 176 ms | 144 ms |
| Omega | 14 | 4.3 MB | 288 ms | 319 ms |
| MapReduce | 13 | 0.2 MB | 24 ms | 60 ms |
| Bigtable | 14 | 0.2 MB | 36 ms | 71 ms |
| Spanner | 14 | 0.4 MB | 73 ms | 89 ms |

Doing more work in-process still beats spawning a subprocess on four of five papers. Note these
are release numbers: the debug build is 8-20x slower, so `make run` on a large PDF feels sluggish
in a way the shipped binary does not.

**Cost of the external dependency.** `pdftotext` would have to be found, version-checked and
error-mapped at runtime, on three platforms, and its absence explained to a reader who just
wanted to open a file. The prebuilt binaries shipped since v0.2.1 (OXD-077) exist precisely so
that installing oxidiris needs nothing else; a hard runtime dependency on a Poppler install would
undo that for the one format most likely to bring new users in.

## Decision

**Use `pdf-extract` 0.12, and take its content-stream order as the reading order.**

**In a crate of its own, `oxidiris-pdf`.** The backend pulls in font parsers, AES and image
codecs. `oxidiris-core` must keep compiling for `wasm32-unknown-unknown` — CI enforces it — so
none of that can live there. The crate exposes two functions, `looks_like_pdf` and `extract`, and
owns the whole third-party surface behind them.

**Output plain text, not a `Document`.** A PDF carries no reliable block structure, and the
plain-text parser already does the one thing that matters: rejoining hard-wrapped lines into
paragraphs. The pipeline is therefore:

```text
pdf bytes -> oxidiris_pdf::extract -> plain text -> oxidiris_core::load -> Document
```

**Detect by header, not by extension**, so `cat paper.pdf | oxidiris -` works and a mis-named file
still reads.

**Do not implement x-coordinate column clustering yet.** OXD-061 sized it XL on the assumption it
would be needed for correct reading order. On this corpus it is not: LaTeX emits its columns in
order, and so does the content stream. The clustering work is worth doing when a PDF that
genuinely interleaves shows up — a Word-produced or OCR-reconstructed file is the likely first
one — and not before.

## What the extracted text still needs

Three artefacts reach the RSVP frame unless something removes them, all handled in
`oxidiris-pdf`'s `cleanup` module as pure `&str -> String` steps:

- **Ligatures.** `efﬁcient` is stored with U+FB01. Expanded through a seven-entry table rather
  than NFKC, which would also rewrite superscripts, fractions and CJK compatibility forms that
  carry meaning in a paper.
- **Hyphenated line breaks.** `admission con-` / `trol` would otherwise become two display units,
  `con-` and `trol`. Welded when there is a letter on both sides and the next line starts
  lowercase. A compound that breaks at its own hyphen (`over-` / `commitment`) is welded shut by
  the same rule; separating those needs a dictionary, and every extractor in the field makes the
  same trade.
- **Isolated short numbers.** Measuring this on the corpus corrected the original guess: the bare
  numeric lines that show up are mostly **chart axis tick labels** (`20`, `40`, `60`) pulled out
  of figures, not running page numbers, which usually get glued onto the preceding line instead.
  Bigtable produces none at all. The rule drops a short numeric line only when blank lines already
  isolate it, which is what keeps a `7` inside a sentence safe.

## What is knowingly not handled

- **Figure and table text still lands in the stream.** Reading Borg, the labels from Figure 1
  (`BorgMaster`, `link shard`, `UI shard`) appear after the first-page footer. Poppler does the
  same thing. Filtering it needs the figure's bounding box, which is the same geometry work as
  OXD-061.
- **No heading structure.** `Document::headings` is empty for a PDF, so the outline panel has
  nothing to show. Section numbers survive as ordinary paragraphs.
- **Encrypted PDFs are refused, not decrypted**, with a message saying so. There is no fixture for
  this path: writing an RC4-encrypted PDF by hand to test it was judged not worth the cost, so the
  branch is exercised by inspection only.
- **Scanned pages** produce `PdfError::NoTextLayer` naming the page count and pointing at OCR.

## Fixtures

`testdata/make_pdf_fixtures.py` generates three small PDFs with no third-party library, so a
contributor can change one without installing anything:

| Fixture | Pins down |
|---|---|
| `pdf_typography.pdf` | `fi` ligature via an `/Encoding /Differences` array, a hyphenated line break, a real compound hyphen that must survive, an isolated page number |
| `pdf_two_column.pdf` | Two columns must not interleave |
| `pdf_no_text_layer.pdf` | A drawn rectangle and no text: must produce an error, not an empty reader |

## Consequences

- `oxidiris` gains a dependency tree of about 70 crates. All are permissive-licensed and the
  highest MSRV in it is 1.85, below the project's declared 1.88.
- **One advisory is now ignored in `deny.toml`.** `cargo deny` flags `ttf-parser` as unmaintained
  (RUSTSEC-2026-0192); it arrives through `lopdf` → `pdf-extract`, and the advisory states no
  upgrade exists — moving to `skrifa` is a change lopdf has to make upstream. It is a maintenance
  notice, not a vulnerability. The ignore entry names the advisory and says to delete it as soon
  as lopdf switches, so this does not quietly become permanent.
- `oxidiris-core` is untouched, and the wasm build stays clean.
- `pdf-extract` panics rather than returning `Err` on some malformed files. Spec §4.4 requires a
  message instead of a crash, so `extract` catches the panic and converts it. This is the only
  place in the project that catches a panic, and it exists because of the backend's error style.