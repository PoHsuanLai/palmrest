//! Synthesized momentum for foreign clients (`foreign_momentum = "ours"`).
//!
//! After a flick the virtual touchpad's contacts stay down and keep moving along the design/11
//! §11.5.1 drag curve, one frame every `momentum.frame_ms`, then lift once the glide has fallen
//! to the stop speed, so the toolkit's own kinetic scroll starts from ~1 px/s and adds nothing.
//! On the wheel output the same glide is a decaying run of hi-res wheel frames. Every frame is
//! the closed-form travel at that instant minus what was already sent, so a late tick sends one
//! larger frame and the total never drifts.

use super::out::Out;
use crate::config::{Config, TouchpadSettings};
use crate::feel::{Drag, HI_RES_PER_DETENT, UnitCarry, WheelDelta};
use crate::outputs::pad::{Pad, PadFrame};
use crate::protocol::Axis;
use crate::time::Mono;

/// Where the glide goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sink {
    /// Move the virtual touchpad's contacts (O2).
    Pad,
    /// Hi-res wheel frames (O4), with the fractional units carried.
    Wheel(UnitCarry),
}

/// A glide in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glide {
    axis: Axis,
    /// The release velocity, signed, finger direction, px/s.
    v0: f64,
    from: Mono,
    /// Px already sent (a magnitude).
    sent: f64,
    next: Mono,
    sink: Sink,
}

/// The foreign glide between reports.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Coast {
    #[default]
    Idle,
    Gliding(Glide),
}

impl Coast {
    /// A glide from `v0` px/s on `axis`, released at `now`.
    pub fn start(axis: Axis, v0: f64, now: Mono, sink: Sink, cfg: &Config) -> Self {
        Coast::Gliding(Glide {
            axis,
            v0,
            from: now,
            sent: 0.0,
            next: now.after(cfg.momentum.frame_ms.duration()),
            sink,
        })
    }

    /// When the next frame is due.
    pub fn deadline(&self) -> Option<Mono> {
        match self {
            Coast::Idle => None,
            Coast::Gliding(g) => Some(g.next),
        }
    }

    /// Touch to stop, a claim, a cancel: the glide ends now and the contacts lift at once.
    pub fn stop(self, pad: Pad) -> (Self, Pad, Vec<Out>) {
        match self {
            Coast::Gliding(Glide {
                sink: Sink::Pad, ..
            }) => {
                let (pad, frames) = pad.lift_now();
                (Coast::Idle, pad, frames.into_iter().map(Out::Pad).collect())
            }
            _ => (Coast::Idle, pad, vec![]),
        }
    }

    /// The frame due at `now`, if the glide is running; the lift once it has run out.
    pub fn advance(self, now: Mono, pad: Pad, cfg: &Config) -> (Self, Pad, Vec<Out>) {
        let Coast::Gliding(g) = self else {
            return (self, pad, vec![]);
        };
        if now <= g.from {
            return (self, pad, vec![]);
        }
        let drag = Drag::from_settings(&cfg.momentum);
        let speed = g.v0.abs();
        let t = now.seconds_since(g.from);
        let travel = drag.travel(speed, t);
        let px = (travel - g.sent) * g.v0.signum();
        let (pad, sink, mut out) = match g.sink {
            Sink::Pad => {
                let (pad, out) = move_pad(pad, g.axis, px, now, &cfg.touchpad);
                (pad, Sink::Pad, out)
            }
            Sink::Wheel(carry) => {
                let units = px * f64::from(HI_RES_PER_DETENT) / f64::from(cfg.scroll.detent_px.0);
                let (carry, q) = carry.step(units);
                let w = WheelDelta::on(g.axis, q);
                let out = if w.has_events() {
                    vec![Out::Wheel(w)]
                } else {
                    vec![]
                };
                (pad, Sink::Wheel(carry), out)
            }
        };
        let end = g
            .from
            .after(std::time::Duration::from_secs_f64(drag.duration(speed)));
        if now >= end {
            let (pad, lift) = match sink {
                Sink::Pad => pad.end(now, &cfg.touchpad),
                Sink::Wheel(_) => (pad, vec![]),
            };
            out.extend(lift.into_iter().map(Out::Pad));
            return (Coast::Idle, pad, out);
        }
        let next = now.after(cfg.momentum.frame_ms.duration()).min(end);
        let g = Glide {
            sent: travel,
            next,
            sink,
            ..g
        };
        (Coast::Gliding(g), pad, out)
    }
}

