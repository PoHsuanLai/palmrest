//! The socket protocol between palmrest and shell-host (12.3.10, O1).
//!
//! `/run/palmrest/events.sock`, `SOCK_SEQPACKET`: one JSON message per packet, so
//! there is no framing to get wrong and a reader never sees half a message. The first packet
//! on every connection is `Hello`, whose `version` a client checks before reading anything
//! else; an incompatible change bumps `PROTOCOL_VERSION`.
//!
//! Scroll deltas are `f32` px in **finger direction** with every gain applied; the host applies
//! natural scrolling (11.3.13). These are the only floats in palmrest's public types, because
//! a sub-pixel delta is the whole point of the stream.

mod message;
mod vocab;
mod wire;

#[cfg(test)]
mod tests;

pub use message::{DeviceInfo, Filter, ToDaemon, ToHost, Topic};
pub use vocab::{
    ACTION_WORD_MAX_BYTES, ACTION_WORDS_MAX, ActiveTouch, Axis, GestureId, HostAction, LockState,
    Over, Phase, TapKind, Workspace,
};
pub use wire::{
    Compat, MAX_PACKET, PROTOCOL_VERSION, ProtocolError, USER_CONFIG_MAX_BYTES, compat, decode,
    encode,
};
