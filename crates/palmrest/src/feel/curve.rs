//! The acceleration curve: which gain turns a sample's raw units into px (`feel.curve`).
//!
//! `Current` is the Python daemon's pair of gains (velocity x repeat), kept bit for bit by
//! `gain.rs`. `MacLike` replaces both with one function of finger speed, shaped after the
//! published description of the reference desktop's scroll acceleration: slow motion is
//! precise (gain exactly 1, the base ratio of `scroll.speed` and `scroll.detent_px`), faster
//! motion gains progressively, and the gain is capped. The shape is our own power law, fitted
//! to that description, not to any vendor table:
//!
//! ```text
//! g(v) = 1                                        v <= v0
//! g(v) = min(g_max, 1 + ((v - v0) / s)^p)         v >  v0
//! ```
//!
//! `v` is the sample's speed in raw units per second (the same estimator as the velocity gain:
//! `|raw| / dt` for `0 < dt <= 50 ms`, else 0). With `p > 1` the curve is continuous with a
//! continuous slope at `v0`, and monotone everywhere.

use super::gain::VELOCITY_DT_MAX;
use crate::config::{Curve, FeelSettings};

/// The `MacLike` constants, resolved from `[feel]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacLike {
    /// `v0`: below this speed the gain is 1.
    pub threshold: f64,
    /// `s`: the speed above `v0` that adds 1.0 to the gain.
    pub scale: f64,
    /// `p`.
    pub exponent: f64,
    /// `g_max`.
    pub gain_max: f64,
}

/// The curve in force, with its constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurveParams {
    Current,
    MacLike(MacLike),
}

impl CurveParams {
    pub fn from_settings(f: &FeelSettings) -> Self {
        match f.curve {
            Curve::Current => Self::Current,
            Curve::MacLike => Self::MacLike(MacLike {
                threshold: f64::from(f.mac_like_threshold_units_s.0),
                scale: f64::from(f.mac_like_scale_units_s.0.max(1)),
                exponent: f.mac_like_exponent.ratio(),
                gain_max: f.mac_like_gain_max.ratio().max(1.0),
            }),
        }
    }
}

impl MacLike {
    /// The gain at `speed` raw units per second.
    pub fn gain(&self, speed: f64) -> f64 {
        let over = (speed - self.threshold).max(0.0) / self.scale;
        (1.0 + over.powf(self.exponent)).min(self.gain_max)
    }

    /// The gain of one sample of `raw` units `dt` seconds after the previous motion.
    pub fn sample_gain(&self, raw: i32, dt: f64) -> f64 {
        if dt <= 0.0 || dt > VELOCITY_DT_MAX {
            return 1.0;
        }
        self.gain(f64::from(raw.unsigned_abs()) / dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn shipped() -> MacLike {
        let mut f = Config::default().feel;
        f.curve = Curve::MacLike;
        match CurveParams::from_settings(&f) {
            CurveParams::MacLike(m) => m,
            CurveParams::Current => unreachable!("curve was set to MacLike"),
        }
    }

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
        let m = shipped();
        for &(v, want) in CASES {
            assert!((m.gain(v) - want).abs() < 1e-5, "{v}: {}", m.gain(v));
        }
    }

    #[test]
    fn monotone_and_continuous() {
        let m = shipped();
        let gains: Vec<f64> = (0..=20_000).map(|v| m.gain(f64::from(v))).collect();
        for w in gains.windows(2) {
            assert!(w[1] >= w[0], "monotone: {w:?}");
            assert!(w[1] - w[0] < 0.01, "continuous at 1 unit/s steps: {w:?}");
        }
        // The slope at v0 is continuous too (p > 1): the first step above v0 is tiny.
        assert!(m.gain(1501.0) - 1.0 < 1e-5);
    }

    #[test]
    fn a_sample_uses_the_velocity_gains_speed_estimate() {
        let m = shipped();
        assert_eq!(m.sample_gain(60, 0.0), 1.0, "no time: no gain");
        assert_eq!(m.sample_gain(600, 0.06), 1.0, "a pause: no gain");
        assert!(
            (m.sample_gain(-54, 0.012) - 2.0).abs() < 1e-9,
            "4500 units/s"
        );
    }
}
