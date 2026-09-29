//! The axis-lock rule and its parameters (11.3.9, 12.5): which axis a stroke's accumulated
//! motion locks to, if any.

use super::curve::CurveParams;
use crate::config::{Config, Um};
use crate::sensor::lattice::{Along, units};
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

/// The lock's tunables, resolved from the config into the units the arithmetic uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LockParams {
    pub threshold: Threshold,
    pub ratio: f64,
    pub timeout: Duration,
    pub speed: u16,
    pub velocity_gain_max: f64,
    pub repeat_gain_max: f64,
    pub detent_px: f64,
    pub curve: CurveParams,
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
        let s = &c.scroll;
        Self {
            threshold: Threshold {
                x: scaled(s.lock_threshold_x_um, Along::X, threshold_multiplier),
                y: scaled(s.lock_threshold_y_um, Along::Y, threshold_multiplier),
            },
            ratio: s.lock_ratio.ratio(),
            timeout: s.lock_timeout_ms.duration(),
            speed: s.speed.0,
            velocity_gain_max: s.velocity_gain_max.ratio(),
            repeat_gain_max: s.repeat_gain_max.ratio(),
            detent_px: f64::from(s.detent_px.0),
            curve: CurveParams::from_settings(&c.feel),
        }
    }
}

/// A threshold on the grid times the P5 multiplier, rounded as the reference feel rounds it.
fn scaled(um: Um, axis: Along, multiplier: f64) -> i32 {
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
        velocity_gain_max: 2.0,
        repeat_gain_max: 1.0,
        detent_px: 60.0,
        curve: CurveParams::Classic,
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
