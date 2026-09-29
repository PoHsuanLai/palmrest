//! One message per packet: the version, the size limit, the codec and the version check.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::message::ToHost;

/// The protocol version carried by `Hello`.
pub const PROTOCOL_VERSION: u16 = 1;
/// No message is larger than this; a longer packet is not ours.
pub const MAX_PACKET: usize = 4096;

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

/// Whether a connection that opened with `message` may be read on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compat {
    Compatible,
    /// A `Hello` of another protocol version.
    Incompatible {
        version: u16,
    },
}

/// `message`'s verdict on the connection: only a `Hello` of another version is incompatible.
pub fn compat(message: &ToHost) -> Compat {
    match message {
        ToHost::Hello { version, .. } if *version != PROTOCOL_VERSION => {
            Compat::Incompatible { version: *version }
        }
        _ => Compat::Compatible,
    }
}
