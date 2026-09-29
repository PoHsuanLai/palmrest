//! `Report` back to bytes: the documented layout run in reverse.
//!
//! Fixtures are synthesised with this, so a test can state a stroke in touches and still feed
//! the reader the bytes it will see from the device. Decode-after-encode is the identity for
//! every report the mouse can send, which the round-trip test pins.

use crate::sensor::magic_mouse::decode::{DOUBLE_REPORT, HEADER_LEN, SHORT_LEN, TOUCH_REPORT};
use crate::sensor::magic_mouse::report::{Report, Stamp};
use crate::touch::RawTouch;

/// Encodes a report as the device would send it: header-only (8 bytes) when it has no touches
/// and no timestamp, else 14 + 8n bytes.
pub fn encode(r: &Report) -> Vec<u8> {
    let short = r.touches.is_empty() && r.stamp == Stamp::Absent;
    let mut out = vec![0u8; if short { SHORT_LEN } else { HEADER_LEN }];
    out[0] = TOUCH_REPORT;
    out[1] = r.buttons.0;
    out[2..4].copy_from_slice(&r.motion.dx.to_le_bytes());
    out[4..6].copy_from_slice(&r.motion.dy.to_le_bytes());
    if let Stamp::Ticks(t) = r.stamp {
        let t = t & 0x3ffff;
        out[11] = ((t & 0x3) << 6) as u8;
        out[12] = ((t >> 2) & 0xff) as u8;
        out[13] = ((t >> 10) & 0xff) as u8;
    }
    out.extend(r.touches.iter().flat_map(encode_touch));
    out
}

/// Wraps two encoded reports in an 0xF7 double, the way Bluetooth sometimes delivers them.
pub fn encode_double(first: &[u8], second: &[u8]) -> Vec<u8> {
    let mut out = vec![DOUBLE_REPORT, u8::try_from(first.len()).unwrap_or(u8::MAX)];
    out.extend_from_slice(first);
    out.extend_from_slice(second);
    out
}

/// One 8-byte touch block in the kernel's layout.
pub fn encode_touch(t: &RawTouch) -> [u8; 8] {
    let x = (t.at.x & 0x0fff) as u16;
    let y = ((-t.at.y) & 0x0fff) as u16;
    let id = t.id.0 & 0x0f;
    let orientation = (i16::from(t.orientation) + 32).clamp(0, 63) as u8;
    [
        (x & 0xff) as u8,
        ((x >> 8) as u8 & 0x0f) | (((y & 0x0f) as u8) << 4),
        (y >> 4) as u8,
        t.major,
        t.minor,
        (t.size & 0x3f) | ((id & 0x03) << 6),
        (orientation << 2) | (id >> 2),
        t.state.nibble() << 4,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensor::magic_mouse::decode::decode;
    use crate::sensor::magic_mouse::report::{Buttons, Motion};
    use crate::touch::{Point, TouchId, TouchState};

    fn touch(id: u8, x: i32, y: i32) -> RawTouch {
        RawTouch {
            id: TouchId(id),
            at: Point::new(x, y),
            major: 40,
            minor: 30,
            size: 12,
            orientation: -5,
            state: TouchState::Touching,
        }
    }

    #[test]
    fn decode_inverts_encode() {
        let cases = [
            Report::empty(),
            Report {
                buttons: Buttons::LEFT,
                motion: Motion { dx: -3, dy: 700 },
                stamp: Stamp::Ticks(0x2_abcd),
                touches: vec![],
            },
            Report {
                buttons: Buttons::NONE,
                motion: Motion::default(),
                stamp: Stamp::Ticks(1),
                touches: vec![touch(0, -1100, 2047)],
            },
            Report {
                buttons: Buttons::RIGHT,
                motion: Motion { dx: 1, dy: -1 },
                stamp: Stamp::Ticks(0x3ffff),
                touches: vec![touch(15, 1258, -1589), touch(3, 0, 0), touch(9, -1, 1)],
            },
        ];
        for (i, r) in cases.iter().enumerate() {
            assert_eq!(decode(&encode(r)), Ok(vec![r.clone()]), "case {i}");
        }
    }

    #[test]
    fn double_round_trips_in_order() {
        let a = Report {
            stamp: Stamp::Ticks(5),
            touches: vec![touch(1, 10, 20)],
            ..Report::empty()
        };
        let b = Report {
            stamp: Stamp::Ticks(6),
            touches: vec![touch(1, 12, 20)],
            ..Report::empty()
        };
        let bytes = encode_double(&encode(&a), &encode(&b));
        assert_eq!(decode(&bytes), Ok(vec![a, b]));
    }
}
