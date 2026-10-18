//! `EvdevMultitouch`: a mouse's evdev node read through its `ABS_MT_*` slots.
//!
// TODO(untested): this sensor has been written against the kernel's multitouch protocol B and
// tested only with synthetic event sequences. The one device it can be tried on here is the
// Magic Mouse through hid-magicmouse; no other mouse with a kernel multitouch driver has run it.
// Known gaps, each unverified: the kernel applies its fuzz filter to MT positions (the Magic
// Mouse node has fuzz 4), so small motions arrive later than on hidraw; the Magic Mouse node
// carries no contact size, so P1 and P3 do not apply (`Shape::Unmeasured`); after SYN_DROPPED
// the slots are cleared rather than re-read with EVIOCGMTSLOTS; a node opened mid-touch starts
// with empty slots; `motion` is in the device's own counts, so P5's counts/s threshold means
// whatever that device's CPI makes it.
//!
//! Frames end at `SYN_REPORT` and carry its kernel timestamp (the node is switched to
//! `CLOCK_MONOTONIC` when opened). The node is only ever read: never grabbed, never written, so
//! the kernel and libinput keep the pointer and the clicks. Touchpads are not read at all
//! (`daemon::evdev_discover`): they stay with libinput.

mod event;
mod slots;
mod spec;
#[cfg(test)]
mod tests;

pub use event::{EVENT_LEN, TimedEvent, parse};
pub use spec::{AbsAxis, EvdevSpec};

use super::SensorError;
use super::frame::Frame;
use super::surface::Surface;
use slots::Slots;
use thiserror::Error;

/// Why an evdev read or node was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvdevError {
    #[error("read of {len} bytes is not a whole number of input events")]
    Partial { len: usize },
    #[error("the node does not state its position resolution, so nothing is known in mm")]
    NoResolution,
}

/// The evdev sensor: the node's geometry and the slot state between reads.
#[derive(Debug, Clone, PartialEq)]
pub struct EvdevMultitouch {
    spec: EvdevSpec,
    slots: Slots,
}

impl EvdevMultitouch {
    pub fn new(spec: EvdevSpec) -> Self {
        Self {
            spec,
            slots: Slots::new(spec.slots()),
        }
    }

    pub fn spec(&self) -> EvdevSpec {
        self.spec
    }
}

impl EvdevMultitouch {
    pub fn surface(&self) -> Surface {
        self.spec.surface()
    }

    /// One read's bytes to the frames it completes. Frames end at `SYN_REPORT` and may span
    /// reads, so the slot state is carried between calls.
    pub fn decode(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, SensorError> {
        let events = parse(bytes)?;
        let resolution = self.spec.resolution();
        let (slots, frames) = events.iter().fold(
            (self.slots.clone(), Vec::new()),
            |(slots, mut frames), e| {
                let (slots, frame) = slots.apply(e, resolution);
                frames.extend(frame);
                (slots, frames)
            },
        );
        self.slots = slots;
        Ok(frames)
    }
}
