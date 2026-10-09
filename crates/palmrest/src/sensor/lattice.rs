//! The reference lattice: the integer grid the gesture core computes on.
//!
//! The scroll feel truncates centroids and carries integer remainders in the Magic Mouse 2's
//! own units, so computing it on floats in millimetres would move truncations and break the
//! reference-feel goldens. The core
//! therefore keeps its integer arithmetic, on a grid fixed in millimetres: the Magic Mouse 2's,
//! 26 units/mm across and 70 units/mm along (the resolution its kernel driver advertises). Every
//! sensor's millimetres are projected onto it, so a finger that moves 10 mm scrolls the same on
//! any surface, and the Magic Mouse lands back on exactly the integers it reported.
//!
//! Configured thresholds are micrometres (`config::Um`) and are rounded onto the same grid per
//! axis. A rule that measures a Euclidean distance on the grid is anisotropic in millimetres,
//! as it always was on the mouse; those rules take an x and a y radius (`Ellipse`).

use super::surface::Surface;
use super::units::{Mm, MmPoint, Resolution, UnitsPerMm};
use crate::config::Um;
use crate::touch::Point;

/// The grid: the Magic Mouse 2's resolution, as its kernel driver reports it
/// (`ABS_MT_POSITION_X/Y` resolution 26 and 70, read with EVIOCGABS on the device).
pub const REFERENCE: Resolution = Resolution {
    x: UnitsPerMm(26.0),
    y: UnitsPerMm(70.0),
};

/// Which axis a length is measured along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Along {
    X,
    Y,
}

impl Along {
    fn per_mm(self) -> UnitsPerMm {
        match self {
            Along::X => REFERENCE.x,
            Along::Y => REFERENCE.y,
        }
    }
}

/// A position in millimetres on the grid.
pub fn point(at: MmPoint) -> Point {
    Point::new(REFERENCE.x.units(at.x), REFERENCE.y.units(at.y))
}

/// A configured length on the grid along `axis`, rounded to the nearest unit.
pub fn units(um: Um, axis: Along) -> i32 {
    axis.per_mm().units(Mm(f64::from(um.0) / 1000.0))
}

/// The surface's edges on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridExtent {
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
}

impl GridExtent {
    pub fn of(s: &Surface) -> Self {
        Self {
            x_min: REFERENCE.x.units(s.extent.x.min),
            x_max: REFERENCE.x.units(s.extent.x.max),
            y_min: REFERENCE.y.units(s.extent.y.min),
            y_max: REFERENCE.y.units(s.extent.y.max),
        }
    }
}

/// A distance rule on the grid with a radius per axis: reached when the displacement leaves
/// the ellipse. With equal radii it is exactly the old "Euclidean length >= r" in grid units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ellipse {
    pub x: i32,
    pub y: i32,
}

impl Ellipse {
    /// The ellipse of two configured radii.
    pub fn of(x: Um, y: Um) -> Self {
        Self {
            x: units(x, Along::X),
            y: units(y, Along::Y),
        }
    }

    /// The displacement's length, scaled so the ellipse is the circle of the larger radius.
    /// The scale factors are exactly 1.0 when the radii agree, so the default rules compute the
    /// same float as before.
    fn scaled(self, d: Point) -> (f64, f64) {
        let (rx, ry) = (f64::from(self.x.max(1)), f64::from(self.y.max(1)));
        let r = rx.max(ry);
        let len = (f64::from(d.x) * (r / rx)).hypot(f64::from(d.y) * (r / ry));
        (len, r)
    }

    /// Whether `d` is on or outside the ellipse.
    pub fn reached(self, d: Point) -> bool {
        let (len, r) = self.scaled(d);
        len >= r
    }

    /// Whether `d` is on or inside the ellipse.
    pub fn contains(self, d: Point) -> bool {
        let (len, r) = self.scaled(d);
        len <= r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn micrometres_round_trip_on_both_axes() {
        for axis in [Along::X, Along::Y] {
            for u in -5000..=5000 {
                let um = Um((axis.per_mm().mm(u).0 * 1000.0).round() as i32);
                assert_eq!(units(um, axis), u, "{u} {axis:?}");
            }
        }
    }

    #[test]
    fn reach_and_containment_table() {
        // (name, ellipse half-axes, offset, reached, contained when the row says so)
        type Case = (&'static str, (i32, i32), (i32, i32), bool, Option<bool>);
        const CASES: &[Case] = &[
            // A circle is the old Euclidean rule.
            (
                "circle 18,24 on the rim",
                (30, 30),
                (18, 24),
                true,
                Some(true),
            ),
            (
                "circle 30,0 on the rim",
                (30, 30),
                (30, 0),
                true,
                Some(true),
            ),
            (
                "circle 0,-30 on the rim",
                (30, 30),
                (0, -30),
                true,
                Some(true),
            ),
            ("circle 21,21 inside", (30, 30), (21, 21), false, Some(true)),
            ("circle 29,1 inside", (30, 30), (29, 1), false, Some(true)),
            ("circle 17,24 inside", (30, 30), (17, 24), false, Some(true)),
            // An ellipse stretches the short axis.
            ("ellipse x on the rim", (60, 30), (60, 0), true, None),
            ("ellipse x just short", (60, 30), (59, 0), false, None),
            ("ellipse y on the rim", (60, 30), (0, 30), true, None),
            ("ellipse y just short", (60, 30), (0, 29), false, None),
        ];
        for &(name, (x, y), (dx, dy), reached, contained) in CASES {
            let e = Ellipse { x, y };
            let d = Point::new(dx, dy);
            assert_eq!(e.reached(d), reached, "{name}: reached");
            if let Some(contained) = contained {
                assert_eq!(e.contains(d), contained, "{name}: contained");
            }
        }
    }
}
