//! One message per packet: the version, the size limit, the codec and the version check.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::message::ToHost;

/// The protocol version carried by `Hello`.
/// 2 added `ToHost::Run` and `ToDaemon::UserConfig` and the larger packet; 3 made the workspace
/// steps one `HostAction::Workspace(Workspace)`. Only the current version is read.
pub const PROTOCOL_VERSION: u16 = 3;
/// No message is larger than this; a longer packet is not ours. Room for [`USER_CONFIG_MAX_BYTES`]
/// of settings with every quote and line break escaped.
pub const MAX_PACKET: usize = 64 * 1024;
/// The most text of the person's settings that travels (`ToDaemon::UserConfig`); a larger file is
/// not a settings file.
pub const USER_CONFIG_MAX_BYTES: usize = 24 * 1024;

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
