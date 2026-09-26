//! Px-free wheel output: hi-res and detent units with carried remainders (O4).
//!
//! Only the wheel device quantises; the socket carries exact px. Kept byte-for-byte with the
//! Python daemon so the switch-over does not change how far a foreign window scrolls.

use crate::protocol::Axis;
use serde::{Deserialize, Serialize};

/// Remainders carried between samples of one locked stroke.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Quantizer {
    raw_remainder: f64,
    hi_res_remainder: i32,
}

/// Whole units produced by one sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Quanta {
    pub hi_res: i32,
    pub detents: i32,
}

impl Quantizer {
    /// Adds `raw` units at `raw_per_hi_res` and returns the whole units now due.
    pub fn step(self, raw: i32, raw_per_hi_res: f64) -> (Self, Quanta) {
        let raw_remainder = self.raw_remainder + f64::from(raw);
        let hi_res = (raw_remainder / raw_per_hi_res).trunc() as i32;
        let raw_remainder = raw_remainder - f64::from(hi_res) * raw_per_hi_res;
        let carried = self.hi_res_remainder + hi_res;
        let detents = carried / 120;
        let next = Self {
            raw_remainder,
            hi_res_remainder: carried - detents * 120,
        };
        (next, Quanta { hi_res, detents })
    }
}

/// One wheel frame in the Python device's orientation: its y is the raw report's unsigned y,
/// which is palmrest's y negated. Plasma saved `NaturalScroll=true` against that orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WheelDelta {
    pub horizontal: i32,
    pub vertical: i32,
    pub horizontal_hi_res: i32,
    pub vertical_hi_res: i32,
}

impl WheelDelta {
    /// The frame for quanta on `axis` (palmrest's frame in, Python's frame out).
    pub fn on(axis: Axis, q: Quanta) -> Self {
        match axis {
            Axis::X => Self {
                horizontal: q.detents,
                horizontal_hi_res: q.hi_res,
                ..Self::default()
            },
            Axis::Y => Self {
                vertical: -q.detents,
                vertical_hi_res: -q.hi_res,
                ..Self::default()
            },
        }
    }

    /// Whether anything would be written.
    pub fn has_events(self) -> bool {
        self != Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remainders_carry() {
        let (q, a) = Quantizer::default().step(5, 2.0);
        assert_eq!(
            a,
            Quanta {
                hi_res: 2,
                detents: 0
            }
        );
        let (_, b) = q.step(1, 2.0);
        assert_eq!(
            b,
            Quanta {
                hi_res: 1,
                detents: 0
            },
            "the half unit carried over"
        );
    }

    #[test]
    fn detents_every_120_hi_res() {
        let mut q = Quantizer::default();
        let mut detents = 0;
        for _ in 0..25 {
            let (n, a) = q.step(10, 1.0);
            q = n;
            detents += a.detents;
        }
        assert_eq!(detents, 2);
        let (_, back) = q.step(-250, 1.0);
        assert_eq!(
            back,
            Quanta {
                hi_res: -250,
                detents: -2
            },
            "truncates toward zero"
        );
    }

    #[test]
    fn wheel_y_is_flipped_into_the_python_orientation() {
        let q = Quanta {
            hi_res: 7,
            detents: 1,
        };
        assert_eq!(
            WheelDelta::on(Axis::Y, q),
            WheelDelta {
                vertical: -1,
                vertical_hi_res: -7,
                ..WheelDelta::default()
            }
        );
        assert_eq!(
            WheelDelta::on(Axis::X, q),
            WheelDelta {
                horizontal: 1,
                horizontal_hi_res: 7,
                ..WheelDelta::default()
            }
        );
    }
}
