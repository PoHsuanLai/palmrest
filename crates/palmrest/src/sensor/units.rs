//! Millimetres on a touch surface, and the resolutions that turn device units into them.

use serde::{Deserialize, Serialize};

/// A length or position on the touch surface, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Mm(pub f64);

/// A position on the touch surface: `+x` rightwards across the device, `+y` toward the user.
/// The origin is the sensor's own; `Surface::extent` says where the edges are.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MmPoint {
    pub x: Mm,
    pub y: Mm,
}

/// Device units per millimetre along one axis.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnitsPerMm(pub f64);

impl UnitsPerMm {
    /// `units` of this axis in millimetres.
    pub fn mm(self, units: i32) -> Mm {
        Mm(f64::from(units) / self.0)
    }

    /// `mm` in whole units of this axis, rounded to the nearest.
    ///
    /// For any integer `u` well inside `i32`, `units(mm(u)) == u` exactly: the round trip is off
    /// by far less than half a unit, so the rounding restores it. That is what keeps the
    /// Magic Mouse bit for bit through the millimetre frame.
    pub fn units(self, mm: Mm) -> i32 {
        (mm.0 * self.0).round() as i32
    }
}

/// Units per millimetre on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Resolution {
    pub x: UnitsPerMm,
    pub y: UnitsPerMm,
}

impl Resolution {
    /// A position given in device units, in millimetres.
    pub fn point(self, x: i32, y: i32) -> MmPoint {
        MmPoint {
            x: self.x.mm(x),
            y: self.y.mm(y),
        }
    }
}

/// The span of one axis, edge to edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MmRange {
    pub min: Mm,
    pub max: Mm,
}

impl MmRange {
    /// The distance from edge to edge.
    pub fn length(self) -> Mm {
        Mm(self.max.0 - self.min.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_round_trip_through_millimetres() {
        for res in [26.0, 70.0, 26.038, 70.574, 40.0, 1.0, 305.0] {
            let r = UnitsPerMm(res);
            for u in (-5000..=5000)
                .step_by(7)
                .chain([i32::from(i16::MIN), 65535])
            {
                assert_eq!(r.units(r.mm(u)), u, "{u} at {res} u/mm");
            }
        }
    }
}
