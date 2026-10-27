//! The scroll feel shared by every output: axis lock, acceleration, px.

pub mod curve;
pub mod gain;
pub mod lock;
pub mod lock_event;
pub mod natural;
pub mod quantize;
pub mod rule;

pub use gain::HI_RES_PER_DETENT;
pub use lock::AxisLock;
pub use lock_event::{Hold, LockEvent, Move, Sample};
pub use natural::{NaturalScroll, offset_delta};
pub use quantize::{Quantized, Quantizer};
pub use rule::{Lock, LockParams, Threshold};
