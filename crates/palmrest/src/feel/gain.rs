//! Raw units to px: the base ratio, the velocity gain and the repeat gain (12.3.8, 11.5.5).
//!
//! The operation order is fixed, because the reference-feel goldens compare truncated wheel
//! units and a reordered float product can move a truncation by one.

use crate::time::Mono;
use std::time::Duration;

/// Raw units per detent are `(64 - speed) * ACCEL_BASE`; also the repeat divisor's start.
pub const ACCEL_BASE: u32 = 7;
/// Hi-res wheel units per detent (Linux `REL_WHEEL_HI_RES`).
pub const HI_RES_PER_DETENT: i32 = 120;
/// A new stroke this soon after the last output counts as a repeat.
pub const REPEAT_WINDOW: Duration = Duration::from_millis(500);
/// The velocity gain applies only to samples at most this far apart.
pub const VELOCITY_DT_MAX: f64 = 0.05;
/// Raw units per second that add 1.0 to the velocity gain.
pub const VELOCITY_RATE_UNIT: f64 = 2000.0;
/// The smallest raw-per-hi-res step, so an extreme speed cannot divide by nearly zero.
pub const RAW_PER_HI_RES_MIN: f64 = 0.05;

/// The repeat-acceleration divisor `D`, 1..=7: each quick successive stroke lowers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatGain {
    divisor: u32,
}

impl Default for RepeatGain {
    fn default() -> Self {
        Self {
            divisor: ACCEL_BASE,
        }
    }
}

impl RepeatGain {
    /// The divisor after a new contact set lands at `now`.
    pub fn on_new_stroke(self, last_output: Option<Mono>, now: Mono) -> Self {
        let repeat = last_output.is_some_and(|t| now.float_since(t) < REPEAT_WINDOW.as_secs_f64());
        Self {
            divisor: if repeat {
                self.divisor.saturating_sub(1).max(1)
            } else {
                ACCEL_BASE
            },
        }
    }

    /// `min(max, 7 / D)`: 1.0, 1.167, 1.4, 1.75, 2.3 with the default cap.
    pub fn gain(self, max: f64) -> f64 {
        max.min(f64::from(ACCEL_BASE) / f64::from(self.divisor))
    }
}

/// `1 + min(rate / 2000, max - 1)` for samples at most 50 ms apart, else 1.
pub fn velocity_gain(raw: i32, dt: f64, max: f64) -> f64 {
    if dt <= 0.0 || dt > VELOCITY_DT_MAX {
        return 1.0;
    }
    let raw_per_second = f64::from(raw.unsigned_abs()) / dt;
    let extra = (raw_per_second / VELOCITY_RATE_UNIT).min(max - 1.0);
    1.0 + extra
}

/// Raw units per hi-res wheel unit at this speed and gain.
pub fn raw_per_hi_res(speed: u16, repeat: f64, velocity: f64) -> f64 {
    let num = f64::from((64 - u32::from(speed.min(63))) * ACCEL_BASE);
    let den = f64::from(HI_RES_PER_DETENT) * repeat * velocity;
    (num / den).max(RAW_PER_HI_RES_MIN)
}

/// Px for `raw` units, given the raw-per-hi-res step: a hi-res unit is 1/120 detent.
pub fn px(raw: f64, raw_per_hi_res: f64, detent_px: f64) -> f64 {
    raw / raw_per_hi_res * (detent_px / f64::from(HI_RES_PER_DETENT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_sequence_matches_the_documented_gains() {
        let mut g = RepeatGain::default();
        let mut gains = vec![g.gain(2.3)];
        for i in 1..6u64 {
            g = g.on_new_stroke(
                Some(Mono::from_millis(i * 1000)),
                Mono::from_millis(i * 1000 + 100),
            );
            gains.push(g.gain(2.3));
        }
        let want = [1.0, 7.0 / 6.0, 1.4, 1.75, 2.3, 2.3];
        for (got, want) in gains.iter().zip(want) {
            assert!((got - want).abs() < 1e-9, "{gains:?}");
        }
        let late = g.on_new_stroke(Some(Mono::from_millis(0)), Mono::from_millis(500));
        assert_eq!(late, RepeatGain::default(), "0.5 s is outside the window");
    }

    #[test]
    fn velocity_gain_table() {
        const CASES: &[(i32, f64, f64)] = &[
            (10, 0.0, 1.0),
            (10, 0.06, 1.0),
            (16, 0.008, 2.0),
            (8, 0.008, 1.5),
            (-8, 0.008, 1.5),
            (0, 0.008, 1.0),
        ];
        for &(raw, dt, want) in CASES {
            assert!(
                (velocity_gain(raw, dt, 2.0) - want).abs() < 1e-9,
                "{raw} {dt}"
            );
        }
    }

    #[test]
    fn base_ratio_is_sixty_px_per_294_units() {
        let rpu = raw_per_hi_res(22, 1.0, 1.0);
        assert!((rpu - 294.0 / 120.0).abs() < 1e-12);
        assert!((px(294.0, rpu, 60.0) - 60.0).abs() < 1e-9);
        assert!((px(1.0, rpu, 60.0) - 0.2041).abs() < 1e-4);
        assert_eq!(raw_per_hi_res(63, 2.3, 2.0), RAW_PER_HI_RES_MIN);
    }
}
