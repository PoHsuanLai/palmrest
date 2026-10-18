//! `MagicMouseHid`: the mouse's hidraw reports, decoded by palmrest itself (12.3.1).
//!
//! `decode` turns bytes into `Report`s in the mouse's own units (and `encode` runs it in
//! reverse for fixtures); this sensor lifts each report into a millimetre `Frame` at the
//! resolution the kernel driver advertises, which is exactly the reference lattice, so the
//! core sees the integers the mouse sent.

pub mod decode;
pub mod encode;
pub mod known;
pub mod report;

pub use known::{Bus, KNOWN, KnownDevice, Layout, Model, Support, known};

use super::SensorError;
use super::frame::{Frame, FrameContact, MagicShape, Shape};
use super::surface::{ButtonZones, DeviceClock, Extent, Surface};
use super::units::{MmRange, Resolution, UnitsPerMm};
use crate::touch::RawTouch;
use report::Report;

/// The Magic Mouse 2's resolution (units/mm), as its kernel driver advertises it.
pub const RESOLUTION: Resolution = Resolution {
    x: UnitsPerMm(26.0),
    y: UnitsPerMm(70.0),
};
/// The coordinate ranges of the kernel driver (x -1100..=1258, y -1589..=2047), which the
/// touch blocks share.
pub const X_RANGE: (i32, i32) = (-1100, 1258);
pub const Y_RANGE: (i32, i32) = (-1589, 2047);

/// The Magic Mouse 2's surface: about 90.7 x 51.9 mm at the advertised resolution.
pub fn surface() -> Surface {
    let range = |axis: UnitsPerMm, (lo, hi): (i32, i32)| MmRange {
        min: axis.mm(lo),
        max: axis.mm(hi),
    };
    Surface {
        extent: Extent {
            x: range(RESOLUTION.x, X_RANGE),
            y: range(RESOLUTION.y, Y_RANGE),
        },
        resolution: RESOLUTION,
        buttons: ButtonZones::SplitLeftRight,
        clock: DeviceClock::Ticks18,
    }
}

/// The hidraw sensor for one model of the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicMouseHid {
    model: Model,
}

impl MagicMouseHid {
    pub fn new(model: Model) -> Self {
        Self { model }
    }

    /// Which mouse this decodes for.
    pub fn model(&self) -> Model {
        self.model
    }
}

impl Default for MagicMouseHid {
    fn default() -> Self {
        Self::new(Model::MagicMouse2)
    }
}

impl MagicMouseHid {
    // TODO(untested): every model gets the Magic Mouse 2's surface. The USB-C model is assumed
    // to share it; the Magic Mouse 1's ranges and resolution were never read from a device.
    pub fn surface(&self) -> Surface {
        surface()
    }

    /// One read's bytes to the frames it completes: one or two whole reports.
    pub fn decode(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, SensorError> {
        Ok(decode::decode(bytes)?.iter().map(lift).collect())
    }
}

/// A decoded report as a millimetre frame.
pub fn lift(r: &Report) -> Frame {
    Frame {
        buttons: r.buttons,
        motion: r.motion,
        contacts: r.touches.iter().map(contact).collect(),
        stamp: r.stamp,
    }
}

fn contact(t: &RawTouch) -> FrameContact {
    FrameContact {
        id: t.id,
        at: RESOLUTION.point(t.at.x, t.at.y),
        state: t.state,
        shape: Shape::MagicMouse(MagicShape {
            major: t.major,
            minor: t.minor,
            size: t.size,
            orientation: t.orientation,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensor::lattice::{self, GridExtent, REFERENCE};
    use crate::touch::{Point, TouchId, TouchState};

    #[test]
    fn the_mouse_is_the_reference_lattice() {
        assert_eq!(RESOLUTION, REFERENCE);
        assert_eq!(
            GridExtent::of(&surface()),
            GridExtent {
                x_min: -1100,
                x_max: 1258,
                y_min: -1589,
                y_max: 2047
            }
        );
    }

    #[test]
    fn every_coordinate_lands_back_on_its_integer() {
        for x in X_RANGE.0 - 1000..=X_RANGE.1 + 1000 {
            for y in [Y_RANGE.0, -1, 0, 1, 777, Y_RANGE.1, x] {
                let t = RawTouch {
                    id: TouchId(3),
                    at: Point::new(x, y),
                    major: 1,
                    minor: 2,
                    size: 3,
                    orientation: -4,
                    state: TouchState::Touching,
                };
                assert_eq!(lattice::point(contact(&t).at), t.at, "({x}, {y})");
            }
        }
    }
}
