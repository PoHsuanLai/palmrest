//! What a sensor says about its touch surface.

use super::units::{Mm, MmRange, Resolution};
use serde::{Deserialize, Serialize};

/// The touch surface: where its edges are and how fine its grid is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    pub extent: Extent,
    pub resolution: Resolution,
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
