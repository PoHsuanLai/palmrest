//! What the axis lock reads and reports: the contacts it follows, the click hold, and the
//! events one report produces.

use super::quantize::Quanta;
use super::rule::Lock;
use crate::touch::{Point, TouchId};
use palmrest_protocol::Axis;

/// A contact the lock follows: its id and where it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub id: TouchId,
    pub at: Point,
}

/// Whether a physical click is holding gestures off (P4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    Open,
    Held,
}

/// One sample's scroll on the locked axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    pub axis: Axis,
    pub raw: i32,
    /// Px in finger direction, all gains applied: the wheel stream before truncation.
    pub px: f64,
    /// The wheel device's share of this sample.
    pub quanta: Quanta,
    /// The gain over the base ratio this sample was scrolled with.
    pub gain: f64,
}

/// What one report did to the lock, in order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LockEvent {
    /// Fewer than one or more than two contacts: the stroke is over.
    Reset,
    /// A new contact set landed (or the set changed).
    Start { fingers: u8 },
    /// The lock timed out on this report's delta.
    Unlocked,
    /// The lock was decided.
    Locked(Lock),
    /// Scroll on the locked axis.
    Moved(Move),
}
