//! The axis-lock rule and its parameters (11.3.9, 12.5): which axis a stroke's accumulated
//! motion locks to, if any.

use super::curve::CurveParams;
use std::time::Duration;

/// The lock's tunables, resolved from the config into the units the arithmetic uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LockParams {
    pub threshold: i32,
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
    let t = f64::from(p.threshold.max(1));
    let lock = if ax >= t && ax >= ay * p.ratio {
        Lock::X
    } else if ay >= t && ay >= ax * p.ratio {
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

#[cfg(test)]
mod tests {
    use super::*;

    const PARAMS: LockParams = LockParams {
        threshold: 40,
        ratio: 1.5,
        timeout: Duration::from_millis(250),
        speed: 32,
        velocity_gain_max: 2.0,
        repeat_gain_max: 1.0,
        detent_px: 60.0,
        curve: CurveParams::Current,
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
