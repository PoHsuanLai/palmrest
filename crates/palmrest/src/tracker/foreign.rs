//! A scroll event's outputs: the socket message always, and for foreign clients (nobody
//! claiming the pointer) the virtual touchpad or the wheel device, with the glide after a flick
//! when palmrest runs it (`foreign_momentum = "ours"`).

use super::coast::{Coast, Sink};
use super::out::Out;
use crate::config::{Config, ForeignMomentum, ForeignOutput};
use crate::feel::{ReleaseRule, ReleaseSamples, UnitCarry, WheelDelta};
use crate::gestures::ScrollEvent;
use crate::outputs::pad::Pad;
use crate::protocol::{Axis, Over, Phase, ToHost};
use crate::time::Mono;

/// What foreign clients have been sent: the pad's contacts, a running glide, and the running
/// stroke's samples for its release velocity.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Foreign {
    pad: Pad,
    coast: Coast,
    samples: ReleaseSamples,
}

impl Foreign {
    /// When `tick` next has work: a delayed lift or a glide frame.
    pub fn deadline(&self) -> Option<Mono> {
        match (self.pad.deadline(), self.coast.deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Time passed: a delayed lift that has come due, and the glide's frame.
    pub fn tick(self, now: Mono, claimed: Over, cfg: &Config) -> (Self, Vec<Out>) {
        let (pad, frames) = self.pad.tick(now);
        let mut out: Vec<Out> = frames.into_iter().map(Out::Pad).collect();
        let (coast, pad, glide) = match claimed {
            Over::Ours => self.coast.stop(pad),
            Over::NotOurs => self.coast.advance(now, pad, cfg),
        };
        out.extend(glide);
        (Self { pad, coast, ..self }, out)
    }

    /// Lifts everything and ends any glide at once (a host claimed the pointer, the device
    /// went away).
    pub fn lift_now(self) -> (Self, Vec<Out>) {
        let (pad, frames) = self.pad.lift_now();
        (
            Self {
                pad,
                ..Self::default()
            },
            frames.into_iter().map(Out::Pad).collect(),
        )
    }

    /// The outputs of one scroll event.
    pub fn scroll(
        self,
        e: &ScrollEvent,
        claimed: Over,
        cfg: &Config,
        now: Mono,
    ) -> (Self, Vec<Out>) {
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
        let (s, foreign) = match (claimed, cfg.foreign_output) {
            (Over::Ours, _) | (_, ForeignOutput::Off) => (self, vec![]),
            (Over::NotOurs, ForeignOutput::Touchpad) => self.pad_outs(e, dx, dy, cfg, now),
            (Over::NotOurs, ForeignOutput::Wheel) => self.wheel_outs(e, cfg, now),
        };
        (s, std::iter::once(host).chain(foreign).collect())
    }

    fn pad_outs(
        self,
        e: &ScrollEvent,
        dx: f64,
        dy: f64,
        cfg: &Config,
        now: Mono,
    ) -> (Self, Vec<Out>) {
        let tp = &cfg.touchpad;
        let pads = |(pad, frames): (Pad, Vec<_>)| (pad, frames.into_iter().map(Out::Pad).collect());
        match e.phase {
            Phase::MayBegin => {
                let (coast, pad, out) = self.coast.stop(self.pad);
                (Self { pad, coast, ..self }, out)
            }
            Phase::Began => {
                let (pad, out) = pads(self.pad.land(now, tp));
                (
                    Self {
                        pad,
                        coast: Coast::Idle,
                        samples: ReleaseSamples::default(),
                    },
                    out,
                )
            }
            Phase::Changed => {
                let (pad, out) = pads(self.pad.moved(dx, dy, now, tp));
                let samples =
                    self.samples
                        .push(now, e.px, &ReleaseRule::from_settings(&cfg.momentum));
                (
                    Self {
                        pad,
                        samples,
                        ..self
                    },
                    out,
                )
            }
            Phase::Ended => match self.glide(e, Sink::Pad, cfg, now) {
                Some(coast) => (Self { coast, ..self }, vec![]),
                None => {
                    let (pad, out) = pads(self.pad.end(now, tp));
                    (Self { pad, ..self }, out)
                }
            },
            Phase::Cancelled => {
                let (pad, out) = pads(self.pad.end(now, tp));
                (
                    Self {
                        pad,
                        coast: Coast::Idle,
                        ..self
                    },
                    out,
                )
            }
        }
    }

    fn wheel_outs(self, e: &ScrollEvent, cfg: &Config, now: Mono) -> (Self, Vec<Out>) {
        match (e.phase, e.axis) {
            (Phase::Changed, Some(axis)) => {
                let w = WheelDelta::on(axis, e.quanta);
                let out = if w.has_events() {
                    vec![Out::Wheel(w)]
                } else {
                    vec![]
                };
                let samples =
                    self.samples
                        .push(now, e.px, &ReleaseRule::from_settings(&cfg.momentum));
                (Self { samples, ..self }, out)
            }
            (Phase::Began, _) => (
                Self {
                    coast: Coast::Idle,
                    samples: ReleaseSamples::default(),
                    ..self
                },
                vec![],
            ),
            (Phase::Ended, _) => {
                let coast = self
                    .glide(e, Sink::Wheel(UnitCarry::default()), cfg, now)
                    .unwrap_or(Coast::Idle);
                (Self { coast, ..self }, vec![])
            }
            _ => (
                Self {
                    coast: Coast::Idle,
                    ..self
                },
                vec![],
            ),
        }
    }

    /// The glide an Ended starts, if palmrest runs momentum and the release was a flick.
    fn glide(&self, e: &ScrollEvent, sink: Sink, cfg: &Config, now: Mono) -> Option<Coast> {
        let axis = e.axis?;
        if cfg.foreign_momentum != ForeignMomentum::Ours {
            return None;
        }
        let v0 = self
            .samples
            .velocity(now, &ReleaseRule::from_settings(&cfg.momentum));
        (v0 != 0.0).then(|| Coast::start(axis, v0, now, sink, cfg))
    }
}
