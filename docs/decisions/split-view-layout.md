# ADR 002 — The status block stays full width, and the panel is a width decision

**Status:** Accepted
**Date:** 2026-09-02
**Task:** OXD-031, OXD-032
**Amends:** `docs/informations/proposals.md` §5, §3.4.4

## Context

The spec's §5 mockup draws two columns, and puts the status block *inside* the left one:

```text
┌─────── FOCUS HERE ─────────┐┌────────────── README.md ──────────────┐
│             ▼              ││ ...                                   │
├────────────────────────────┤│                                       │
│ speed: 450 WPM (eff. 361)  ││ ...                                   │
│ word: 45/999 (42%)         ││                                       │
└────────────────────────────┘└───────────────────────────────────────┘
```

Two things did not survive contact with the implementation.

**The status block does not fit in half a window.** At the 80-column minimum the left column is 40
wide, 38 inside the border. `speed: 450 WPM (eff. 361)` is 25 columns and `word: 45/999 (42%)` is
18; stacked they fit, but the key hint line — 69 columns, generated from the binding table so it
cannot silently disagree with the dispatcher — does not, and neither does the progress bar beside
it. Something would have to be clipped on every frame at the size we promise to support.

**Terminal *height* was deciding whether the panel appeared.** `size_class` in `term.rs` folds
width and height into one verdict, and `Full` needs 80x24. A 100x20 window is plenty wide for two
columns; it was losing the panel over four missing rows. Spec §3.4.4 only ever talks about
columns: *"Khi terminal < 80 cột: bỏ panel phải"*.

## Decision

**The status block spans the full window width, below both columns.** The reader frame and the
text panel split the space above it.

**Panel and outline visibility are decided on width alone** — `width >= 80`, plus the existing
`TooSmall` floor that stops anything being drawn at all. Height no longer takes the panel away.

**Below 80 columns the outline is drawn as an overlay** rather than a sidebar column. There is no
room for a third column there, and navigating is a moment of its own: playback is already paused
while the outline has focus, so covering the reader frame costs nothing.

## Consequences

**Good.** Every status field stays legible at 80 columns, which is the size the accessibility
notes in §3.4 commit to. The panel is available on short-but-wide windows — a `tmux` pane above a
shell is exactly that shape. One layout function, `ui::panes`, is the single source of truth for
where everything goes, and it is a pure function of the terminal rectangle, so the event loop can
ask it for the panel width *before* drawing and re-wrap the document only when that width changed.

**Cost.** The screen is less symmetric than the mockup. The status block is a band across the
bottom rather than a footer under the reader.

**Watch out.** `ui::panes` and `App::relayout` must agree; they do because the latter calls the
former. If a future pane is added, add it there and nowhere else.

## Alternatives rejected

- **Stack the status fields vertically inside the left column,** as the mockup does. Rejected: it
  costs four rows of the reader frame at exactly the sizes where rows are scarcest, and the key
  hints still have nowhere to go.
- **Shorten the status text when narrow.** Rejected: the effective-WPM number exists precisely
  because a reader who sees only the configured number believes a speed they are not reading at
  (§3.2.4). Dropping it when the window is small drops it for the readers most likely to be on a
  small window.