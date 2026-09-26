//! The decoded form of one 0x12 touch report: the Magic Mouse's own units, before
//! `MagicMouseHid` lifts it into a millimetre `Frame`.

pub use crate::sensor::frame::{Buttons, Motion};
use crate::touch::RawTouch;
use serde::{Deserialize, Serialize};

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
