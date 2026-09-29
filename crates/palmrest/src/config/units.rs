//! The unit newtypes every settings key is expressed in (22-SETTINGS §4).
//!
//! Integers only, so a settings file compares exactly and a ratio like 1.5 is `Fraction(1500)`.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Px(pub u16);

/// Milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ms(pub u16);

/// A percentage, 0..=100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Percent(pub u8);

/// A ratio in per-mille: 1000 is 1.0. Gains exceed 1.0, so it is not clamped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fraction(pub u16);

/// A plain quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Count(pub u16);

/// Micrometres on the touch surface: every length and position threshold.
/// Integers, like every other key, so a file compares exactly; the gesture core rounds them
/// onto its grid per axis (`sensor::lattice`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Um(pub i32);

/// Signed raw Magic Mouse 2 report units: the pre-millimetre keys (read, converted and
/// reported as deprecated) and the Magic Mouse's own contact size and major axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Units(pub i32);

impl Ms {
    pub fn duration(self) -> Duration {
        Duration::from_millis(u64::from(self.0))
    }
}

impl Fraction {
    /// The ratio as a float; 1500 is exactly 1.5.
    pub fn ratio(self) -> f64 {
        f64::from(self.0) / 1000.0
    }
}

impl Percent {
    pub fn share(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}
