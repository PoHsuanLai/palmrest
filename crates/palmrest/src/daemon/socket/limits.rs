//! What the socket thread allows a subscriber, and when it stops listening or stops complaining.
//! Every bound is against a subscriber that is broken or hostile: it is refused or cut, and
//! never allowed to wedge the thread that serves shell-host.

use latchkey::guard::{ACCEPT_PAUSE, COMPLAINT_INTERVAL, Voice};
use std::time::{Duration, Instant};

/// Subscribers connected at once. shell-host is the only one in practice; a few more cover a
/// restart overlapping its predecessor and tools such as a stream viewer.
pub(super) const MAX_CLIENTS: usize = 8;
/// Packets read from one subscriber in one pass; the rest wait for the next pass, so one chatty
/// subscriber cannot keep the others, or the reader's messages, waiting.
pub(super) const PACKETS_PER_PASS: usize = 16;
/// Connections accepted in one pass.
pub(super) const ACCEPTS_PER_PASS: usize = 8;
/// Messages the reader may have queued for the socket thread. At 90 reports a second this is
/// more than ten seconds of a stuck thread; past it the reader drops messages rather than wait.
pub(super) const QUEUE_MAX: usize = 1024;
/// How often the seat file is read when no subscriber says anything, so a user switch is noticed
/// with no traffic.
pub(super) const SEAT_RECHECK: Duration = Duration::from_secs(1);

/// Whether the listener is polled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Intake {
    Open,
    /// Not polled until this instant.
    PausedUntil(Instant),
}

impl Intake {
    /// The listener, left alone from `now` for `ACCEPT_PAUSE`.
    pub(super) fn paused_at(now: Instant) -> Self {
        Intake::PausedUntil(now + ACCEPT_PAUSE)
    }

    /// The state at `now`: a pause that has run out is over.
    pub(super) fn at(self, now: Instant) -> Self {
        match self {
            Intake::PausedUntil(until) if until > now => self,
            _ => Intake::Open,
        }
    }

    /// How long the thread may sleep: for ever while the listener is open, else until the pause
    /// ends.
    pub(super) fn sleep(self, now: Instant) -> Option<Duration> {
        match self.at(now) {
            Intake::PausedUntil(until) => Some(until.saturating_duration_since(now)),
            Intake::Open => None,
        }
    }
}

/// The last time a refusal was logged, so a subscriber that keeps knocking cannot fill the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Complaints {
    last: Option<Instant>,
}

impl Complaints {
    /// Notes a refusal at `now`: spoken the first time and then once per `COMPLAINT_INTERVAL`.
    pub(super) fn note(self, now: Instant) -> (Self, Voice) {
        match self.last {
            Some(at) if now.saturating_duration_since(at) < COMPLAINT_INTERVAL => {
                (self, Voice::Silent)
            }
            _ => (Self { last: Some(now) }, Voice::Speak),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pause_ends_and_sets_the_sleep() {
        let t0 = Instant::now();
        let paused = Intake::paused_at(t0);
        assert_eq!(paused.at(t0 + ACCEPT_PAUSE / 2), paused);
        assert_eq!(paused.at(t0 + ACCEPT_PAUSE), Intake::Open);
        assert_eq!(Intake::Open.sleep(t0), None);
        assert_eq!(paused.sleep(t0 + ACCEPT_PAUSE / 2), Some(ACCEPT_PAUSE / 2));
        assert_eq!(paused.sleep(t0 + ACCEPT_PAUSE * 2), None);
    }

    #[test]
    fn a_refusal_is_logged_once_an_interval() {
        let t0 = Instant::now();
        let (c, first) = Complaints::default().note(t0);
        assert_eq!(first, Voice::Speak);
        let (c, again) = c.note(t0 + Duration::from_secs(1));
        assert_eq!(again, Voice::Silent);
        let (_, later) = c.note(t0 + COMPLAINT_INTERVAL);
        assert_eq!(later, Voice::Speak);
    }
}
