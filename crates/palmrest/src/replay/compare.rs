//! `palmrest replay FILE --curve mac-like`: one recording under both acceleration curves, so a
//! recorded session can be judged by how far each curve scrolls it and how hard it accelerates.

use crate::config::{Config, Curve};
use crate::recording::{Record, RecordedSource};
use crate::replay::{Timing, outs, replay_from};
use crate::tracker::Out;
use palmrest_protocol::{Phase, ToHost};

/// What one curve made of a recording (socket px, finger direction).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveRun {
    pub curve: Curve,
    /// Sum of |dx| + |dy| over every scroll message.
    pub cumulative_px: f64,
    /// Signed totals.
    pub net_x: f64,
    pub net_y: f64,
    /// The largest gain over the base ratio of any sample.
    pub peak_gain: f64,
    /// Scroll gestures that locked (Began).
    pub strokes: usize,
}

/// The two runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Comparison {
    pub classic: CurveRun,
    pub mac_like: CurveRun,
}

/// Replays Magic Mouse hidraw `records` under `Classic` and `MacLike`, every other key as in
/// `cfg`.
pub fn compare_curves(records: &[Record], cfg: &Config, timing: Timing) -> Comparison {
    compare_curves_from(&RecordedSource::MagicMouseHid, records, cfg, timing)
}

/// `compare_curves` for records from any source.
pub fn compare_curves_from(
    source: &RecordedSource,
    records: &[Record],
    cfg: &Config,
    timing: Timing,
) -> Comparison {
    let run = |curve| {
        let mut c = cfg.clone();
        c.feel.curve = curve;
        let (tracker, steps) = replay_from(source, records, c, timing);
        summarise(curve, &outs(&steps), tracker.peak_gain())
    };
    Comparison {
        classic: run(Curve::Classic),
        mac_like: run(Curve::MacLike),
    }
}

fn summarise(curve: Curve, outs: &[Out], peak_gain: f64) -> CurveRun {
    let start = CurveRun {
        curve,
        cumulative_px: 0.0,
        net_x: 0.0,
        net_y: 0.0,
        peak_gain,
        strokes: 0,
    };
    outs.iter().fold(start, |r, o| match o {
        Out::Host(ToHost::Scroll {
            phase,
            dx_px,
            dy_px,
            ..
        }) => {
            let (dx, dy) = (f64::from(*dx_px), f64::from(*dy_px));
            CurveRun {
                cumulative_px: r.cumulative_px + dx.abs() + dy.abs(),
                net_x: r.net_x + dx,
                net_y: r.net_y + dy,
                strokes: r.strokes + usize::from(*phase == Phase::Began),
                ..r
            }
        }
        _ => r,
    })
}

impl Comparison {
    /// The table `palmrest replay --curve mac-like` prints.
    pub fn table(&self) -> String {
        let row = |r: &CurveRun| {
            let name = match r.curve {
                Curve::Classic => "classic",
                Curve::MacLike => "mac-like",
            };
            format!(
                "{name:<9} {:>13.1} {:>9.1} {:>9.1} {:>9.3} {:>7}\n",
                r.cumulative_px, r.net_x, r.net_y, r.peak_gain, r.strokes
            )
        };
        let ratio = if self.classic.cumulative_px > 0.0 {
            self.mac_like.cumulative_px / self.classic.cumulative_px
        } else {
            0.0
        };
        format!(
            "curve     cumulative_px     net_x     net_y peak_gain strokes\n{}{}mac-like / classic cumulative px: {ratio:.3}\n",
            row(&self.classic),
            row(&self.mac_like)
        )
    }
}
