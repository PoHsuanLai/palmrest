//! One touch as the mouse reports it: 12-bit coordinates, a 4-bit id, an MTTouch state.
//!
//! Coordinates follow the kernel's reading (signed 12-bit, y negated), so `+x` is rightwards
//! across the mouse and `+y` toward the user. The Python daemon read them unsigned and fixed
//! wraps afterwards; `wrap_delta` gives the same deltas for both readings, which is what makes
//! the parity fixtures comparable at all.

use serde::{Deserialize, Serialize};

/// The span of a 12-bit coordinate; deltas are taken modulo this.
pub const COORD_SPAN: i32 = 4096;

/// A touch id, 0..=15, stable for the life of one contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TouchId(pub u8);

/// A position on the mouse surface in raw device units (x ~26 u/mm, y ~70.6 u/mm).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// The wrap-safe displacement from `from` to `self`.
    pub fn delta_from(self, from: Point) -> Point {
        Point::new(wrap_delta(self.x, from.x), wrap_delta(self.y, from.y))
    }

    /// Euclidean length, for "moved less than N units" rules.
    pub fn length(self) -> f64 {
        f64::from(self.x).hypot(f64::from(self.y))
    }
}

/// The MTTouch state carried in the high nibble of the touch's last byte (12.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchState {
    NotTracking,
    StartInRange,
    HoverInRange,
    MakeTouch,
    Touching,
    BreakTouch,
    LingerInRange,
    OutOfRange,
    /// Nibbles 8..=15: never observed; carried so a surprise is visible in a recording.
    Other(u8),
}

impl TouchState {
    /// The state for a raw nibble (only the low four bits are read).
    pub fn from_nibble(n: u8) -> Self {
        match n & 0x0f {
            0 => Self::NotTracking,
            1 => Self::StartInRange,
            2 => Self::HoverInRange,
            3 => Self::MakeTouch,
            4 => Self::Touching,
            5 => Self::BreakTouch,
            6 => Self::LingerInRange,
            7 => Self::OutOfRange,
            other => Self::Other(other),
        }
    }

    /// The nibble this state came from.
    pub fn nibble(self) -> u8 {
        match self {
            Self::NotTracking => 0,
            Self::StartInRange => 1,
            Self::HoverInRange => 2,
            Self::MakeTouch => 3,
            Self::Touching => 4,
            Self::BreakTouch => 5,
            Self::LingerInRange => 6,
            Self::OutOfRange => 7,
            Self::Other(n) => n & 0x0f,
        }
    }
}

/// One decoded 8-byte touch block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTouch {
    pub id: TouchId,
    pub at: Point,
    pub major: u8,
    pub minor: u8,
    /// Contact size, 0..=63; the kernel calls 8 and above a firm touch.
    pub size: u8,
    pub orientation: i8,
    pub state: TouchState,
}

/// Sign-extends the low 12 bits of `raw`.
pub fn sign_extend_12(raw: u16) -> i32 {
    let v = i32::from(raw & 0x0fff);
    if v >= 0x800 { v - COORD_SPAN } else { v }
}

/// The shortest signed distance from `old` to `new` on a 12-bit circle.
///
/// For the kernel reading this is plain subtraction (the surface never spans half the circle);
/// for the Python reading it undoes the wrap. Both give the same answer, by construction.
pub fn wrap_delta(new: i32, old: i32) -> i32 {
    let d = new - old;
    if d > COORD_SPAN / 2 {
        d - COORD_SPAN
    } else if d < -COORD_SPAN / 2 {
        d + COORD_SPAN
    } else {
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_extension_table() {
        const CASES: &[(u16, i32)] = &[
            (0x000, 0),
            (0x001, 1),
            (0x7ff, 2047),
            (0x800, -2048),
            (0xfff, -1),
            (0xbb4, -1100),
            (0x4ea, 1258),
            (0xf123, 0x123),
        ];
        for &(raw, want) in CASES {
            assert_eq!(sign_extend_12(raw), want, "raw {raw:#x}");
        }
    }

    #[test]
    fn wrap_delta_table() {
        const CASES: &[(i32, i32, i32)] = &[
            (10, 4, 6),
            (4, 10, -6),
            (2, 4094, 4),
            (4094, 2, -4),
            (2048, 0, 2048),
            (0, 2049, 2047),
            (-5, 5, -10),
        ];
        for &(new, old, want) in CASES {
            assert_eq!(wrap_delta(new, old), want, "{new} - {old}");
        }
    }

    #[test]
    fn unsigned_and_signed_readings_give_the_same_delta() {
        for &(a, b) in &[(0x7f0u16, 0x810u16), (0xff0, 0x010), (0x100, 0x0f0)] {
            let unsigned = wrap_delta(i32::from(b), i32::from(a));
            let signed = wrap_delta(sign_extend_12(b), sign_extend_12(a));
            assert_eq!(unsigned, signed, "{a:#x} -> {b:#x}");
        }
    }

    #[test]
    fn state_nibbles_round_trip() {
        for n in 0..16u8 {
            assert_eq!(TouchState::from_nibble(n).nibble(), n);
        }
        assert_eq!(TouchState::from_nibble(3), TouchState::MakeTouch);
        assert_eq!(TouchState::from_nibble(0x14), TouchState::Touching);
    }
}