/// Moves the pair by `px` on `axis`, re-landing at the centre near an edge (F8) and leaving
/// out a move that does not change the contacts' position. A re-land is followed at once by
/// this frame's move from the centre, so the glide loses no distance to it (the stroke's own
/// re-land drops that frame's delta).
fn move_pad(pad: Pad, axis: Axis, px: f64, now: Mono, tp: &TouchpadSettings) -> (Pad, Vec<Out>) {
    let before = pad.at();
    let (dx, dy) = match axis {
        Axis::X => (px, 0.0),
        Axis::Y => (0.0, px),
    };
    let (pad, mut frames) = pad.moved(dx, dy, now, tp);
    let relanded = frames.iter().any(|f| matches!(f, PadFrame::Land { .. }));
    let pad = if relanded {
        let (pad, again) = pad.moved(dx, dy, now, tp);
        frames.extend(again);
        pad
    } else {
        pad
    };
    let out = frames
        .into_iter()
        .filter(|f| !matches!(f, PadFrame::Move { at } if Some(*at) == before))
        .map(Out::Pad)
        .collect();
    (pad, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ForeignMomentum;

    fn cfg() -> Config {
        Config {
            foreign_momentum: ForeignMomentum::Ours,
            ..Config::default()
        }
    }

    /// Runs a glide at every deadline and returns the px sent (from the wheel's hi-res units)
    /// and when it ended.
    fn run_wheel(v0: f64) -> (f64, Mono, usize) {
        let c = cfg();
        let mut coast = Coast::start(Axis::X, v0, Mono(0), Sink::Wheel(UnitCarry::default()), &c);
        let (mut units, mut frames, mut last) = (0, 0, Mono(0));
        while let Some(d) = coast.deadline() {
            let (next, _, out) = coast.advance(d, Pad::Up, &c);
            for o in out {
                if let Out::Wheel(w) = o {
                    units += w.horizontal_hi_res;
                    frames += 1;
                }
            }
            (coast, last) = (next, d);
        }
        (f64::from(units) * 60.0 / 120.0, last, frames)
    }

    #[test]
    fn a_2000_px_s_release_glides_501_px_in_976_ms() {
        let (px, end, frames) = run_wheel(2000.0);
        assert!((px - 501.5).abs() / 501.5 < 0.05, "{px}");
        assert!((end.0 as f64 / 1e6 - 976.0).abs() < 2.0, "{end:?}");
        assert!(frames > 50, "a run of frames, not one jump: {frames}");
        let (back, _, _) = run_wheel(-2000.0);
        assert!((back + px).abs() <= 0.5, "symmetric: {back}");
    }

    #[test]
    fn on_the_pad_the_same_glide_moves_the_contacts_501_px_through_its_relands() {
        let c = cfg();
        let (mut pad, _) = Pad::Up.land(Mono(0), &c.touchpad);
        let mut coast = Coast::start(Axis::Y, 2000.0, Mono(0), Sink::Pad, &c);
        let (mut units, mut y, mut end) = (0, pad.at().map(|p| p.y), Mono(0));
        while let Some(d) = coast.deadline() {
            let (next, p, out) = coast.advance(d, pad, &c);
            for o in out {
                match o {
                    Out::Pad(PadFrame::Land { at, .. }) => y = Some(at.y),
                    Out::Pad(PadFrame::Move { at }) => {
                        units += at.y - y.unwrap();
                        y = Some(at.y);
                    }
                    Out::Pad(PadFrame::Lift) => (y, end) = (None, d),
                    _ => {}
                }
            }
            (coast, pad) = (next, p);
        }
        let px = f64::from(units) / 10.0;
        assert!((px - 501.5).abs() / 501.5 < 0.05, "{px}");
        assert!(
            (end.0 as f64 / 1e6 - 976.0).abs() < 2.0,
            "lifted at {end:?}"
        );
        assert_eq!(pad, Pad::Up);
    }

    #[test]
    fn frames_decay() {
        let c = cfg();
        let mut coast = Coast::start(
            Axis::Y,
            3000.0,
            Mono(0),
            Sink::Wheel(UnitCarry::default()),
            &c,
        );
        let mut sizes = Vec::new();
        while let Some(d) = coast.deadline() {
            let (next, _, out) = coast.advance(d, Pad::Up, &c);
            sizes.extend(out.iter().filter_map(|o| match o {
                Out::Wheel(w) => Some(-w.vertical_hi_res),
                _ => None,
            }));
            coast = next;
        }
        assert!(
            sizes[0] > 40,
            "3000 px/s for 8 ms is 24 px, 48 units: {sizes:?}"
        );
        assert!(sizes.windows(2).all(|w| w[1] <= w[0] + 1), "{sizes:?}");
        assert!(*sizes.last().unwrap() <= 1);
    }

    #[test]
    fn stopping_lifts_the_pad_at_once() {
        let c = cfg();
        let (pad, _) = Pad::Up.land(Mono(0), &c.touchpad);
        let coast = Coast::start(Axis::Y, 2000.0, Mono(0), Sink::Pad, &c);
        let (coast, pad, out) = coast.stop(pad);
        assert_eq!((coast, pad), (Coast::Idle, Pad::Up));
        assert_eq!(out, [Out::Pad(PadFrame::Lift)]);
    }
}
