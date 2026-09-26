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
    /// The raw remainder's value in hi-res units when it was carried: what the exact stream
    /// has already counted of it.
    carried_units: f64,
}

/// Whole units produced by one sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Quanta {
    pub hi_res: i32,
    pub detents: i32,
}

/// One sample through the quantiser: the wheel's whole units and the exact hi-res amount the
/// socket carries (as px).
///
/// The exact stream is the wheel stream before truncation: after every sample, the exact
/// total equals the wheel's hi-res total plus the carried remainder (valued at this sample's
/// rate), so the two never differ by a whole unit within a stroke.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantized {
    pub quanta: Quanta,
    pub exact_units: f64,
}

impl Quantizer {
    /// Adds `raw` units at `raw_per_hi_res` and returns the whole units now due.
    pub fn step(self, raw: i32, raw_per_hi_res: f64) -> (Self, Quantized) {
        let raw_remainder = self.raw_remainder + f64::from(raw);
        let hi_res = (raw_remainder / raw_per_hi_res).trunc() as i32;
        let raw_remainder = raw_remainder - f64::from(hi_res) * raw_per_hi_res;
        let carried = self.hi_res_remainder + hi_res;
        let detents = carried / 120;
        let carried_units = raw_remainder / raw_per_hi_res;
        let exact_units = f64::from(hi_res) + carried_units - self.carried_units;
        let next = Self {
            raw_remainder,
            hi_res_remainder: carried - detents * 120,
            carried_units,
        };
        (
            next,
            Quantized {
                quanta: Quanta { hi_res, detents },
                exact_units,
            },
        )
    }
}

/// Fractional hi-res units carried between frames of a synthesized glide, which has px, not
/// raw units, to quantise.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UnitCarry {
    units: f64,
    hi_res_remainder: i32,
}

impl UnitCarry {
    /// Adds `units` hi-res units and returns the whole units now due.
    pub fn step(self, units: f64) -> (Self, Quanta) {
        let total = self.units + units;
        let hi_res = total.trunc() as i32;
        let carried = self.hi_res_remainder + hi_res;
        let detents = carried / 120;
        let next = Self {
            units: total - f64::from(hi_res),
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
            a.quanta,
            Quanta {
                hi_res: 2,
                detents: 0
            }
        );
        assert_eq!(a.exact_units, 2.5);
        let (_, b) = q.step(1, 2.0);
        assert_eq!(
            b.quanta,
            Quanta {
                hi_res: 1,
                detents: 0
            },
            "the half unit carried over"
        );
        assert_eq!(b.exact_units, 0.5);
    }

    #[test]
    fn detents_every_120_hi_res() {
        let mut q = Quantizer::default();
        let mut detents = 0;
        for _ in 0..25 {
            let (n, a) = q.step(10, 1.0);
            q = n;
            detents += a.quanta.detents;
        }
        assert_eq!(detents, 2);
        let (_, back) = q.step(-250, 1.0);
        assert_eq!(
            back.quanta,
            Quanta {
                hi_res: -250,
                detents: -2
            },
            "truncates toward zero"
        );
    }

    #[test]
    fn exact_stream_stays_within_one_unit_of_the_wheel_as_the_rate_changes() {
        let mut q = Quantizer::default();
        let (mut exact, mut wheel) = (0.0, 0);
        for (i, raw) in [7, 3, 11, 2, 9, 1, 13, 4].iter().enumerate() {
            let rate = 2.0 + (i % 3) as f64 * 0.7;
            let (n, a) = q.step(*raw, rate);
            q = n;
            exact += a.exact_units;
            wheel += a.quanta.hi_res;
            assert!(
                (exact - f64::from(wheel)).abs() < 1.0,
                "sample {i}: {exact} vs {wheel}"
            );
        }
    }

    #[test]
    fn unit_carry_keeps_fractions_and_counts_detents() {
        let (c, a) = UnitCarry::default().step(0.6);
        assert_eq!(a, Quanta::default());
        let (c, b) = c.step(0.6);
        assert_eq!(b.hi_res, 1, "0.6 + 0.6 = 1.2");
        let (_, d) = c.step(-5.3);
        assert_eq!(d.hi_res, -5, "0.2 - 5.3 truncates toward zero");
        let (_, e) = (0..3).fold((UnitCarry::default(), 0), |(c, n), _| {
            let (c, q) = c.step(50.0);
            (c, n + q.detents)
        });
        assert_eq!(e, 1, "150 hi-res units cross one detent");
    }

    #[test]
    fn wheel_y_is_flipped_into_the_python_orientation() {
        let q = Quanta {
            hi_res: 7,
            detents: 1,
        };
        let y = WheelDelta {
            vertical: -1,
            vertical_hi_res: -7,
            ..WheelDelta::default()
        };
        assert_eq!(WheelDelta::on(Axis::Y, q), y);
        let x = WheelDelta {
            horizontal: 1,
            horizontal_hi_res: 7,
            ..WheelDelta::default()
        };
        assert_eq!(WheelDelta::on(Axis::X, q), x);
    }
}
