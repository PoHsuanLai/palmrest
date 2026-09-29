//! The input seam: a `Sensor` turns a device's reads into `Frame`s in millimetres.
//!
//! `Sensor` mirrors `outputs::Emitter` on the other side of the core. Two implementations are
//! swapped at this boundary: `MagicMouseHid` (the mouse's hidraw reports, decoded by palmrest
//! itself; the one the daemon was written for) and `EvdevMultitouch` (the `ABS_MT_*` slots a
//! kernel multitouch driver exposes on an evdev node; untested on anything but the Magic Mouse).
//!
//! Everything downstream of a `Frame` is device-generic: the tracker projects each frame onto
//! the reference lattice (`lattice`) and every threshold is configured in millimetres.

pub mod evdev;
pub mod frame;
pub mod lattice;
pub mod magic_mouse;
pub mod surface;
pub mod units;

pub use evdev::{EvdevError, EvdevMultitouch, EvdevSpec};
pub use frame::{Buttons, Frame, FrameContact, FrameStamp, MagicShape, Motion, Shape};
pub use magic_mouse::{MagicMouseHid, Model};
pub use surface::{ButtonZones, DeviceClock, Extent, SizeMm, Surface};
pub use units::{Mm, MmPoint, MmRange, Resolution, UnitsPerMm};

use crate::sensor::magic_mouse::decode::DecodeError;
use thiserror::Error;

/// A read that produced no frames, and why. The reader counts it and drops the read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SensorError {
    #[error(transparent)]
    Hid(#[from] DecodeError),
    #[error(transparent)]
    Evdev(#[from] EvdevError),
}

/// A touch-sensing pointing device, as the core sees it.
///
/// `decode` takes `&mut self` and may return any number of frames, because the two sources
/// cut their streams differently: one hidraw read is one or two whole reports (an 0xF7 double),
/// while an evdev read is a run of `input_event`s whose frames end at `SYN_REPORT` and may span
/// reads, so the evdev sensor carries slot state between calls. It returns a `Result` rather
/// than a bare `Vec` because a malformed read is a fact the reader logs and counts, not an
/// empty frame list.
pub trait Sensor {
    /// The touch surface: its size, resolution, buttons and clock.
    fn surface(&self) -> Surface;
    /// One read's bytes to the frames it completes, in order.
    fn decode(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, SensorError>;
}
