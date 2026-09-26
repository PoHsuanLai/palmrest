//! G1/G2: the axis lock's decisions as scroll phases (12.3.5, 12.4 "Scroll").
//!
//! MayBegin when a set of one or two fingers lands; Began when the axis locks; Changed per
//! locked sample; Ended when the set ends, changes or the lock times out; Cancelled on a click
//! or a palm. Every MayBegin is closed by exactly one Ended or Cancelled, so a host never holds
//! a gesture open.

use crate::feel::{AxisLock, Hold, Lock, LockEvent, LockParams, Quanta, Sample};
use crate::protocol::{Axis, GestureId, Phase};
use crate::time::Mono;

/// How far the current gesture has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// MayBegin sent, no lock yet.
    Announced,
    Began(Axis),
    /// Two fingers locked sideways: the workspace swipe owns them.
    Blocked,
    /// The last gesture was Cancelled and the fingers are still down; the next lock opens a
    /// new gesture.
    Quiet,
}

/// The scroll gesture between reports.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollG {
    lock: AxisLock,
    current: Option<(GestureId, u8, Stage)>,
    next_id: u32,
}

impl Default for ScrollG {
    fn default() -> Self {
        Self {
            lock: AxisLock::default(),
            current: None,
            next_id: 1,
        }
    }
}

/// One phase to emit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollEvent {
    pub gesture: GestureId,
    pub phase: Phase,
    pub fingers: u8,
    pub axis: Option<Axis>,
    /// Finger-direction px on `axis`; zero except for Changed.
    pub px: f64,
    /// The wheel device's share of a Changed sample.
    pub quanta: Quanta,
}

impl ScrollG {
    /// The lock decision after the last report.
    pub fn lock(&self) -> Lock {
        self.lock.lock()
    }

    /// The running gesture's stage, if fingers are down.
    pub fn stage(&self) -> Option<Stage> {
        self.current.map(|(_, _, s)| s)
    }

    /// Consumes one report's contacts.
    pub fn step(
        self,
        contacts: &[Sample],
        now: Mono,
        p: &LockParams,
        hold: Hold,
    ) -> (Self, Vec<ScrollEvent>) {
        let (lock, events) = self.lock.step(contacts, now, p, hold);
        let start = Self { lock, ..self };
        events
            .into_iter()
            .fold((start, Vec::new()), |(s, mut out), e| {
                let s = s.apply(e, &mut out);
                (s, out)
            })
    }

    /// Cancels the running gesture (P3 palm, P4 click, disconnect).
    pub fn cancel(self) -> (Self, Vec<ScrollEvent>) {
        match self.current {
            Some((id, fingers, stage)) if stage != Stage::Quiet => {
                let ev = event(id, Phase::Cancelled, fingers, axis_of(stage));
                let (next, next_id) = (GestureId(self.next_id), self.next_id + 1);
                let s = Self {
                    lock: self.lock.cancel(),
                    current: Some((next, fingers, Stage::Quiet)),
                    next_id,
                };
                (s, vec![ev])
            }
            _ => (self, vec![]),
        }
    }

    /// Cancels and forgets everything, the repeat history included (device gone).
    pub fn disconnect(self) -> (Self, Vec<ScrollEvent>) {
        let (s, ev) = self.cancel();
        (
            Self {
                lock: s.lock.reset_all(),
                current: None,
                next_id: s.next_id,
            },
            ev,
        )
    }

    fn apply(self, e: LockEvent, out: &mut Vec<ScrollEvent>) -> Self {
        match e {
            LockEvent::Reset => self.close(out),
            LockEvent::Start { fingers } => self.close(out).open(fingers, out),
            LockEvent::Unlocked => {
                let fingers = self.current.map_or(1, |(_, f, _)| f);
                self.close(out).open(fingers, out)
            }
            LockEvent::Locked(lock) => self.locked(lock, out),
            LockEvent::Moved(m) => {
                if let Some((id, fingers, Stage::Began(axis))) = self.current {
                    out.push(ScrollEvent {
                        px: m.px,
                        quanta: m.quanta,
                        ..event(id, Phase::Changed, fingers, Some(axis))
                    });
                }
                self
            }
        }
    }

    fn locked(self, lock: Lock, out: &mut Vec<ScrollEvent>) -> Self {
        let Some((id, fingers, stage)) = self.current else {
            return self;
        };
        let axis = match lock {
            Lock::X => Axis::X,
            Lock::Y => Axis::Y,
            Lock::Blocked => {
                return Self {
                    current: Some((id, fingers, Stage::Blocked)),
                    ..self
                };
            }
            Lock::Free { .. } => return self,
        };
        if stage == Stage::Quiet {
            out.push(event(id, Phase::MayBegin, fingers, None));
        }
        out.push(event(id, Phase::Began, fingers, Some(axis)));
        Self {
            current: Some((id, fingers, Stage::Began(axis))),
            ..self
        }
    }

    fn close(self, out: &mut Vec<ScrollEvent>) -> Self {
        if let Some((id, fingers, stage)) = self.current
            && stage != Stage::Quiet
        {
            out.push(event(id, Phase::Ended, fingers, axis_of(stage)));
        }
        Self {
            current: None,
            ..self
        }
    }

    fn open(self, fingers: u8, out: &mut Vec<ScrollEvent>) -> Self {
        let id = GestureId(self.next_id);
        out.push(event(id, Phase::MayBegin, fingers, None));
        Self {
            current: Some((id, fingers, Stage::Announced)),
            next_id: self.next_id + 1,
            ..self
        }
    }
}

fn axis_of(stage: Stage) -> Option<Axis> {
    match stage {
        Stage::Began(a) => Some(a),
        _ => None,
    }
}

fn event(gesture: GestureId, phase: Phase, fingers: u8, axis: Option<Axis>) -> ScrollEvent {
    ScrollEvent {
        gesture,
        phase,
        fingers,
        axis,
        px: 0.0,
        quanta: Quanta::default(),
    }
}
