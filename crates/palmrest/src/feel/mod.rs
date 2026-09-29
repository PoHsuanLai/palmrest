//! The scroll feel shared by every output: axis lock, gains, px, wheel quantisation.

pub mod curve;
pub mod gain;
pub mod lock;
pub mod lock_event;
pub mod momentum;
pub mod natural;
pub mod quantize;
pub mod release;
pub mod rule;

pub use curve::{CurveParams, MacLike};
pub use gain::{HI_RES_PER_DETENT, RepeatGain};
pub use lock::AxisLock;
pub use lock_event::{Hold, LockEvent, Move, Sample};
pub use momentum::Drag;
pub use natural::{NaturalScroll, offset_delta};
pub use quantize::{Quanta, Quantized, Quantizer, UnitCarry, WheelDelta};
pub use release::{ReleaseRule, Samples as ReleaseSamples};
pub use rule::{Lock, LockParams, Threshold};
