//! Bytes from hidraw to `Report`s (12.3.1).
//!
//! Every byte came from a radio, so nothing here panics: a malformed read is a `DecodeError`,
//! counted by the reader and dropped.

use crate::sensor::magic_mouse::report::{Buttons, FrameStamp, Motion, Report};
use crate::touch::{RawTouch, TouchId, TouchState, sign_extend_12};
use thiserror::Error;

/// The multitouch report id.
pub const TOUCH_REPORT: u8 = 0x12;
/// The Bluetooth wrapper that carries two reports in one read.
pub const DOUBLE_REPORT: u8 = 0xf7;
/// Header bytes before the first touch block.
pub const HEADER_LEN: usize = 14;
/// Bytes per touch block.
pub const TOUCH_LEN: usize = 8;
/// The most touches one report can carry.
pub const MAX_TOUCHES: usize = 15;
/// The length of a header-only report.
pub const SHORT_LEN: usize = 8;

/// Why a read produced no reports.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    #[error("empty read")]
    Empty,
    #[error("report 0x12 of length {len} is neither 8 nor 14 + 8n with n <= 15")]
    Length { len: usize },
    #[error("0xf7 double report of length {len} names a first report of {first_len} bytes")]
    Double { len: usize, first_len: usize },
    #[error("report id {id:#04x} is not a touch report")]
    OtherId { id: u8 },
}

/// Decodes one hidraw read: a single 0x12 report, or an 0xF7 pair, in order.
pub fn decode(bytes: &[u8]) -> Result<Vec<Report>, DecodeError> {
    match bytes.first() {
        None => Err(DecodeError::Empty),
        Some(&DOUBLE_REPORT) => decode_double(bytes),
        Some(_) => decode_single(bytes).map(|r| vec![r]),
    }
}

fn decode_double(bytes: &[u8]) -> Result<Vec<Report>, DecodeError> {
    let len = bytes.len();
    let first_len = usize::from(
        *bytes
            .get(1)
            .ok_or(DecodeError::Double { len, first_len: 0 })?,
    );
    let split = 2 + first_len;
    let (Some(first), Some(second)) = (bytes.get(2..split), bytes.get(split..)) else {
        return Err(DecodeError::Double { len, first_len });
    };
    Ok(vec![decode_single(first)?, decode_single(second)?])
}

fn decode_single(bytes: &[u8]) -> Result<Report, DecodeError> {
    match bytes.first() {
        None => Err(DecodeError::Empty),
        Some(&TOUCH_REPORT) => decode_touch_report(bytes),
        Some(&id) => Err(DecodeError::OtherId { id }),
    }
}

fn decode_touch_report(d: &[u8]) -> Result<Report, DecodeError> {
    let len = d.len();
    let count = touch_count(len).ok_or(DecodeError::Length { len })?;
    let buttons = Buttons(d[1]);
    let motion = Motion {
        dx: i16::from_le_bytes([d[2], d[3]]),
        dy: i16::from_le_bytes([d[4], d[5]]),
    };
    let stamp = if len >= HEADER_LEN {
        FrameStamp::Ticks(ticks(d[11], d[12], d[13]))
    } else {
        FrameStamp::Absent
    };
    let touches = (0..count)
        .map(|i| {
            let at = HEADER_LEN + TOUCH_LEN * i;
            decode_touch([
                d[at],
                d[at + 1],
                d[at + 2],
                d[at + 3],
                d[at + 4],
                d[at + 5],
                d[at + 6],
                d[at + 7],
            ])
        })
        .collect();
    Ok(Report {
        buttons,
        motion,
        stamp,
        touches,
    })
}

/// The number of touches a 0x12 report of this length carries, or `None` if the length is
/// not one the mouse sends.
pub fn touch_count(len: usize) -> Option<usize> {
    if len == SHORT_LEN {
        return Some(0);
    }
    let body = len.checked_sub(HEADER_LEN)?;
    (body % TOUCH_LEN == 0 && body / TOUCH_LEN <= MAX_TOUCHES).then_some(body / TOUCH_LEN)
}

/// The 18-bit timestamp from header bytes 11..=13.
pub fn ticks(d11: u8, d12: u8, d13: u8) -> u32 {
    (u32::from(d11) >> 6) | (u32::from(d12) << 2) | (u32::from(d13) << 10)
}

/// One 8-byte touch block, in the kernel's reading.
pub fn decode_touch(t: [u8; 8]) -> RawTouch {
    let x = sign_extend_12(u16::from(t[1] & 0x0f) << 8 | u16::from(t[0]));
    let y = -sign_extend_12(u16::from(t[2]) << 4 | u16::from(t[1] >> 4));
    RawTouch {
        id: TouchId(((t[6] << 2) | (t[5] >> 6)) & 0x0f),
        at: crate::touch::Point::new(x, y),
        major: t[3],
        minor: t[4],
        size: t[5] & 0x3f,
        orientation: i8::try_from(i16::from(t[6] >> 2) - 32).unwrap_or(0), // 0..=63 - 32 always fits
        state: TouchState::from_nibble(t[7] >> 4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_table() {
        const CASES: &[(usize, Option<usize>)] = &[
            (0, None),
            (7, None),
            (8, Some(0)),
            (13, None),
            (14, Some(0)),
            (15, None),
            (22, Some(1)),
            (29, None),
            (30, Some(2)),
            (134, Some(15)),
            (142, None),
        ];
        for &(len, want) in CASES {
            assert_eq!(touch_count(len), want, "len {len}");
        }
    }

    #[test]
    fn ticks_use_eighteen_bits() {
        assert_eq!(ticks(0xff, 0xff, 0xff), (1 << 18) - 1);
        assert_eq!(ticks(0x40, 0, 0), 1);
        assert_eq!(ticks(0, 1, 0), 4);
        assert_eq!(ticks(0, 0, 1), 1024);
    }

    #[test]
    fn other_ids_and_empty_reads_are_errors() {
        assert_eq!(decode(&[]), Err(DecodeError::Empty));
        assert_eq!(
            decode(&[0x10, 0, 0]),
            Err(DecodeError::OtherId { id: 0x10 })
        );
        assert_eq!(
            decode(&[DOUBLE_REPORT]),
            Err(DecodeError::Double {
                len: 1,
                first_len: 0
            })
        );
        assert_eq!(
            decode(&[DOUBLE_REPORT, 20, 0x12]),
            Err(DecodeError::Double {
                len: 3,
                first_len: 20
            })
        );
    }
}
