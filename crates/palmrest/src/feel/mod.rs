//! The scroll feel shared by every output: axis lock, gains, px, wheel quantisation.

pub mod curve;
pub mod gain;
pub mod lock;
pub mod momentum;
pub mod natural;
pub mod quantize;
pub mod release;
pub mod rule;

pub use curve::{CurveParams, MacLike};
pub use gain::{HI_RES_PER_DETENT, RepeatGain};
pub use lock::{AxisLock, Hold, Lock, LockEvent, LockParams, Move, Sample, Threshold};
pub use momentum::Drag;
pub use natural::{NaturalScroll, offset_delta};
pub use quantize::{Quanta, Quantized, Quantizer, UnitCarry, WheelDelta};
pub use release::{ReleaseRule, Samples as ReleaseSamples};

use crate::config::{Config, Um};
use crate::sensor::lattice::{Along, units};

impl LockParams {
    /// The lock's parameters from the config, with the P5 threshold multiplier applied when
    /// the mouse is moving.
    pub fn from_config(c: &Config, threshold_multiplier: f64) -> Self {
        let s = &c.scroll;
        Self {
            threshold: Threshold {
                x: scaled(s.lock_threshold_x_um, Along::X, threshold_multiplier),
                y: scaled(s.lock_threshold_y_um, Along::Y, threshold_multiplier),
            },
            ratio: s.lock_ratio.ratio(),
            timeout: s.lock_timeout_ms.duration(),
            speed: s.speed.0,
            velocity_gain_max: s.velocity_gain_max.ratio(),
            repeat_gain_max: s.repeat_gain_max.ratio(),
            detent_px: f64::from(s.detent_px.0),
            curve: CurveParams::from_settings(&c.feel),
        }
    }
}

/// A threshold on the grid times the P5 multiplier, rounded as the reference feel rounds it.
fn scaled(um: Um, axis: Along, multiplier: f64) -> i32 {
    (f64::from(units(um, axis)) * multiplier).round() as i32
}
