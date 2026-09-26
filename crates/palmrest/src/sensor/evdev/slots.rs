//! Multitouch protocol B as a value: the slots, the buttons, the motion since the last frame.

use super::event::{BTN_RIGHT, REL_X, REL_Y, SYN_DROPPED, TimedEvent};
use crate::outputs::events::{
    ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_SLOT, ABS_MT_TRACKING_ID, BTN_LEFT, EV_ABS,
    EV_KEY, EV_REL, EV_SYN, SYN_REPORT,
};
use crate::sensor::frame::{Buttons, Frame, FrameContact, FrameStamp, Motion, Shape};
use crate::sensor::units::Resolution;
use crate::touch::{TouchId, TouchState};

/// Whether a slot holds a contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tracking {
    Lifted,
    Live(i32),
}

/// One slot. The position outlives the contact: the kernel does not resend a value that did
/// not change, even for a new contact in the same slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    tracking: Tracking,
    x: i32,
    y: i32,
}

const EMPTY: Slot = Slot {
    tracking: Tracking::Lifted,
    x: 0,
    y: 0,
};

/// Whether events are being dropped until the next `SYN_REPORT` (after `SYN_DROPPED`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sync {
    InStep,
    Dropping,
}

/// Everything between two `SYN_REPORT`s.
#[derive(Debug, Clone, PartialEq)]
pub struct Slots {
    slots: Vec<Slot>,
    /// The slot position events apply to; `None` after a slot number out of range.
    current: Option<usize>,
    buttons: Buttons,
    motion: Motion,
    sync: Sync,
}

impl Slots {
    pub fn new(n: usize) -> Self {
        Self {
            slots: vec![EMPTY; n],
            current: Some(0),
            buttons: Buttons::NONE,
            motion: Motion::default(),
            sync: Sync::InStep,
        }
    }

    /// Applies one event; a `SYN_REPORT` completes a frame.
    pub fn apply(self, e: &TimedEvent, res: Resolution) -> (Self, Option<Frame>) {
        let ev = e.event;
        match (self.sync, ev.kind, ev.code) {
            (_, EV_SYN, SYN_DROPPED) => (
                Self {
                    sync: Sync::Dropping,
                    ..self
                },
                None,
            ),
            (Sync::Dropping, EV_SYN, SYN_REPORT) => (self.resynced(), None),
            (Sync::Dropping, _, _) => (self, None),
            (Sync::InStep, EV_SYN, SYN_REPORT) => self.frame(e, res),
            (Sync::InStep, EV_ABS, code) => (self.abs(code, ev.value), None),
            (Sync::InStep, EV_KEY, code) => (self.key(code, ev.value), None),
            (Sync::InStep, EV_REL, code) => (self.rel(code, ev.value), None),
            (Sync::InStep, _, _) => (self, None),
        }
    }

    /// After a drop: every contact is taken as lifted and the next frame starts clean.
    fn resynced(self) -> Self {
        let slots = self
            .slots
            .iter()
            .map(|s| Slot {
                tracking: Tracking::Lifted,
                ..*s
            })
            .collect();
        Self {
            slots,
            motion: Motion::default(),
            sync: Sync::InStep,
            ..self
        }
    }

    fn abs(mut self, code: u16, value: i32) -> Self {
        if code == ABS_MT_SLOT {
            self.current = usize::try_from(value)
                .ok()
                .filter(|&i| i < self.slots.len());
            return self;
        }
        let Some(slot) = self.current.and_then(|i| self.slots.get_mut(i)) else {
            return self;
        };
        match code {
            ABS_MT_TRACKING_ID if value < 0 => slot.tracking = Tracking::Lifted,
            ABS_MT_TRACKING_ID => slot.tracking = Tracking::Live(value),
            ABS_MT_POSITION_X => slot.x = value,
            ABS_MT_POSITION_Y => slot.y = value,
            _ => {}
        }
        self
    }

    fn key(self, code: u16, value: i32) -> Self {
        let bit = match code {
            BTN_LEFT => Buttons::LEFT.0,
            BTN_RIGHT => Buttons::RIGHT.0,
            _ => return self,
        };
        let buttons = if value == 0 {
            Buttons(self.buttons.0 & !bit)
        } else {
            Buttons(self.buttons.0 | bit)
        };
        Self { buttons, ..self }
    }

    fn rel(self, code: u16, value: i32) -> Self {
        let add = |a: i16| {
            i16::try_from(i32::from(a).saturating_add(value).clamp(-32768, 32767)).unwrap_or(a)
        };
        let m = self.motion;
        let motion = match code {
            REL_X => Motion { dx: add(m.dx), ..m },
            REL_Y => Motion { dy: add(m.dy), ..m },
            _ => m,
        };
        Self { motion, ..self }
    }

    fn frame(self, e: &TimedEvent, res: Resolution) -> (Self, Option<Frame>) {
        let contacts = self
            .slots
            .iter()
            .filter_map(|s| match s.tracking {
                Tracking::Live(id) => Some(FrameContact {
                    id: TouchId((id & 0xff) as u8),
                    at: res.point(s.x, s.y),
                    state: TouchState::Touching,
                    shape: Shape::Unmeasured,
                }),
                Tracking::Lifted => None,
            })
            .collect();
        let frame = Frame {
            buttons: self.buttons,
            motion: self.motion,
            contacts,
            stamp: FrameStamp::Kernel(e.at),
        };
        (
            Self {
                motion: Motion::default(),
                ..self
            },
            Some(frame),
        )
    }
}
