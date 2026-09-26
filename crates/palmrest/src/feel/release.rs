//! Release velocity (design/11 §11.5.2): the least-squares slope of cumulative px over the
//! samples of the last `window` before the fingers lift, zero when the finger paused first.

use crate::config::MomentumSettings;
use crate::time::Mono;
use std::time::Duration;

/// The release rule's constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleaseRule {
    pub window: Duration,
    pub held_still: Duration,
    pub min_samples: usize,
    /// Below this |v0| there is no glide.
    pub start: f64,
    /// |v0| is clamped to this.
    pub cap: f64,
}

impl ReleaseRule {
    pub fn from_settings(m: &MomentumSettings) -> Self {
        Self {
            window: m.window_ms.duration(),
            held_still: m.held_still_ms.duration(),
            min_samples: usize::from(m.min_samples.0.max(2)),
            start: f64::from(m.start_px_s.0),
            cap: f64::from(m.cap_px_s.0),
        }
    }
}

/// The recent `(time, cumulative px)` samples of one stroke on its locked axis.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Samples {
    points: Vec<(Mono, f64)>,
    total: f64,
}

impl Samples {
    /// Adds `px` at `at`, keeping only what `rule`'s window can still use.
    pub fn push(self, at: Mono, px: f64, rule: &ReleaseRule) -> Self {
        let total = self.total + px;
        let keep = rule.window.max(rule.held_still) * 2;
        let points = self
            .points
            .into_iter()
            .filter(|(t, _)| at.since(*t) <= keep)
            .chain([(at, total)])
            .collect();
        Self { points, total }
    }

    /// The signed release velocity in px/s for a lift at `end`: 0 with too few samples, after
    /// a pause, or below the start speed; clamped to the cap.
    pub fn velocity(&self, end: Mono, rule: &ReleaseRule) -> f64 {
        let Some(&(newest, _)) = self.points.last() else {
            return 0.0;
        };
        if end.since(newest) > rule.held_still {
            return 0.0;
        }
        let recent: Vec<(f64, f64)> = self
            .points
            .iter()
            .filter(|(t, _)| end.since(*t) <= rule.window)
            .map(|&(t, x)| (t.float_since(end), x))
            .collect();
        if recent.len() < rule.min_samples {
            return 0.0;
        }
        let v = slope(&recent);
        if v.abs() < rule.start {
            0.0
        } else {
            v.clamp(-rule.cap, rule.cap)
        }
    }
}

fn slope(points: &[(f64, f64)]) -> f64 {
    let n = points.len() as f64;
    let tm = points.iter().map(|p| p.0).sum::<f64>() / n;
    let xm = points.iter().map(|p| p.1).sum::<f64>() / n;
    let (num, den) = points.iter().fold((0.0, 0.0), |(num, den), &(t, x)| {
        (num + (t - tm) * (x - xm), den + (t - tm) * (t - tm))
    });
    if den <= f64::EPSILON { 0.0 } else { num / den }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn rule() -> ReleaseRule {
        ReleaseRule::from_settings(&Config::default().momentum)
    }

    fn steady(px_per_ms: f64, from_ms: u64, n: u64, every: u64) -> Samples {
        (0..n).fold(Samples::default(), |s, i| {
            s.push(
                Mono::from_millis(from_ms + i * every),
                px_per_ms * every as f64,
                &rule(),
            )
        })
    }

    #[test]
    fn a_steady_stroke_releases_at_its_speed() {
        let s = steady(-2.0, 0, 20, 10);
        let v = s.velocity(Mono::from_millis(195), &rule());
        assert!((v + 2000.0).abs() < 1e-6, "{v}");
    }

    #[test]
    fn the_rules_that_give_no_glide() {
        let r = rule();
        let s = steady(2.0, 0, 20, 10);
        assert_eq!(s.velocity(Mono::from_millis(251), &r), 0.0, "held still");
        assert_eq!(
            steady(2.0, 0, 2, 10).velocity(Mono::from_millis(15), &r),
            0.0
        );
        let slow = steady(0.05, 0, 20, 10);
        assert_eq!(slow.velocity(Mono::from_millis(195), &r), 0.0, "50 px/s");
        let fast = steady(20.0, 0, 20, 10);
        assert_eq!(
            fast.velocity(Mono::from_millis(195), &r),
            12_000.0,
            "capped"
        );
        assert_eq!(Samples::default().velocity(Mono(0), &r), 0.0);
    }
}
