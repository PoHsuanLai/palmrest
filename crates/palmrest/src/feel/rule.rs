//! The axis-lock rule and its parameters (11.3.9, 12.5): which axis a stroke's accumulated
//! motion locks to, if any.

use crate::config::{Config, Fraction, Ms, Px, Um};
use crate::sensor::lattice::units;
use palmrest_protocol::Axis;
use std::time::Duration;

/// Accumulated motion an axis needs before it can lock, per axis, in grid units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Threshold {
    pub x: i32,
    pub y: i32,
}

impl Threshold {
    /// The same number of grid units on both axes.
    pub const fn both(units: i32) -> Self {
        Self { x: units, y: units }
    }
}

/// Accumulated sideways motion before the X axis can lock.
const LOCK_THRESHOLD_X: Um = Um(1538);
/// Accumulated up or down motion before the Y axis can lock.
const LOCK_THRESHOLD_Y: Um = Um(571);
/// How much further one axis must go than the other to lock (1.5).
const LOCK_RATIO: Fraction = Fraction(1500);
/// A delta arriving this long after the previous motion unlocks.
const LOCK_TIMEOUT: Ms = Ms(250);
/// Pixels scrolled by one wheel step at the base ratio.
const DETENT_PX: Px = Px(60);

/// The lock's tunables, resolved from the config into the units the arithmetic uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LockParams {
    pub threshold: Threshold,
    pub ratio: f64,
    pub timeout: Duration,
    pub speed: u16,
    pub detent_px: f64,
}

/// The lock decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lock {
    /// Not locked yet: motion accumulates here.
    Free {
        pend_x: i32,
        pend_y: i32,
    },
    X,
    Y,
    /// Two fingers moved sideways: no scroll until the lock times out.
    Blocked,
}

/// The lock rule: X first, then Y; two fingers on X are Blocked.
pub fn decide(x: i32, y: i32, fingers: usize, p: &LockParams) -> Option<Lock> {
    let (ax, ay) = (f64::from(x.abs()), f64::from(y.abs()));
    let tx = f64::from(p.threshold.x.max(1));
    let ty = f64::from(p.threshold.y.max(1));
    let lock = if ax >= tx && ax >= ay * p.ratio {
        Lock::X
    } else if ay >= ty && ay >= ax * p.ratio {
        Lock::Y
    } else {
        return None;
    };
    Some(if lock == Lock::X && fingers > 1 {
        Lock::Blocked
    } else {
        lock
    })
}

impl LockParams {
    /// The lock's parameters from the config, with the P5 threshold multiplier applied when
    /// the mouse is moving.
    pub fn from_config(c: &Config, threshold_multiplier: f64) -> Self {
        Self {
            threshold: Threshold {
                x: scaled(LOCK_THRESHOLD_X, Axis::X, threshold_multiplier),
                y: scaled(LOCK_THRESHOLD_Y, Axis::Y, threshold_multiplier),
            },
            ratio: LOCK_RATIO.ratio(),
            timeout: LOCK_TIMEOUT.duration(),
            speed: c.scroll.speed.0,
            detent_px: f64::from(DETENT_PX.0),
        }
    }
}

/// A threshold on the grid times the P5 multiplier, rounded as the reference feel rounds it.
fn scaled(um: Um, axis: Axis, multiplier: f64) -> i32 {
    (f64::from(units(um, axis)) * multiplier).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARAMS: LockParams = LockParams {
        threshold: Threshold::both(40),
        ratio: 1.5,
        timeout: Duration::from_millis(250),
        speed: 32,
        detent_px: 60.0,
    };

    #[test]
    fn decide_table() {
        const CASES: &[(i32, i32, usize, Option<Lock>)] = &[
            (39, 0, 1, None),
            (40, 0, 1, Some(Lock::X)),
            (-40, 26, 1, Some(Lock::X)),
            (-40, 27, 1, None),
            (0, -40, 1, Some(Lock::Y)),
            (26, 40, 1, Some(Lock::Y)),
            (40, 0, 2, Some(Lock::Blocked)),
            (0, 40, 2, Some(Lock::Y)),
        ];
        for &(x, y, n, want) in CASES {
            assert_eq!(decide(x, y, n, &PARAMS), want, "({x}, {y}) x{n}");
        }
    }
}
