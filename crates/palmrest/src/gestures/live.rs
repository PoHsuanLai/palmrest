//! The live-tracking model for a swipe with content following the fingers (12.3.7, 12.5).
//!
//! Pure formulas, for the host's page swipe (12.3.6) and the later live workspace mode. Units
//! are pages: progress `p = sum(dx) / D`, velocity in pages per second.

use crate::config::GestureSettings;
use crate::sensor::lattice::{Along, units};
use std::time::Duration;

/// The base finish velocity (R10: 0.002/ms).
pub const BASE_FINISH_VELOCITY: f64 = 2.0;
/// The finish animation is this many times the time the velocity would take.
pub const FINISH_FACTOR: f64 = 3.0;

/// Where the gesture settles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settle {
    /// Move to the next page in the progress direction.
    Commit,
    /// Snap back.
    Cancel,
}

/// Progress in pages from raw travel.
pub fn progress(sum_dx_units: f64, d_units: f64) -> f64 {
    if d_units == 0.0 {
        0.0
    } else {
        sum_dx_units / d_units
    }
}

/// The displayed progress past a bound: dampened by `scale` and capped at `cap` past it
/// (0.5 past -> 0.125 displayed with the defaults).
pub fn displayed(p: f64, lo: f64, hi: f64, scale: f64, cap: f64) -> f64 {
    let past = |bound: f64, over: f64| bound + over.signum() * (over.abs() * scale).min(cap);
    if p > hi {
        past(hi, p - hi)
    } else if p < lo {
        past(lo, p - lo)
    } else {
        p
    }
}

/// Least-squares slope of `(seconds, pages)` samples over the last `window`, pages/s.
pub fn velocity(samples: &[(f64, f64)], window: Duration) -> f64 {
    let Some(&(t_end, _)) = samples.last() else {
        return 0.0;
    };
    let recent: Vec<(f64, f64)> = samples
        .iter()
        .copied()
        .filter(|(t, _)| t_end - t <= window.as_secs_f64())
        .collect();
    let n = recent.len() as f64;
    if recent.len() < 2 {
        return 0.0;
    }
    let mt = recent.iter().map(|s| s.0).sum::<f64>() / n;
    let mp = recent.iter().map(|s| s.1).sum::<f64>() / n;
    let stt: f64 = recent.iter().map(|s| (s.0 - mt).powi(2)).sum();
    if stt == 0.0 {
        return 0.0;
    }
    recent.iter().map(|s| (s.0 - mt) * (s.1 - mp)).sum::<f64>() / stt
}

/// Commit or cancel at lift: past half a page commits unless flicked back; short of it commits
/// only on a flick forward.
pub fn settle(p: f64, v: f64, commit_velocity: f64) -> Settle {
    let toward = v * p.signum();
    let commit = if p.abs() >= 0.5 {
        toward > -commit_velocity
    } else {
        toward >= commit_velocity
    };
    if commit && p != 0.0 {
        Settle::Commit
    } else {
        Settle::Cancel
    }
}

/// The finish animation's duration (ease-out-cubic), clamped to `min..=max`.
pub fn finish_duration(dp: f64, v: f64, min: Duration, max: Duration) -> Duration {
    let secs = dp.abs() / v.abs().max(BASE_FINISH_VELOCITY) * FINISH_FACTOR;
    Duration::from_secs_f64(secs.clamp(min.as_secs_f64(), max.as_secs_f64()))
}

/// The settings-derived constants of the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveParams {
    pub d_units: f64,
    pub overshoot_scale: f64,
    pub overshoot_cap: f64,
    pub velocity_window: Duration,
    pub commit_velocity: f64,
    pub finish_min: Duration,
    pub finish_max: Duration,
}

impl LiveParams {
    /// The live workspace model's parameters.
    pub fn workspace(g: &GestureSettings) -> Self {
        Self {
            d_units: f64::from(units(g.live_workspace_d_um, Along::X)),
            overshoot_scale: g.live_workspace_overshoot_scale.ratio(),
            overshoot_cap: g.live_workspace_overshoot_cap.ratio(),
            velocity_window: g.live_workspace_velocity_window_ms.duration(),
            commit_velocity: g.live_workspace_commit_velocity.ratio(),
            finish_min: g.live_workspace_finish_min_ms.duration(),
            finish_max: g.live_workspace_finish_max_ms.duration(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overshoot_half_a_page_shows_an_eighth() {
        let p = LiveParams::workspace(&GestureSettings::default());
        let d = |x| displayed(x, -1.0, 1.0, p.overshoot_scale, p.overshoot_cap);
        assert!((d(1.5) - 1.125).abs() < 1e-12);
        assert!((d(-1.5) + 1.125).abs() < 1e-12);
        assert!((d(3.0) - 1.125).abs() < 1e-12, "capped");
        assert_eq!(d(0.3), 0.3);
        assert_eq!(progress(500.0, p.d_units), 0.5);
    }

    #[test]
    fn settle_table() {
        const CASES: &[(f64, f64, Settle)] = &[
            (0.6, 0.0, Settle::Commit),
            (0.6, -1.4, Settle::Commit),
            (0.6, -1.5, Settle::Cancel),
            (0.3, 1.5, Settle::Commit),
            (0.3, 1.4, Settle::Cancel),
            (-0.3, -1.6, Settle::Commit),
            (0.0, 3.0, Settle::Cancel),
        ];
        for &(p, v, want) in CASES {
            assert_eq!(settle(p, v, 1.5), want, "p {p} v {v}");
        }
    }

    #[test]
    fn velocity_is_the_slope_over_the_window() {
        let samples: Vec<(f64, f64)> = (0..20)
            .map(|i| (f64::from(i) * 0.01, f64::from(i) * 0.02))
            .collect();
        assert!((velocity(&samples, Duration::from_millis(80)) - 2.0).abs() < 1e-9);
        assert_eq!(velocity(&samples[..1], Duration::from_millis(80)), 0.0);
    }

    #[test]
    fn finish_duration_is_clamped() {
        let (lo, hi) = (Duration::from_millis(100), Duration::from_millis(400));
        assert_eq!(
            finish_duration(0.5, 0.0, lo, hi),
            Duration::from_millis(400)
        );
        assert_eq!(
            finish_duration(0.05, 10.0, lo, hi),
            Duration::from_millis(100)
        );
        let mid = finish_duration(0.2, 3.0, lo, hi).as_secs_f64();
        assert!((mid - 0.2).abs() < 1e-9);
    }
}
