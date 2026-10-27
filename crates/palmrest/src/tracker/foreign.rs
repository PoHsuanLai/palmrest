//! A scroll event's outputs: the socket message always, and for foreign clients (nobody
//! claiming the pointer) the virtual touchpad. The app runs its own kinetic scroll after the
//! pad lifts.

use super::out::Out;
use crate::gestures::ScrollEvent;
use crate::outputs::pad::Pad;
use crate::time::Mono;
use palmrest_protocol::{Axis, Over, Phase, ToHost};

/// What foreign clients have been sent: the pad's contacts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Foreign {
    pad: Pad,
}

impl Foreign {
    /// When `tick` next has work: a delayed lift.
    pub fn deadline(&self) -> Option<Mono> {
        self.pad.deadline()
    }

    /// Time passed: a delayed lift that has come due.
    pub fn tick(self, now: Mono) -> (Self, Vec<Out>) {
        let (pad, frames) = self.pad.tick(now);
        (Self { pad }, frames.into_iter().map(Out::Pad).collect())
    }

    /// Lifts everything at once (a host claimed the pointer, the device went away).
    pub fn lift_now(self) -> (Self, Vec<Out>) {
        let (pad, frames) = self.pad.lift_now();
        (Self { pad }, frames.into_iter().map(Out::Pad).collect())
    }

    /// The outputs of one scroll event.
    pub fn scroll(self, e: &ScrollEvent, claimed: Over, now: Mono) -> (Self, Vec<Out>) {
        let (dx, dy) = match e.axis {
            Some(Axis::X) => (e.px, 0.0),
            Some(Axis::Y) => (0.0, e.px),
            None => (0.0, 0.0),
        };
        let host = Out::Host(ToHost::Scroll {
            gesture: e.gesture,
            phase: e.phase,
            dx_px: dx as f32,
            dy_px: dy as f32,
            fingers: e.fingers,
            axis: e.axis,
            t_ns: now.0,
        });
        let (s, foreign) = match claimed {
            Over::Ours => (self, vec![]),
            Over::NotOurs => self.pad_outs(e.phase, dx, dy, now),
        };
        (s, std::iter::once(host).chain(foreign).collect())
    }

    fn pad_outs(self, phase: Phase, dx: f64, dy: f64, now: Mono) -> (Self, Vec<Out>) {
        let pads = |(pad, frames): (Pad, Vec<_>)| (pad, frames.into_iter().map(Out::Pad).collect());
        let (pad, out) = match phase {
            Phase::MayBegin => return (self, vec![]),
            Phase::Began => pads(self.pad.land(now)),
            Phase::Changed => pads(self.pad.moved(dx, dy, now)),
            Phase::Ended | Phase::Cancelled => pads(self.pad.end(now)),
        };
        (Self { pad }, out)
    }
}
