//! What palmrest needs to know about an evdev node, read once when it is opened.

use super::EvdevError;
use crate::sensor::surface::{ButtonZones, DeviceClock, Extent, Surface};
use crate::sensor::units::{MmRange, Resolution, UnitsPerMm};
use serde::{Deserialize, Serialize};

/// One absolute axis as `EVIOCGABS` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbsAxis {
    pub min: i32,
    pub max: i32,
    /// Units per millimetre; 0 when the driver does not say.
    pub resolution: i32,
}

/// A multitouch evdev node's geometry: its two position axes and its slot count. Built only
/// through `new`, so a spec always has a usable resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvdevSpec {
    x: AbsAxis,
    y: AbsAxis,
    slots: u16,
}

impl EvdevSpec {
    /// A spec from the node's `ABS_MT_POSITION_X/Y` and `ABS_MT_SLOT` maximum.
    ///
    /// A node that does not state its resolution is refused: without it nothing can be said in
    /// millimetres, and guessing one would scale every threshold by the guess.
    pub fn new(x: AbsAxis, y: AbsAxis, slot_max: i32) -> Result<Self, EvdevError> {
        if x.resolution <= 0 || y.resolution <= 0 {
            return Err(EvdevError::NoResolution);
        }
        let slots = u16::try_from(slot_max.saturating_add(1).clamp(1, 64)).unwrap_or(1);
        Ok(Self { x, y, slots })
    }

    /// How many slots the node has.
    pub fn slots(&self) -> usize {
        usize::from(self.slots)
    }

    pub fn resolution(&self) -> Resolution {
        Resolution {
            x: UnitsPerMm(f64::from(self.x.resolution)),
            y: UnitsPerMm(f64::from(self.y.resolution)),
        }
    }

    /// The surface this node describes.
    pub fn surface(&self) -> Surface {
        let res = self.resolution();
        let range = |per: UnitsPerMm, a: AbsAxis| MmRange {
            min: per.mm(a.min),
            max: per.mm(a.max),
        };
        Surface {
            extent: Extent {
                x: range(res.x, self.x),
                y: range(res.y, self.y),
            },
            resolution: res,
            buttons: ButtonZones::Reported,
            clock: DeviceClock::Kernel,
        }
    }
}
