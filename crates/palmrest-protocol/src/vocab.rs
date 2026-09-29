//! The words a message is made of: gesture ids, phases, axes, taps, touch and pointer state.

use serde::{Deserialize, Serialize};

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
