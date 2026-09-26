//! The scroll feel shared by every output: axis lock, gains, px, wheel quantisation.

pub mod gain;
pub mod lock;
pub mod natural;
pub mod quantize;
pub mod rule;

pub use gain::RepeatGain;
pub use lock::{AxisLock, Hold, Lock, LockEvent, LockParams, Move, Sample};
pub use natural::{NaturalScroll, offset_delta};
pub use quantize::{Quanta, Quantized, Quantizer, WheelDelta};

use crate::config::Config;

impl LockParams {
    /// The lock's parameters from the config, with the P5 threshold multiplier applied when
    /// the mouse is moving.
    pub fn from_config(c: &Config, threshold_multiplier: f64) -> Self {
        let s = &c.scroll;
        Self {
            threshold: (f64::from(s.lock_threshold.0) * threshold_multiplier).round() as i32,
            ratio: s.lock_ratio.ratio(),
            timeout: s.lock_timeout_ms.duration(),
            speed: s.speed.0,
            velocity_gain_max: s.velocity_gain_max.ratio(),
            repeat_gain_max: s.repeat_gain_max.ratio(),
            detent_px: f64::from(s.detent_px.0),
        }
    }
}
