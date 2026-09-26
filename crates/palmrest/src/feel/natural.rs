//! Natural scrolling: from finger direction to a content offset change (11.3.13).
//!
//! palmrest never applies this to its own streams; they stay in finger direction and the host
//! applies the user's choice. It lives here so the host and the tests share one definition.

use serde::{Deserialize, Serialize};

/// The user's scroll direction choice (`scroll.natural`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NaturalScroll {
    /// Content follows the finger.
    #[default]
    Natural,
    Traditional,
}

/// The scroll offset change for a finger-direction delta: with `Natural`, a finger moving
/// toward the user (+y) moves the content down, so the offset decreases.
pub fn offset_delta(dx_px: f64, dy_px: f64, n: NaturalScroll) -> (f64, f64) {
    match n {
        NaturalScroll::Natural => (-dx_px, -dy_px),
        NaturalScroll::Traditional => (dx_px, dy_px),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_follows_the_finger() {
        assert_eq!(
            offset_delta(0.0, 10.0, NaturalScroll::Natural),
            (-0.0, -10.0)
        );
        assert_eq!(
            offset_delta(3.0, 10.0, NaturalScroll::Traditional),
            (3.0, 10.0)
        );
    }
}
