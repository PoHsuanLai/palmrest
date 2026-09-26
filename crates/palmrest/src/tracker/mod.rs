//! `Tracker::step`: one report in, the outputs it causes out (12.4).
//!
//! The whole gesture state of the daemon is this one value. It never reads a clock or touches a
//! device: `now` is an argument and every effect is an `Out`, so a recording replays to exactly
//! the outputs the live daemon produced.

mod foreign;
mod out;

pub use out::{Out, Source, action_outs, tap_gesture, tap_outs};

use crate::action::Gesture;
use crate::config::{Config, Rejection};
use crate::feel::{Hold, Lock, LockParams};
use crate::gestures::{
    ClickGuard, Contacts, FingerDirection, MotionWindow, Moving, OneSwipe, PalmTurn, Press,
    ScrollEvent, ScrollG, SwipeEvent, SwipeG, TapG,
};
use crate::outputs::pad::Pad;
use crate::protocol::{ActiveTouch, Over, ToHost};
use crate::report::Report;
use crate::time::Mono;

/// Every piece of gesture state between reports.
#[derive(Debug, Clone, PartialEq)]
pub struct Tracker {
    cfg: Config,
    contacts: Contacts,
    click: ClickGuard,
    motion: MotionWindow,
    scroll: ScrollG,
    one: OneSwipe,
    swipe: SwipeG,
    tap: TapG,
    pad: Pad,
    claimed: Over,
    touching: ActiveTouch,
}

