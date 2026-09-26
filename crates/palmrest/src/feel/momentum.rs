//! The glide after a flick (design/11 §11.3.6, §11.5.1): the drag `v' = -a v^b` in closed
//! form, so a frame at any time lands exactly on the curve. The same model and formula as the
//! shell-host scroll engine, so a flick glides the same distance on our surfaces and, with
//! `foreign_momentum = "ours"`, on everyone else's.
//!
//! Speeds are magnitudes in px/s; the caller carries the direction.

use crate::config::MomentumSettings;

/// `v' = -a v^b` down to the stop speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drag {
    pub a: f64,
    pub b: f64,
    pub stop: f64,
}

impl Drag {
    pub fn from_settings(m: &MomentumSettings) -> Self {
        Self {
            a: m.a.ratio(),
            b: m.b.ratio().min(0.99),
            stop: f64::from(m.stop_px_s.0.max(1)),
        }
    }

    /// Seconds until a glide from `v0` falls to the stop speed.
    pub fn duration(&self, v0: f64) -> f64 {
        if v0 <= self.stop {
            return 0.0;
        }
        let e = 1.0 - self.b;
        (v0.powf(e) - self.stop.powf(e)) / (e * self.a)
    }

    /// The speed `t` seconds in; 0 once the glide is over.
    pub fn speed(&self, v0: f64, t: f64) -> f64 {
        if t >= self.duration(v0) {
            return 0.0;
        }
        let e = 1.0 - self.b;
        (v0.powf(e) - e * self.a * t).max(0.0).powf(1.0 / e)
    }

    /// Px covered `t` seconds in; constant once the glide is over.
    pub fn travel(&self, v0: f64, t: f64) -> f64 {
        let t = t.clamp(0.0, self.duration(v0));
        if t <= 0.0 {
            return 0.0;
        }
        let (e, f) = (1.0 - self.b, 2.0 - self.b);
        let base = (v0.powf(e) - e * self.a * t).max(0.0);
        (v0.powf(f) - base.powf(f / e)) / (f * self.a)
    }

    /// The whole distance of a glide from `v0`.
    pub fn total(&self, v0: f64) -> f64 {
        self.travel(v0, self.duration(v0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn drag() -> Drag {
        Drag::from_settings(&Config::default().momentum)
    }

    #[test]
    fn the_worked_numbers_of_11_3_6() {
        // (v0 px/s, total px, duration s).
        const CASES: &[(f64, f64, f64)] = &[
            (2000.0, 501.5, 0.976),
            (1000.0, 203.7, 0.772),
            (5000.0, 1650.0, 1.319),
            (12_000.0, 5151.0, 1.749),
        ];
        let d = drag();
        for &(v0, total, secs) in CASES {
            assert!(
                (d.total(v0) - total).abs() / total < 0.005,
                "{v0}: {}",
                d.total(v0)
            );
            assert!(
                (d.duration(v0) - secs).abs() < 0.002,
                "{v0}: {}",
                d.duration(v0)
            );
        }
    }

    #[test]
    fn speed_is_the_slope_of_travel_and_the_glide_stops() {
        let d = drag();
        let (v0, t, h) = (2000.0, 0.3, 1e-4);
        let slope = (d.travel(v0, t + h) - d.travel(v0, t - h)) / (2.0 * h);
        assert!((slope - d.speed(v0, t)).abs() < 0.5, "{slope}");
        assert!((d.speed(v0, 0.0) - v0).abs() < 1e-6);
        assert_eq!(d.speed(v0, 2.0), 0.0);
        assert_eq!(d.travel(v0, 2.0), d.total(v0));
        assert_eq!(d.duration(0.5), 0.0);
        assert_eq!(d.total(0.5), 0.0);
    }
}
