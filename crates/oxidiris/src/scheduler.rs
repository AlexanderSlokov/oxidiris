//! Drift-free deadline scheduling.
//!
//! Implements the timing half of OXD-021. See spec §4.2.
//!
//! # Why not `sleep(60_000 / wpm)`
//!
//! The obvious loop adds the render time and the scheduler's own lateness to every single step.
//! At 300 WPM that is a few milliseconds per word, which compounds into tens of seconds over a
//! twenty-minute read. Worse than the total error, the rhythm becomes audibly uneven, and an
//! uneven rhythm is exactly what RSVP cannot tolerate.
//!
//! The fix is to anchor each deadline to the previous *deadline* rather than to the current time,
//! so lateness never accumulates.

use std::time::{Duration, Instant};

/// If the clock is this far behind, assume the process was suspended and resynchronise instead of
/// firing a burst of catch-up steps in the reader's face.
const RESYNC_THRESHOLD: Duration = Duration::from_secs(2);

/// Tracks when the next token is due.
#[derive(Debug, Clone, Copy)]
pub struct Deadline {
    next: Instant,
}

impl Deadline {
    /// Start a schedule at `now`.
    pub fn start(now: Instant) -> Self {
        Deadline { next: now }
    }

    /// How long to wait before the next token, given the current time.
    ///
    /// Zero when the deadline has already passed.
    pub fn timeout(&self, now: Instant) -> Duration {
        self.next.saturating_duration_since(now)
    }

    /// Whether the next token is due.
    pub fn is_due(&self, now: Instant) -> bool {
        now >= self.next
    }

    /// Schedule the following token `step` after the *previous deadline*.
    ///
    /// Anchoring to `self.next` rather than to `now` is the entire point: rendering jitter is
    /// absorbed instead of accumulated.
    pub fn advance(&mut self, now: Instant, step: Duration) {
        self.next += step;
        if now.saturating_duration_since(self.next) > RESYNC_THRESHOLD {
            self.next = now + step;
        }
    }

    /// Restart the schedule from `now`, used when playback resumes after a pause.
    pub fn resync(&mut self, now: Instant) {
        self.next = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: Duration = Duration::from_millis(200); // 300 WPM

    /// The OXD-021 acceptance case, run against a simulated clock so it finishes instantly.
    ///
    /// Every iteration pretends the render arrived late by a varying amount. A naive
    /// `now + step` scheduler would accumulate all of that lateness; this one must not.
    #[test]
    fn a_thousand_steps_accumulate_no_drift() {
        let base = Instant::now();
        let mut deadline = Deadline::start(base);
        let mut jitter = Duration::ZERO;

        for i in 0..1000u32 {
            // Simulated wall clock: the ideal time plus a render delay of 0-7 ms.
            jitter = Duration::from_millis(u64::from(i % 8));
            let now = base + STEP * i + jitter;
            deadline.advance(now, STEP);
        }

        assert_eq!(deadline.timeout(base), STEP * 1000, "deadline drifted despite per-step jitter");

        // Sanity: the jitter really was present, so the test is not vacuous.
        assert!(jitter > Duration::ZERO);
    }

    #[test]
    fn total_error_stays_far_below_one_percent() {
        let base = Instant::now();
        let mut deadline = Deadline::start(base);
        for i in 0..1000u32 {
            deadline.advance(base + STEP * i + Duration::from_millis(3), STEP);
        }
        let ideal = STEP * 1000;
        let actual = deadline.timeout(base);
        let error = actual.abs_diff(ideal);
        assert!(
            error.as_secs_f64() / ideal.as_secs_f64() < 0.01,
            "error {error:?} exceeds 1% of {ideal:?}"
        );
    }

    #[test]
    fn timeout_is_zero_once_the_deadline_has_passed() {
        let base = Instant::now();
        let d = Deadline::start(base);
        assert_eq!(d.timeout(base + Duration::from_secs(1)), Duration::ZERO);
        assert!(d.is_due(base));
    }

    #[test]
    fn timeout_counts_down_towards_the_deadline() {
        let base = Instant::now();
        let mut d = Deadline::start(base);
        d.advance(base, STEP);
        assert_eq!(d.timeout(base), STEP);
        assert_eq!(d.timeout(base + Duration::from_millis(50)), Duration::from_millis(150));
    }

    /// After a laptop suspend the deadline is hours behind. Catching up would fire thousands of
    /// tokens instantly; resynchronising is the only sane behaviour.
    #[test]
    fn a_long_stall_resynchronises_instead_of_replaying() {
        let base = Instant::now();
        let mut d = Deadline::start(base);
        let woke_up = base + Duration::from_secs(3600);
        d.advance(woke_up, STEP);
        assert_eq!(d.timeout(woke_up), STEP);
    }

    #[test]
    fn a_short_stall_is_absorbed_not_resynchronised() {
        let base = Instant::now();
        let mut d = Deadline::start(base);
        let slightly_late = base + Duration::from_millis(500);
        d.advance(slightly_late, STEP);
        assert_eq!(d.timeout(base), STEP);
    }
}