impl Tracker {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            contacts: Contacts::default(),
            click: ClickGuard::Idle,
            motion: MotionWindow::default(),
            scroll: ScrollG::default(),
            one: OneSwipe::Idle,
            swipe: SwipeG::default(),
            tap: TapG::Idle,
            pad: Pad::Up,
            claimed: Over::NotOurs,
            touching: ActiveTouch::Up,
        }
    }

    /// The configuration in force.
    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Applies a reloaded configuration; running gestures continue under it.
    pub fn with_config(self, cfg: Config) -> Self {
        Self { cfg, ..self }
    }

    /// The scroll lock decision after the last report (for parity comparison).
    pub fn lock(&self) -> Lock {
        self.scroll.lock()
    }

    /// Who holds the pointer.
    pub fn claimed(&self) -> Over {
        self.claimed
    }

    /// When `tick` next has work to do, if ever.
    pub fn deadline(&self) -> Option<Mono> {
        self.pad.deadline()
    }

    /// A host claimed or released the pointer (the aggregate over subscribers). Contacts on the
    /// virtual touchpad lift at once when a host takes over, so nothing is left pressed.
    pub fn claim(self, over: Over) -> (Self, Vec<Out>) {
        let (pad, frames) = if over == Over::Ours {
            self.pad.lift_now()
        } else {
            (self.pad, vec![])
        };
        (
            Self {
                claimed: over,
                pad,
                ..self
            },
            frames.into_iter().map(Out::Pad).collect(),
        )
    }

    /// Time passed with no report: performs a delayed touchpad lift that has come due.
    pub fn tick(self, now: Mono) -> (Self, Vec<Out>) {
        let (pad, frames) = self.pad.tick(now);
        (
            Self { pad, ..self },
            frames.into_iter().map(Out::Pad).collect(),
        )
    }

    /// The mouse went away: cancel what is running, lift everything, say so.
    pub fn disconnect(self, now: Mono) -> (Self, Vec<Out>) {
        let (scroll, events) = self.scroll.disconnect();
        let (mut s, mut out) = Self { scroll, ..self }.scroll_events(&events, now);
        let (_, frames) = s.pad.lift_now();
        out.extend(frames.into_iter().map(Out::Pad));
        if s.touching == ActiveTouch::Down {
            out.push(Out::Host(ToHost::Touches {
                active: ActiveTouch::Up,
            }));
        }
        out.push(Out::Host(ToHost::DeviceGone));
        s = Self {
            claimed: s.claimed,
            scroll: s.scroll,
            ..Self::new(s.cfg)
        };
        (s, out)
    }

    /// Consumes one report.
    pub fn step(self, r: &Report, now: Mono) -> (Self, Vec<Out>) {
        let pr = &self.cfg.palm_rejection;
        let (click, press) = self
            .click
            .step(r.buttons, now, pr.click_guard_ms.duration());
        let hold = click.hold(now);
        let window = pr.mouse_moving_window_ms.duration();
        let motion = self.motion.step(r.motion, now, window);
        let moving = motion.moving(f64::from(pr.mouse_moving_threshold_counts_s.0), window);
        let multiplier = match (self.cfg.rejection, moving) {
            (Rejection::On, Moving::Yes) => pr.mouse_moving_lock_multiplier.ratio(),
            _ => 1.0,
        };
        let (contacts, seen) = self.contacts.step(&r.touches, now, &self.cfg);
        let s = Self {
            click,
            motion,
            contacts,
            ..self
        };

        let (s, mut out) = s.tick(now);
        let (s, touches) = s.touches(seen.samples.is_empty(), ActiveTouch::Down);
        out.extend(touches);
        let (s, cancelled) = match (press, seen.palm) {
            (Press::Edge, _) | (_, PalmTurn::InSet) => s.cancel_scroll(now),
            _ => (s, vec![]),
        };
        out.extend(cancelled);
        let params = LockParams::from_config(&s.cfg, multiplier);
        let (scroll, events) = s.scroll.clone().step(&seen.samples, now, &params, hold);
        let (s, scrolled) = Self { scroll, ..s }.scroll_events(&events, now);
        out.extend(scrolled);
        let (s, fired) = s.gestures(&seen.samples, now, hold);
        out.extend(fired);
        let (s, touches) = s.touches(seen.samples.is_empty(), ActiveTouch::Up);
        out.extend(touches);
        (s, out)
    }

    /// `Touches{Down}` before this report's scroll phases, `Touches{Up}` after them.
    fn touches(self, empty: bool, which: ActiveTouch) -> (Self, Vec<Out>) {
        let next = if empty {
            ActiveTouch::Up
        } else {
            ActiveTouch::Down
        };
        if next == which && next != self.touching {
            (
                Self {
                    touching: next,
                    ..self
                },
                vec![Out::Host(ToHost::Touches { active: next })],
            )
        } else {
            (self, vec![])
        }
    }

    fn cancel_scroll(self, now: Mono) -> (Self, Vec<Out>) {
        let (scroll, events) = self.scroll.clone().cancel();
        Self { scroll, ..self }.scroll_events(&events, now)
    }

    fn scroll_events(self, events: &[ScrollEvent], now: Mono) -> (Self, Vec<Out>) {
        let mut pad = self.pad;
        let mut out = Vec::new();
        for e in events {
            let (p, o) = foreign::scroll_outs(e, pad, self.claimed, &self.cfg, now);
            pad = p;
            out.extend(o);
        }
        (Self { pad, ..self }, out)
    }

    /// G4, G5 and the double-taps, after the scroll has seen the report.
    fn gestures(self, samples: &[crate::feel::Sample], now: Mono, hold: Hold) -> (Self, Vec<Out>) {
        let map = &self.cfg.gestures.gesture_action_map;
        let mut out = Vec::new();
        let (one, g4) = self
            .one
            .step(samples, self.scroll.lock(), now, &self.cfg, hold);
        if let Some(dir) = g4 {
            let g = pick(
                dir,
                Gesture::OneFingerSwipeRight,
                Gesture::OneFingerSwipeLeft,
            );
            out.extend(action_outs(
                map.action_for(g),
                Source::OneFingerSwipe,
                self.claimed,
                self.cfg.gestures.g4_foreign_mode,
            ));
        }
        let (swipe, events) = self.swipe.clone().step(samples, now, &self.cfg, hold);
        for e in events {
            out.extend(self.swipe_outs(e, now));
        }
        let (tap, kind) = self.tap.clone().step(samples, now, &self.cfg, hold);
        if let Some(kind) = kind {
            out.extend(tap_outs(
                kind,
                map.action_for(tap_gesture(kind)),
                self.claimed,
                now.0,
            ));
        }
        (
            Self {
                one,
                swipe,
                tap,
                ..self
            },
            out,
        )
    }

    fn swipe_outs(&self, e: SwipeEvent, now: Mono) -> Vec<Out> {
        match e {
            SwipeEvent::Stream {
                gesture,
                phase,
                dx_units,
            } => {
                vec![Out::Host(ToHost::Swipe {
                    gesture,
                    phase,
                    fingers: 2,
                    dx_units,
                    t_ns: now.0,
                })]
            }
            SwipeEvent::Fire(dir) => {
                let g = pick(
                    dir,
                    Gesture::TwoFingerSwipeRight,
                    Gesture::TwoFingerSwipeLeft,
                );
                let action = self.cfg.gestures.gesture_action_map.action_for(g);
                action_outs(
                    action,
                    Source::Other,
                    self.claimed,
                    self.cfg.gestures.g4_foreign_mode,
                )
            }
        }
    }
}

fn pick(dir: FingerDirection, right: Gesture, left: Gesture) -> Gesture {
    match dir {
        FingerDirection::Right => right,
        FingerDirection::Left => left,
    }
}
