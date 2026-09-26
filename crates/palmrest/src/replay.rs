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
///
/// Between records the tracker is ticked at its deadlines, as the reader thread's poll timeout
/// does, and after the last record until it has none left; each tick that produced outputs is a
/// `Step` of its own at the deadline. A delayed touchpad lift and a synthesized glide therefore
/// replay frame by frame. A step per record is kept for records; ticks only add steps.
pub fn replay(records: &[Record], cfg: Config, timing: Timing) -> Vec<Step> {
    replay_to_end(records, cfg, timing).1
}

/// `replay`, also returning the tracker after the last record and its ticks.
pub fn replay_to_end(records: &[Record], cfg: Config, timing: Timing) -> (Tracker, Vec<Step>) {
    let mut tracker = Tracker::new(cfg);
    let mut clock = StampClock::new();
    let mut steps = Vec::with_capacity(records.len());
    for r in records {
        let (t, ticks) = ticks_until(tracker, Some(r.at));
        steps.extend(ticks);
        let (t, clk, step) = feed(t, clock, r, timing);
        tracker = t;
        clock = clk;
        steps.push(step);
    }
    let (tracker, ticks) = ticks_until(tracker, None);
    steps.extend(ticks);
    (tracker, steps)
}

/// The most ticks one gap may take; a glide at 1 ms frames lasts a few thousand.
const MAX_TICKS: usize = 100_000;

/// Ticks `tracker` at every deadline before `until` (all of them if `None`).
fn ticks_until(mut tracker: Tracker, until: Option<Mono>) -> (Tracker, Vec<Step>) {
    let mut steps = Vec::new();
    for _ in 0..MAX_TICKS {
        let Some(d) = tracker.deadline() else { break };
        if until.is_some_and(|u| d >= u) {
            break;
        }
        let (t, outs) = tracker.tick(d);
        tracker = t;
        if !outs.is_empty() {
            steps.push(Step {
                at: d,
                outs,
                lock: tracker.lock(),
                error: None,
            });
        }
        if tracker.deadline().is_some_and(|next| next <= d) {
            break; // a deadline that does not move on would never let the replay finish
        }
    }
    (tracker, steps)
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
