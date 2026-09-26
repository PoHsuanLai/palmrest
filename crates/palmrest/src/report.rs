//! The decoded form of one 0x12 touch report.

use crate::touch::RawTouch;
use serde::{Deserialize, Serialize};

/// A physical button bit of the report header. The kernel turns these into clicks; palmrest
/// only uses them to keep clicks from being read as taps or scrolls (P4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Buttons(pub u8);

impl Buttons {
    pub const NONE: Buttons = Buttons(0);
    pub const LEFT: Buttons = Buttons(1);
    pub const RIGHT: Buttons = Buttons(2);

    /// Whether any physical button is held in this report.
    pub fn any(self) -> bool {
        self.0 & 0x03 != 0
    }
}

/// Mouse motion of the report, in the sensor's counts (P5 only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Motion {
    pub dx: i16,
    pub dy: i16,
}

/// The 18-bit device timestamp, when the report is long enough to carry one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stamp {
    Ticks(u32),
    Absent,
}

/// One touch report: header fields plus zero to fifteen touches, in report order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub buttons: Buttons,
    pub motion: Motion,
    pub stamp: Stamp,
    pub touches: Vec<RawTouch>,
}

impl Report {
    /// A report with no touches and nothing pressed: what the mouse sends when the last finger
    /// lifts.
    pub fn empty() -> Self {
        Self {
            buttons: Buttons::NONE,
            motion: Motion::default(),
            stamp: Stamp::Absent,
            touches: Vec::new(),
        }
    }
}
