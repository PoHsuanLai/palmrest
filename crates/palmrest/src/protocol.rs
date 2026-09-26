//! The socket protocol between palmrest and shell-host (12.3.10, O1).
//!
//! `$XDG_RUNTIME_DIR/palmrest/events.sock`, `SOCK_SEQPACKET`: one JSON message per packet, so
//! there is no framing to get wrong and a reader never sees half a message. The first packet
//! on every connection is `Hello`, whose `version` a client checks before reading anything
//! else; an incompatible change bumps `PROTOCOL_VERSION`.
//!
//! Scroll deltas are `f32` px in **finger direction** with every gain applied; the host applies
//! natural scrolling (11.3.13). These are the only floats in palmrest's public types, because
//! a sub-pixel delta is the whole point of the stream.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The protocol version carried by `Hello`.
pub const PROTOCOL_VERSION: u16 = 1;
/// No message is larger than this; a longer packet is not ours.
pub const MAX_PACKET: usize = 4096;

/// One gesture from MayBegin to Ended or Cancelled; every message of it carries the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GestureId(pub u32);

/// Where a gesture is in its life (12.3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    MayBegin,
    Began,
    Changed,
    Ended,
    Cancelled,
}

/// The axis a scroll is locked to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
}

/// Which double-tap fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapKind {
    One,
    Two,
}

/// Fingers on the mouse or not, for the host's double-scroll window (11.3.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveTouch {
    Down,
    Up,
}

/// Whether the pointer is over one of the subscriber's surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Over {
    Ours,
    #[default]
    NotOurs,
}

/// The mouse palmrest is reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub path: String,
    pub vendor: u16,
    pub product: u16,
}

/// A kind of message a subscriber may ask for. `Hello` and `DeviceGone` always arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    Scroll,
    Swipe,
    Tap,
    Touches,
}

/// What a subscriber receives.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Filter {
    #[default]
    All,
    Only(Vec<Topic>),
}

impl Filter {
    /// Whether `msg` passes this filter.
    pub fn admits(&self, msg: &ToHost) -> bool {
        match (self, msg.topic()) {
            (_, None) | (Filter::All, _) => true,
            (Filter::Only(topics), Some(t)) => topics.contains(&t),
        }
    }
}

/// palmrest to a host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum ToHost {
    /// First on every connection, and again after the mouse reconnects.
    Hello { version: u16, device: DeviceInfo },
    /// One- or two-finger scroll (G1, G2): px in finger direction, gains applied.
    Scroll {
        gesture: GestureId,
        phase: Phase,
        dx_px: f32,
        dy_px: f32,
        fingers: u8,
        axis: Option<Axis>,
        t_ns: u64,
    },
    /// Two-finger horizontal travel in raw units, for a host running the live model (12.3.7).
    Swipe {
        gesture: GestureId,
        phase: Phase,
        fingers: u8,
        dx_units: f32,
        t_ns: u64,
    },
    /// A double-tap fired while this host held the pointer.
    Tap { kind: TapKind, t_ns: u64 },
    /// The first finger landed or the last one lifted.
    Touches { active: ActiveTouch },
    /// The mouse went away; every running gesture was Cancelled first.
    DeviceGone,
}

impl ToHost {
    /// The topic a subscriber filters this message by; `None` for always-delivered messages.
    pub fn topic(&self) -> Option<Topic> {
        match self {
            Self::Hello { .. } | Self::DeviceGone => None,
            Self::Scroll { .. } => Some(Topic::Scroll),
            Self::Swipe { .. } => Some(Topic::Swipe),
            Self::Tap { .. } => Some(Topic::Tap),
            Self::Touches { .. } => Some(Topic::Touches),
        }
    }
}

/// A host to palmrest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum ToDaemon {
    /// Sent on `wl_pointer.enter`/`leave` of the host's surfaces (11.3.14).
    PointerOver { over: Over },
    /// Narrows what this connection receives; the default is everything.
    Subscribe { filter: Filter },
}

/// A packet that is not a message of this protocol.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("packet of {0} bytes exceeds {MAX_PACKET}")]
    TooLong(usize),
    #[error("malformed message: {0}")]
    Malformed(#[from] serde_json::Error),
}

/// One packet's bytes.
pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    serde_json::to_vec(msg).unwrap_or_default() // plain data with string keys; cannot fail
}

/// A message from one packet.
pub fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, ProtocolError> {
    if bytes.len() > MAX_PACKET {
        return Err(ProtocolError::TooLong(bytes.len()));
    }
    Ok(serde_json::from_slice(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<ToHost> {
        vec![
            ToHost::Hello {
                version: PROTOCOL_VERSION,
                device: DeviceInfo {
                    name: "mouse".into(),
                    path: "/dev/hidraw3".into(),
                    vendor: 0x4c,
                    product: 0x269,
                },
            },
            ToHost::Scroll {
                gesture: GestureId(7),
                phase: Phase::Changed,
                dx_px: 0.0,
                dy_px: -12.25,
                fingers: 1,
                axis: Some(Axis::Y),
                t_ns: 123,
            },
            ToHost::Swipe {
                gesture: GestureId(8),
                phase: Phase::Began,
                fingers: 2,
                dx_units: 0.0,
                t_ns: 5,
            },
            ToHost::Tap {
                kind: TapKind::Two,
                t_ns: 9,
            },
            ToHost::Touches {
                active: ActiveTouch::Down,
            },
            ToHost::DeviceGone,
        ]
    }

    #[test]
    fn every_message_round_trips() {
        for m in samples() {
            assert_eq!(decode::<ToHost>(&encode(&m)).unwrap(), m);
            assert!(encode(&m).len() < MAX_PACKET);
        }
        for m in [
            ToDaemon::PointerOver { over: Over::Ours },
            ToDaemon::Subscribe {
                filter: Filter::Only(vec![Topic::Scroll, Topic::Touches]),
            },
        ] {
            assert_eq!(decode::<ToDaemon>(&encode(&m)).unwrap(), m);
        }
    }

    #[test]
    fn wire_form_is_adjacently_tagged() {
        let bytes = encode(&ToDaemon::PointerOver { over: Over::Ours });
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"kind":"pointer_over","v":{"over":"ours"}}"#
        );
        assert_eq!(
            String::from_utf8(encode(&ToHost::DeviceGone)).unwrap(),
            r#"{"kind":"device_gone"}"#
        );
    }

    #[test]
    fn filters_admit_by_topic() {
        let only = Filter::Only(vec![Topic::Scroll]);
        let got: Vec<bool> = samples().iter().map(|m| only.admits(m)).collect();
        assert_eq!(got, [true, true, false, false, false, true]);
        assert!(samples().iter().all(|m| Filter::All.admits(m)));
    }

    #[test]
    fn junk_is_an_error_not_a_panic() {
        assert!(decode::<ToDaemon>(b"{").is_err());
        assert!(decode::<ToDaemon>(&[b' '; MAX_PACKET + 1]).is_err());
    }
}
