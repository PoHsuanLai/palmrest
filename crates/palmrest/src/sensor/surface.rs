//! What a sensor says about its touch surface.

use super::units::{Mm, MmRange, Resolution};
use serde::{Deserialize, Serialize};

/// The touch surface: where its edges are, how fine its grid is, how it clicks, and whether its
/// frames carry a clock of their own.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    pub extent: Extent,
    pub resolution: Resolution,
    pub buttons: ButtonZones,
    pub clock: DeviceClock,
}

/// The edges of the surface in the sensor's own millimetre frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Extent {
    pub x: MmRange,
    pub y: MmRange,
}

/// The physical size of the surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SizeMm {
    pub width: Mm,
    pub height: Mm,
}

impl Surface {
    /// Width and height, edge to edge.
    pub fn size(&self) -> SizeMm {
        SizeMm {
            width: self.extent.x.length(),
            height: self.extent.y.length(),
        }
    }
}

/// How the device's buttons relate to the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonZones {
    /// One clicking surface whose left and right halves are two buttons; the device itself
    /// reports which one was pressed (the Magic Mouse).
    SplitLeftRight,
    /// Buttons as the kernel reports them (`BTN_LEFT`, `BTN_RIGHT`); where they are is unknown.
    Reported,
}

/// Where a frame's time comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceClock {
    /// An 18-bit device counter of unpublished unit, fitted onto the monotonic clock
    /// (`stamp::StampClock`).
    Ticks18,
    /// The kernel stamped each frame on `CLOCK_MONOTONIC` (evdev's `SYN_REPORT` time).
    Kernel,
    /// No clock: the arrival time is the event time.
    ArrivalOnly,
}
