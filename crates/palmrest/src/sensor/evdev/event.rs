//! `struct input_event` as read from an evdev node.

use super::EvdevError;
use crate::outputs::events::InputEvent;
use crate::time::Mono;

/// The size of `struct input_event` on a 64-bit kernel: `timeval` (two `i64`), type, code,
/// value.
// TODO(untested): 32-bit userspace on a 64-bit-time kernel has a different layout; only the
// 64-bit one is read.
pub const EVENT_LEN: usize = 24;

pub const SYN_DROPPED: u16 = 3;
pub const REL_X: u16 = 0x00;
pub const REL_Y: u16 = 0x01;
pub const BTN_RIGHT: u16 = 0x111;

/// One event with the kernel's timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedEvent {
    pub at: Mono,
    pub event: InputEvent,
}

impl TimedEvent {
    /// The event's 24 bytes, as the kernel writes them.
    pub fn to_bytes(self) -> [u8; EVENT_LEN] {
        let mut b = self.event.to_bytes();
        let sec = i64::try_from(self.at.0 / 1_000_000_000).unwrap_or(i64::MAX);
        let usec = i64::try_from(self.at.0 % 1_000_000_000 / 1000).unwrap_or(0);
        b[..8].copy_from_slice(&sec.to_le_bytes());
        b[8..16].copy_from_slice(&usec.to_le_bytes());
        b
    }
}

/// Splits one read into events. The kernel only ever returns whole events.
pub fn parse(bytes: &[u8]) -> Result<Vec<TimedEvent>, EvdevError> {
    if !bytes.len().is_multiple_of(EVENT_LEN) {
        return Err(EvdevError::Partial { len: bytes.len() });
    }
    Ok(bytes.as_chunks::<EVENT_LEN>().0.iter().map(one).collect())
}

fn one(b: &[u8; EVENT_LEN]) -> TimedEvent {
    let i64_at = |i: usize| {
        let mut a = [0u8; 8];
        a.copy_from_slice(&b[i..i + 8]);
        i64::from_le_bytes(a)
    };
    let (sec, usec) = (i64_at(0), i64_at(8));
    let ns = u64::try_from(sec)
        .unwrap_or(0)
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::try_from(usec).unwrap_or(0).saturating_mul(1000));
    TimedEvent {
        at: Mono(ns),
        event: InputEvent {
            kind: u16::from_le_bytes([b[16], b[17]]),
            code: u16::from_le_bytes([b[18], b[19]]),
            value: i32::from_le_bytes([b[20], b[21], b[22], b[23]]),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_round_trip_and_partial_reads_are_errors() {
        let e = TimedEvent {
            at: Mono(12_345_678_000),
            event: InputEvent {
                kind: 3,
                code: 0x35,
                value: -1100,
            },
        };
        let bytes: Vec<u8> = [e, e].iter().flat_map(|e| e.to_bytes()).collect();
        assert_eq!(parse(&bytes), Ok(vec![e, e]));
        assert_eq!(parse(&bytes[..30]), Err(EvdevError::Partial { len: 30 }));
        assert_eq!(parse(&[]), Ok(vec![]));
    }
}
