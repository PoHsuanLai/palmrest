//! A scroll event's outputs: the socket message always, and for foreign clients (nobody
//! claiming the pointer) the virtual touchpad or the wheel device.

use super::out::Out;
use crate::config::{Config, ForeignOutput};
use crate::feel::WheelDelta;
use crate::gestures::ScrollEvent;
use crate::outputs::pad::Pad;
use crate::protocol::{Axis, Over, Phase, ToHost};
use crate::time::Mono;

/// The outputs of one scroll event, and the pad after it.
pub fn scroll_outs(
    e: &ScrollEvent,
    pad: Pad,
    claimed: Over,
    cfg: &Config,
    now: Mono,
) -> (Pad, Vec<Out>) {
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
    let (pad, foreign) = match (claimed, cfg.foreign_output) {
        (Over::Ours, _) | (_, ForeignOutput::Off) => (pad, vec![]),
        (Over::NotOurs, ForeignOutput::Touchpad) => pad_outs(e, pad, dx, dy, cfg, now),
        (Over::NotOurs, ForeignOutput::Wheel) => (pad, wheel_outs(e)),
    };
    (pad, std::iter::once(host).chain(foreign).collect())
}

fn pad_outs(
    e: &ScrollEvent,
    pad: Pad,
    dx: f64,
    dy: f64,
    cfg: &Config,
    now: Mono,
) -> (Pad, Vec<Out>) {
    let tp = &cfg.touchpad;
    let (pad, frames) = match e.phase {
        Phase::MayBegin => (pad, vec![]),
        Phase::Began => pad.land(now, tp),
        Phase::Changed => pad.moved(dx, dy, now, tp),
        Phase::Ended | Phase::Cancelled => pad.end(now, tp),
    };
    (pad, frames.into_iter().map(Out::Pad).collect())
}

fn wheel_outs(e: &ScrollEvent) -> Vec<Out> {
    match (e.phase, e.axis) {
        (Phase::Changed, Some(axis)) => {
            let w = WheelDelta::on(axis, e.quanta);
            if w.has_events() {
                vec![Out::Wheel(w)]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}
