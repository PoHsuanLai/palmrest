//! One sensor frame: what was touching the surface at one instant, in millimetres.

use super::units::MmPoint;
use crate::time::Mono;
use crate::touch::{TouchId, TouchState};
use serde::{Deserialize, Serialize};

/// The physical buttons held in a frame. palmrest never makes clicks (the kernel does); it only
/// keeps clicks from being read as taps or scrolls (P4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Buttons(pub u8);

impl Buttons {
    pub const NONE: Buttons = Buttons(0);
    pub const LEFT: Buttons = Buttons(1);
    pub const RIGHT: Buttons = Buttons(2);

    /// Whether any physical button is held in this frame.
    pub fn any(self) -> bool {
        self.0 & 0x03 != 0
    }
}

/// Mouse motion during the frame, in the sensor's counts (P5 only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Motion {
    pub dx: i16,
    pub dy: i16,
}

/// A frame's own time. A frame without one is stamped `None`: the arrival time stands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stamp {
    /// The Magic Mouse's 18-bit counter, of unpublished unit, fitted onto the monotonic clock
    /// (`stamp::StampClock`).
    Ticks(u32),
    /// A kernel timestamp on `CLOCK_MONOTONIC` (evdev's `SYN_REPORT` time).
    Kernel(Mono),
}

/// The Magic Mouse's own description of a contact, in its unpublished units. P1 (light touch)
/// and P3 (palm) read these; no other sensor has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagicShape {
    pub major: u8,
    pub minor: u8,
    /// Contact size, 0..=63; the kernel calls 8 and above a firm touch.
    pub size: u8,
    pub orientation: i8,
}

/// One contact of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameContact {
    /// Stable for the life of the contact.
    pub id: TouchId,
    pub at: MmPoint,
    /// The Magic Mouse's MTTouch state; `Touching` for sensors that only report contacts.
    pub state: TouchState,
    /// `None`: nothing usable, and the contact is treated as a firm, finger-sized touch, so P1
    /// and P3 do not apply to it.
    // TODO(untested): an evdev node whose ABS_MT_TOUCH_MAJOR has a resolution could give P3 a
    // major axis in mm; the Magic Mouse's kernel driver exposes major << 2 with no resolution,
    // so no scale is known and none is guessed.
    pub shape: Option<MagicShape>,
}

/// Everything a sensor reported at one instant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub buttons: Buttons,
    pub motion: Motion,
    pub contacts: Vec<FrameContact>,
    pub stamp: Option<Stamp>,
}
