//! P2's bands on the grid: the two sides and the rear (the +y end, toward the palm).

use crate::config::{Percent, Um};
use crate::sensor::Surface;
use crate::sensor::lattice::{Along, GridExtent, units};
use crate::touch::Point;

/// The width of the left band, from the surface's left edge.
const SIDE_BAND_LEFT: Um = Um(7692);
/// The width of the right band, from the surface's right edge.
const SIDE_BAND_RIGHT: Um = Um(7615);
/// The share of the surface's length, from the back, that is the rear band.
const REAR_BAND: Percent = Percent(15);

/// Where a landing contact is watched instead of counted, for one surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bands {
    /// Left of this is the left band.
    left_below: i32,
    /// Right of this is the right band.
    right_above: i32,
    /// Beyond this (toward the user) is the rear band.
    rear_from: f64,
}

impl Bands {
    /// The bands of `surface`: fixed widths from its side edges, a share of its length at
    /// the rear.
    pub fn resolve(surface: &Surface) -> Self {
        let g = GridExtent::of(surface);
        Self {
            left_below: g.x_min + units(SIDE_BAND_LEFT, Along::X),
            right_above: g.x_max - units(SIDE_BAND_RIGHT, Along::X),
            rear_from: f64::from(g.y_max) - REAR_BAND.share() * f64::from(g.y_max - g.y_min),
        }
    }

    /// Whether `at` is in a band.
    pub fn contains(&self, at: Point) -> bool {
        at.x < self.left_below || at.x > self.right_above || f64::from(at.y) > self.rear_from
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensor::magic_mouse;

    #[test]
    fn band_edges_on_the_magic_mouse() {
        let b = Bands::resolve(&magic_mouse::surface());
        const CASES: &[(i32, i32, bool)] = &[
            (-900, 0, false),
            (-901, 0, true),
            (1060, 0, false),
            (1061, 0, true),
            (0, 1501, false),
            (0, 1502, true),
            (0, -1589, false),
        ];
        for &(x, y, want) in CASES {
            assert_eq!(b.contains(Point::new(x, y)), want, "({x}, {y})");
        }
    }
}
