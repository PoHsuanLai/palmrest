//! The scroll axis lock, as a value.
//!
//! Settled behaviour (11.3.9): accumulate centroid deltas until one axis passes the threshold
//! and dominates by the ratio; lock it and drop the accumulated motion so locking never jumps;
//! a delta arriving at least the timeout after the previous motion unlocks; two fingers locking
//! X become `Blocked`, because that motion belongs to the workspace swipe. The reference-feel
//! goldens compare its decision on every report.

use super::curve::CurveParams;
use super::gain::{HI_RES_PER_DETENT, RepeatGain, raw_per_hi_res, velocity_gain};
use super::lock_event::{Hold, LockEvent, Move, Sample};
use super::quantize::{Quanta, Quantizer};
use super::rule::{Lock, LockParams, decide};
use crate::time::Mono;
use crate::touch::TouchId;
use palmrest_protocol::Axis;

/// The lock between reports.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisLock {
    set: Option<Vec<TouchId>>,
    last: Vec<Sample>,
    lock: Lock,
    last_motion: Option<Mono>,
    repeat: RepeatGain,
    repeat_gain: f64,
    last_output: Option<Mono>,
    quant: Quantizer,
}

const FREE: Lock = Lock::Free {
    pend_x: 0,
    pend_y: 0,
};

impl Default for AxisLock {
    fn default() -> Self {
        Self {
            set: None,
            last: Vec::new(),
            lock: FREE,
            last_motion: None,
            repeat: RepeatGain::default(),
            repeat_gain: 1.0,
            last_output: None,
            quant: Quantizer::default(),
        }
    }
}

impl AxisLock {
    /// The current decision.
    pub fn lock(&self) -> Lock {
        self.lock
    }

    /// The contact ids of the running stroke, sorted.
    pub fn set(&self) -> Option<&[TouchId]> {
        self.set.as_deref()
    }

    /// The repeat gain of the running stroke.
    pub fn repeat_gain(&self) -> f64 {
        self.repeat_gain
    }

    /// Drops the lock but keeps following the same contacts: a click or a palm cancelled the
    /// stroke, and the next motion must lock afresh.
    pub fn cancel(self) -> Self {
        self.unlocked()
    }

    /// Forgets the stroke and the repeat history (reconnect).
    pub fn reset_all(self) -> Self {
        Self::default()
    }

    /// Consumes one report's contacts.
    pub fn step(
        self,
        contacts: &[Sample],
        now: Mono,
        p: &LockParams,
        hold: Hold,
    ) -> (Self, Vec<LockEvent>) {
        if !(1..=2).contains(&contacts.len()) {
            let was = self.set.is_some();
            let s = Self {
                set: None,
                last: Vec::new(),
                last_motion: None,
                ..self.unlocked()
            };
            return (s, if was { vec![LockEvent::Reset] } else { vec![] });
        }
        let mut ids: Vec<TouchId> = contacts.iter().map(|c| c.id).collect();
        ids.sort();
        let duplicate = ids.windows(2).any(|w| w[0] == w[1]);
        if self.set.as_deref() != Some(&ids[..]) || duplicate {
            return (
                self.start(ids, contacts, now, p),
                vec![LockEvent::Start {
                    fingers: contacts.len() as u8,
                }],
            );
        }
        let (s, dx, dy) = self.centroid_delta(contacts);
        if (dx, dy) == (0, 0) || hold == Hold::Held {
            return (s, vec![]);
        }
        s.motion(dx, dy, now, p)
    }

    fn motion(self, dx: i32, dy: i32, now: Mono, p: &LockParams) -> (Self, Vec<LockEvent>) {
        let timed_out = self
            .last_motion
            .is_some_and(|t| now.float_since(t) >= p.timeout.as_secs_f64());
        match self.lock {
            Lock::Free { .. } => self.accumulate(dx, dy, now, p, vec![]),
            _ if timed_out => self
                .unlocked()
                .accumulate(dx, dy, now, p, vec![LockEvent::Unlocked]),
            Lock::Blocked => (
                Self {
                    last_motion: Some(now),
                    ..self
                },
                vec![],
            ),
            Lock::X | Lock::Y => self.scroll(dx, dy, now, p),
        }
    }

    fn scroll(self, dx: i32, dy: i32, now: Mono, p: &LockParams) -> (Self, Vec<LockEvent>) {
        let (axis, raw) = if self.lock == Lock::X {
            (Axis::X, dx)
        } else {
            (Axis::Y, dy)
        };
        if raw == 0 {
            return (self, vec![]);
        }
        let dt = self.last_motion.map_or(0.0, |t| now.float_since(t));
        let (repeat, velocity) = match p.curve {
            CurveParams::Classic => (
                self.repeat_gain,
                velocity_gain(raw, dt, p.velocity_gain_max),
            ),
            CurveParams::MacLike(m) => (1.0, m.sample_gain(raw, dt)),
        };
        let rpu = raw_per_hi_res(p.speed, repeat, velocity);
        let (quant, q) = self.quant.step(raw, rpu);
        let quanta = q.quanta;
        let output = quanta != Quanta::default();
        let s = Self {
            quant,
            last_motion: Some(now),
            last_output: if output { Some(now) } else { self.last_output },
            ..self
        };
        let m = Move {
            axis,
            raw,
            px: q.exact_units * p.detent_px / f64::from(HI_RES_PER_DETENT),
            quanta,
            gain: repeat * velocity,
        };
        (s, vec![LockEvent::Moved(m)])
    }

    fn accumulate(
        self,
        dx: i32,
        dy: i32,
        now: Mono,
        p: &LockParams,
        mut ev: Vec<LockEvent>,
    ) -> (Self, Vec<LockEvent>) {
        let Lock::Free { pend_x, pend_y } = self.lock else {
            return (self, ev);
        };
        let (x, y) = (pend_x + dx, pend_y + dy);
        let decided = decide(x, y, self.set.as_ref().map_or(0, Vec::len), p);
        let s = match decided {
            Some(lock) => {
                ev.push(LockEvent::Locked(lock));
                Self {
                    lock,
                    quant: Quantizer::default(),
                    last_motion: Some(now),
                    ..self
                }
            }
            None => Self {
                lock: Lock::Free {
                    pend_x: x,
                    pend_y: y,
                },
                last_motion: Some(now),
                ..self
            },
        };
        (s, ev)
    }

    fn start(self, ids: Vec<TouchId>, contacts: &[Sample], now: Mono, p: &LockParams) -> Self {
        let repeat = self.repeat.on_new_stroke(self.last_output, now);
        Self {
            set: Some(ids),
            last: contacts.to_vec(),
            last_motion: Some(now),
            repeat,
            repeat_gain: repeat.gain(p.repeat_gain_max),
            ..self.unlocked()
        }
    }

    fn unlocked(self) -> Self {
        Self {
            lock: FREE,
            quant: Quantizer::default(),
            ..self
        }
    }

    /// The per-axis mean displacement since the last report, truncated toward zero.
    fn centroid_delta(mut self, contacts: &[Sample]) -> (Self, i32, i32) {
        let (mut sx, mut sy) = (0, 0);
        for c in contacts {
            let prev = self
                .last
                .iter()
                .find(|s| s.id == c.id)
                .map_or(c.at, |s| s.at);
            let d = c.at.delta_from(prev);
            sx += d.x;
            sy += d.y;
            match self.last.iter_mut().find(|s| s.id == c.id) {
                Some(s) => s.at = c.at,
                None => self.last.push(*c),
            }
        }
        let n = contacts.len() as i32;
        (self, sx / n, sy / n)
    }
}

#[cfg(test)]
mod tests;
