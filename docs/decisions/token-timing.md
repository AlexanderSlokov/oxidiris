# ADR 001 — Tokens carry a weight, not a duration

**Status:** Accepted
**Date:** 2026-09-01
**Task:** OXD-010, OXD-018
**Amends:** `docs/informations/proposals.md` §4.1

## Context

The spec sketched `Token` with a `duration_ms: u32` field: the display time baked in at parse
time. That is the obvious shape, and it is wrong for one specific reason.

Reading speed is adjustable *during* playback (`J`/`K`, spec §7.2). If duration is a stored field,
every keypress forces one of two bad options:

1. **Re-pace the whole document.** A 5 500-token file gets rewritten on every 25 WPM nudge, and
   the reader can hold the key down.
2. **Leave old durations stale.** The tokens already read keep their timing, which is correct, but
   so do the tokens *ahead* of the cursor, which is not — the speed change would not take effect
   until the document was reloaded.

There is a second, quieter problem: a stored duration makes `Token` untestable without deciding a
speed first, and it pushes a playback concern into the parser.

## Decision

`Token` stores the WPM-independent parts of the timing model:

```rust
pub struct Token {
    pub weight: f32,     // length x punctuation x kind
    pub pause_ms: u32,   // structural pause, not scaled by speed
    // ...
}

impl Token {
    pub fn duration_ms(&self, wpm: u16) -> u32 {
        let base = 60_000.0 / f32::from(wpm.max(1));
        (base * self.weight) as u32 + self.pause_ms
    }
}
```

Duration is derived at the moment the token is shown, from the speed in force at that moment.

## Consequences

**Good.** A speed change is `self.wpm = new` and nothing else: naturally non-retroactive, O(1),
and correct while the key is held down. Structural pauses stay absolute, which is what we want —
the beat after a paragraph should not shrink just because the reader sped up. `Player` needs no
clock to be tested, which is what lets OXD-018 be verified with simulated time and reused as-is
under WebAssembly (OXD-080).

**Cost.** Callers that want a duration must supply a speed. In practice only two places do:
`Player::current_duration_ms` and `pacing::effective_wpm`.

**Watch out.** `weight` is an `f32` multiplier chain. Do not accumulate it across tokens, and do
not compare weights for equality across pacing modes.

## Alternatives rejected

- **Store duration and re-pace on change.** Rejected on cost: O(n) per keypress, and the parser
  would need to know the speed.
- **Store duration at a reference WPM and scale on read.** Equivalent to this decision but loses
  precision twice and hides the structural pause inside a scalable number, so a fast reader would
  get shortened paragraph breaks.