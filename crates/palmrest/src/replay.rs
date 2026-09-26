//! `palmrest replay`: a recording through the pure core, exactly as the reader thread drives it.
//!
//! The acceptance tests and the parity comparison run through here, so what they check is the
//! daemon's own path from bytes to outputs, not a test-only shortcut.

use crate::config::Config;
use crate::decode::{DecodeError, decode};
use crate::feel::Lock;
use crate::recording::Record;
use crate::stamp::StampClock;
use crate::time::Mono;
use crate::tracker::{Out, Tracker};

/// Where event times come from during a replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    /// The recorded arrival times, as-is (fixtures, parity).
    Arrival,
    /// Through the timestamp calibration, as the live daemon does.
    Calibrated,
}

/// What one record produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub at: Mono,
    pub outs: Vec<Out>,
    /// The scroll lock decision after this record (parity compares it).
    pub lock: Lock,
    pub error: Option<DecodeError>,
}

/// Replays `records` from a fresh tracker.
pub fn replay(records: &[Record], cfg: Config, timing: Timing) -> Vec<Step> {
    let mut tracker = Tracker::new(cfg);
    let mut clock = StampClock::new();
    let mut steps = Vec::with_capacity(records.len());
    for r in records {
        let (t, clk, step) = feed(tracker, clock, r, timing);
        tracker = t;
        clock = clk;
        steps.push(step);
    }
    steps
}

/// One hidraw read through decode, the stamp clock and the tracker: the reader thread's body.
pub fn feed(
    tracker: Tracker,
    clock: StampClock,
    r: &Record,
    timing: Timing,
) -> (Tracker, StampClock, Step) {
    match decode(&r.bytes) {
        Err(e) => {
            let lock = tracker.lock();
            (
                tracker,
                clock,
                Step {
                    at: r.at,
                    outs: vec![],
                    lock,
                    error: Some(e),
                },
            )
        }
        Ok(reports) => {
            let (mut tracker, mut clock, mut outs) = (tracker, clock, Vec::new());
            for report in &reports {
                let (c, now) = match timing {
                    Timing::Arrival => (clock, r.at),
                    Timing::Calibrated => clock.observe(report.stamp, r.at),
                };
                let (t, o) = tracker.step(report, now);
                tracker = t;
                clock = c;
                outs.extend(o);
            }
            let lock = tracker.lock();
            (
                tracker,
                clock,
                Step {
                    at: r.at,
                    outs,
                    lock,
                    error: None,
                },
            )
        }
    }
}

/// Every output of a replay, flattened.
pub fn outs(steps: &[Step]) -> Vec<Out> {
    steps.iter().flat_map(|s| s.outs.iter().cloned()).collect()
}
