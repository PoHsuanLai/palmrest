//! The acceleration curve: the gain that turns a sample's raw units into px.
//!
//! One function of finger speed, shaped after the published description of the reference
//! desktop's scroll acceleration: slow motion is precise (gain exactly 1, the base ratio of
//! `scroll.speed` and the wheel step), faster motion gains progressively, and the gain is capped.
//! The shape is our own power law, fitted to that description, not to any vendor table:
//!
//! ```text
//! g(v) = 1                                        v <= v0
//! g(v) = min(g_max, 1 + ((v - v0) / s)^p)         v >  v0
//! ```
//!
//! `v` is the sample's speed in raw units per second (`|raw| / dt` for `0 < dt <= 50 ms`, else
//! 0). With `p > 1` the curve is continuous with a continuous slope at `v0`, and monotone
//! everywhere.
//!
//! Speeds are grid units per second of one sample (26/mm across, 70/mm along: the Magic Mouse
//! 2's raw units). They shape the transfer function rather than recognise a gesture, and apply
//! to whichever axis locked, so they stay in grid units rather than becoming x/y millimetres.

use super::gain::VELOCITY_DT_MAX;
use crate::config::{Count, Fraction};

/// `v0`: below this speed the gain is 1.
const PRECISE_BELOW: Count = Count(1500);
/// `s`: the speed above `v0` that adds 1.0 to the gain.
const RAMP: Count = Count(3000);
/// `p`.
const EXPONENT: Fraction = Fraction(1600);
/// `g_max`.
const GAIN_MAX: Fraction = Fraction(6000);

/// The gain at `speed` raw units per second.
pub fn gain(speed: f64) -> f64 {
    let over = (speed - f64::from(PRECISE_BELOW.0)).max(0.0) / f64::from(RAMP.0);
    (1.0 + over.powf(EXPONENT.ratio())).min(GAIN_MAX.ratio())
}

/// The gain of one sample of `raw` units `dt` seconds after the previous motion.
pub fn sample_gain(raw: i32, dt: f64) -> f64 {
    if dt <= 0.0 || dt > VELOCITY_DT_MAX {
        return 1.0;
    }
    gain(f64::from(raw.unsigned_abs()) / dt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_shape() {
        // (speed units/s, gain): 1:1 up to v0 = 1500, 2.0 at v0 + s, the cap 6.0 from
        // 5^(1/1.6) * 3000 + 1500 = 9703 units/s.
        const CASES: &[(f64, f64)] = &[
            (0.0, 1.0),
            (750.0, 1.0),
            (1500.0, 1.0),
            (2250.0, 1.108_818),
            (3000.0, 1.329_877),
            (4500.0, 2.0),
            (6000.0, 2.913_137),
            (9000.0, 5.332_155),
            (9703.0, 5.999_912),
            (10_000.0, 6.0),
            (30_000.0, 6.0),
        ];
        for &(v, want) in CASES {
            assert!((gain(v) - want).abs() < 1e-5, "{v}: {}", gain(v));
        }
    }

    #[test]
    fn monotone_and_continuous() {
        let gains: Vec<f64> = (0..=20_000).map(|v| gain(f64::from(v))).collect();
        for w in gains.windows(2) {
            assert!(w[1] >= w[0], "monotone: {w:?}");
            assert!(w[1] - w[0] < 0.01, "continuous at 1 unit/s steps: {w:?}");
        }
        // The slope at v0 is continuous too (p > 1): the first step above v0 is tiny.
        assert!(gain(1501.0) - 1.0 < 1e-5);
    }

    #[test]
    fn a_sample_uses_the_speed_estimate_of_a_short_gap() {
        assert_eq!(sample_gain(60, 0.0), 1.0, "no time: no gain");
        assert_eq!(sample_gain(600, 0.06), 1.0, "a pause: no gain");
        assert!((sample_gain(-54, 0.012) - 2.0).abs() < 1e-9, "4500 units/s");
    }
}
