//! G5: two-finger horizontal travel switches workspaces (Trigger mode), the Python daemon's
//! `DesktopSwipe` as a value.
//!
//! Travel is the pair's centroid from where it landed, on the grid. Vertical travel that
//! dominates past `vertical_max_um` re-anchors (the pair is scrolling; "dominates" compares grid
//! units, as it always did); `threshold_um` of horizontal travel fires once
//! in the direction the fingers moved and re-anchors, so a long drag keeps going after the
//! cooldown. The same pair also feeds the host a raw `Swipe` stream for the live model.

use crate::config::{Config, SwipeRepeat, WorkspaceMode};
use crate::feel::{Hold, Sample};
use crate::protocol::{GestureId, Phase};
use crate::sensor::lattice::{Along, units};
use crate::time::Mono;
use crate::touch::TouchId;

/// Which way the fingers moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
struct Pair {
    ids: [TouchId; 2],
    last: [Sample; 2],
    travel_x: i32,
    travel_y: i32,
    gesture: GestureId,
    fired: Fired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fired {
    Not,
    Once,
}

/// The swipe between reports.
#[derive(Debug, Clone, PartialEq)]
pub struct SwipeG {
    pair: Option<Pair>,
    last_fire: Option<Mono>,
    next_id: u32,
}

impl Default for SwipeG {
    fn default() -> Self {
        Self {
            pair: None,
            last_fire: None,
            next_id: 1,
        }
    }
}

/// What one report produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwipeEvent {
    /// For the host: the pair's phase and this report's centroid dx in grid units.
    Stream {
        gesture: GestureId,
        phase: Phase,
        dx_units: f32,
    },
    /// Trigger mode fired.
    Fire(FingerDirection),
}

impl SwipeG {
    /// Consumes one report's contacts.
    pub fn step(
        self,
        contacts: &[Sample],
        now: Mono,
        cfg: &Config,
        hold: Hold,
    ) -> (Self, Vec<SwipeEvent>) {
        let [a, b] = contacts else { return self.end() };
        let mut ids = [a.id, b.id];
        ids.sort();
        match self.pair.clone() {
            Some(p) if p.ids == ids && a.id != b.id => self.advance(p, [*a, *b], now, cfg, hold),
            _ => {
                let (s, mut ev) = self.end();
                let gesture = GestureId(s.next_id);
                ev.push(SwipeEvent::Stream {
                    gesture,
                    phase: Phase::Began,
                    dx_units: 0.0,
                });
                let pair = Pair {
                    ids,
                    last: [*a, *b],
                    travel_x: 0,
                    travel_y: 0,
                    gesture,
                    fired: Fired::Not,
                };
                (
                    Self {
                        pair: Some(pair),
                        next_id: s.next_id + 1,
                        ..s
                    },
                    ev,
                )
            }
        }
    }

    fn end(self) -> (Self, Vec<SwipeEvent>) {
        let ev = self
            .pair
            .as_ref()
            .map(|p| SwipeEvent::Stream {
                gesture: p.gesture,
                phase: Phase::Ended,
                dx_units: 0.0,
            })
            .into_iter()
            .collect();
        (Self { pair: None, ..self }, ev)
    }

    fn advance(
        self,
        p: Pair,
        now_at: [Sample; 2],
        now: Mono,
        cfg: &Config,
        hold: Hold,
    ) -> (Self, Vec<SwipeEvent>) {
        let (sx, sy) = now_at.iter().fold((0, 0), |(sx, sy), c| {
            let prev = p.last.iter().find(|s| s.id == c.id).map_or(c.at, |s| s.at);
            let d = c.at.delta_from(prev);
            (sx + d.x, sy + d.y)
        });
        let (dx, dy) = (sx / 2, sy / 2);
        let mut ev = Vec::new();
        if dx != 0 {
            ev.push(SwipeEvent::Stream {
                gesture: p.gesture,
                phase: Phase::Changed,
                dx_units: dx as f32,
            });
        }
        let p = Pair {
            last: now_at,
            travel_x: p.travel_x + dx,
            travel_y: p.travel_y + dy,
            ..p
        };
        let s = &cfg.swipe;
        let vertical_max = units(s.vertical_max_um, Along::Y);
        let threshold = units(s.threshold_um, Along::X);
        let anchor = |p: Pair| Pair {
            travel_x: 0,
            travel_y: 0,
            ..p
        };
        let (p, last_fire) = if hold == Hold::Held
            || (p.travel_y.abs() > vertical_max && p.travel_y.abs() > p.travel_x.abs())
        {
            (anchor(p), self.last_fire)
        } else if p.travel_x.abs() < threshold.max(1) {
            (p, self.last_fire)
        } else {
            let dir = if p.travel_x > 0 {
                FingerDirection::Right
            } else {
                FingerDirection::Left
            };
            let cooled = self
                .last_fire
                .is_none_or(|t| now.since(t) >= s.cooldown_ms.duration());
            let allowed = s.workspace_mode == WorkspaceMode::Trigger
                && !(s.repeat == SwipeRepeat::OncePerGesture && p.fired == Fired::Once);
            if cooled && allowed {
                ev.push(SwipeEvent::Fire(dir));
                (
                    Pair {
                        fired: Fired::Once,
                        ..anchor(p)
                    },
                    Some(now),
                )
            } else {
                (anchor(p), self.last_fire)
            }
        };
        (
            Self {
                pair: Some(p),
                last_fire,
                ..self
            },
            ev,
        )
    }
}
