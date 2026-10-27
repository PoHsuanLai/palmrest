//! G4: a quick one-finger horizontal flick is Back or Forward.
//!
//! Evaluated at lift and never instead of scrolling: the stroke has already scrolled
//! horizontally by the time the finger lifts, and on our own surfaces the host makes the swipe
//! decision itself (12.3.6), so this only ever adds a key for foreign clients.

use super::swipe::FingerDirection;
use crate::config::{Fraction, Ms, Um};
use crate::feel::{Hold, Lock, Sample};
use crate::sensor::lattice::{Along, units};
use crate::time::Mono;
use crate::touch::{Point, TouchId};

/// How far one finger slides to swipe.
const TRAVEL: Um = Um(11538);
/// The longest the swipe may take from the axis lock.
const MAX_DURATION: Ms = Ms(400);
/// How much further sideways than up or down (2.0).
const AXIS_RATIO: Fraction = Fraction(2000);
/// How soon before lifting the last movement must be.
const SETTLE: Ms = Ms(50);

/// A one-finger stroke being watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stroke {
    id: TouchId,
    origin: Point,
    at: Point,
    last_motion: Mono,
    locked_at: Option<Mono>,
}

/// The one-finger swipe between reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OneSwipe {
    #[default]
    Idle,
    Tracking(Stroke),
    /// Disqualified (a second finger, a Y lock, a click) until every finger lifts.
    Void,
}

impl OneSwipe {
    /// Consumes one report's contacts and the scroll lock's decision after it.
    pub fn step(
        self,
        contacts: &[Sample],
        lock: Lock,
        now: Mono,
        hold: Hold,
    ) -> (Self, Option<FingerDirection>) {
        match (self, contacts) {
            (Self::Tracking(s), []) => (Self::Idle, judge(&s, now)),
            (_, []) => (Self::Idle, None),
            (_, _) if hold == Hold::Held => (Self::Void, None),
            (Self::Void, _) => (Self::Void, None),
            (Self::Idle, [c]) => (
                Self::Tracking(Stroke {
                    id: c.id,
                    origin: c.at,
                    at: c.at,
                    last_motion: now,
                    locked_at: None,
                })
                .locked(lock, now),
                None,
            ),
            (Self::Tracking(s), [c]) if s.id == c.id => {
                (Self::Tracking(s.moved(c.at, now)).locked(lock, now), None)
            }
            _ => (Self::Void, None),
        }
    }

    fn locked(self, lock: Lock, now: Mono) -> Self {
        match (self, lock) {
            (Self::Tracking(s), Lock::X) => Self::Tracking(Stroke {
                locked_at: s.locked_at.or(Some(now)),
                ..s
            }),
            (Self::Tracking(_), Lock::Y | Lock::Blocked) => Self::Void,
            (other, _) => other,
        }
    }
}

impl Stroke {
    fn moved(self, at: Point, now: Mono) -> Self {
        if at == self.at {
            self
        } else {
            Self {
                at,
                last_motion: now,
                ..self
            }
        }
    }
}

/// The G4 rule at lift: far enough, fast enough, straight enough, still moving.
fn judge(s: &Stroke, lift: Mono) -> Option<FingerDirection> {
    let locked_at = s.locked_at?;
    let travel = s.at.delta_from(s.origin);
    let far = travel.x.abs() >= units(TRAVEL, Along::X);
    let quick = lift.since(locked_at) <= MAX_DURATION.duration();
    let straight = f64::from(travel.x.abs()) >= AXIS_RATIO.ratio() * f64::from(travel.y.abs());
    let moving = lift.since(s.last_motion) <= SETTLE.duration();
    (far && quick && straight && moving).then_some(if travel.x > 0 {
        FingerDirection::Right
    } else {
        FingerDirection::Left
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i32, y: i32) -> Vec<Sample> {
        vec![Sample {
            id: TouchId(1),
            at: Point::new(x, y),
        }]
    }

    /// A stroke of `n` samples 10 ms apart, locking X at sample 2, then a lift `settle` ms later.
    fn stroke(dx: i32, dy: i32, n: u64, settle: u64) -> Option<FingerDirection> {
        let mut s = OneSwipe::Idle;
        for i in 0..n {
            let lock = if i >= 2 {
                Lock::X
            } else {
                Lock::Free {
                    pend_x: 0,
                    pend_y: 0,
                }
            };
            let (next, fired) = s.step(
                &at(dx * i as i32, dy * i as i32),
                lock,
                Mono::from_millis(i * 10),
                Hold::Open,
            );
            assert_eq!(fired, None);
            s = next;
        }
        s.step(
            &[],
            Lock::X,
            Mono::from_millis((n - 1) * 10 + settle),
            Hold::Open,
        )
        .1
    }

    #[test]
    fn g4_table() {
        /// dx and dy per sample, samples, settle ms, expected, name.
        type Case = (i32, i32, u64, u64, Option<FingerDirection>, &'static str);
        const CASES: &[Case] = &[
            (
                15,
                0,
                26,
                8,
                Some(FingerDirection::Right),
                "375 units in 250 ms",
            ),
            (-15, 0, 26, 8, Some(FingerDirection::Left), "leftwards"),
            (10, 0, 26, 8, None, "250 units is short"),
            (6, 0, 61, 8, None, "600 ms is slow"),
            (15, 8, 26, 8, None, "not straight"),
            (15, 0, 26, 60, None, "paused before lifting"),
        ];
        for &(dx, dy, n, settle, want, name) in CASES {
            assert_eq!(stroke(dx, dy, n, settle), want, "{name}");
        }
    }

    #[test]
    fn a_second_finger_voids_it() {
        let two = vec![
            Sample {
                id: TouchId(1),
                at: Point::new(0, 0),
            },
            Sample {
                id: TouchId(2),
                at: Point::new(500, 0),
            },
        ];
        let (s, _) = OneSwipe::Idle.step(&at(0, 0), Lock::X, Mono(0), Hold::Open);
        let (s, _) = s.step(&two, Lock::X, Mono::from_millis(10), Hold::Open);
        let (s, _) = s.step(&at(400, 0), Lock::X, Mono::from_millis(20), Hold::Open);
        assert_eq!(
            s.step(&[], Lock::X, Mono::from_millis(30), Hold::Open).1,
            None
        );
    }
}
