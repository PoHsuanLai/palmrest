//! Output frames as evdev events: the pure half of the uinput emitter.
//!
//! Kept apart from the device so the exact event sequence of every frame is tested without
//! creating a device; the emitter only writes these bytes.

use super::pad::PadFrame;
use crate::keys::KeyCombo;

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_REL: u16 = 0x02;
pub const EV_ABS: u16 = 0x03;
pub const SYN_REPORT: u16 = 0;
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_MT_SLOT: u16 = 0x2f;
pub const ABS_MT_POSITION_X: u16 = 0x35;
pub const ABS_MT_POSITION_Y: u16 = 0x36;
pub const ABS_MT_TRACKING_ID: u16 = 0x39;
pub const BTN_LEFT: u16 = 0x110;
pub const BTN_TOOL_FINGER: u16 = 0x145;
pub const BTN_TOUCH: u16 = 0x14a;
pub const BTN_TOOL_DOUBLETAP: u16 = 0x14d;
pub const BTN_TOOL_TRIPLETAP: u16 = 0x14e;
pub const BTN_TOOL_QUADTAP: u16 = 0x14f;
/// The most contacts the virtual touchpad has slots for.
pub const PAD_SLOTS: u8 = 5;

/// One `struct input_event` without its timestamp (the kernel stamps uinput writes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

const fn ev(kind: u16, code: u16, value: i32) -> InputEvent {
    InputEvent { kind, code, value }
}

const SYN: InputEvent = ev(EV_SYN, SYN_REPORT, 0);

impl InputEvent {
    /// The 24-byte `struct input_event` of a 64-bit kernel, zero time.
    pub fn to_bytes(self) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[16..18].copy_from_slice(&self.kind.to_le_bytes());
        b[18..20].copy_from_slice(&self.code.to_le_bytes());
        b[20..24].copy_from_slice(&self.value.to_le_bytes());
        b
    }
}

/// A key combination: modifiers then key down, report, then up in reverse, report.
pub fn key_events(k: &KeyCombo) -> Vec<InputEvent> {
    let codes: Vec<u16> = k.codes().collect();
    let down = codes.iter().map(|&c| ev(EV_KEY, c, 1));
    let up = codes.iter().rev().map(|&c| ev(EV_KEY, c, 0));
    down.chain([SYN]).chain(up).chain([SYN]).collect()
}

/// The multitouch (protocol B) state of the virtual touchpad between frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PadEncoder {
    fingers: u8,
    spacing: i32,
    next_tracking_id: i32,
}

impl PadEncoder {
    /// The events of one frame.
    pub fn encode(self, f: &PadFrame) -> (Self, Vec<InputEvent>) {
        match *f {
            PadFrame::Land {
                fingers,
                at,
                spacing,
            } => {
                let fingers = fingers.clamp(2, PAD_SLOTS);
                let s = Self {
                    fingers,
                    spacing,
                    next_tracking_id: self.next_tracking_id.wrapping_add(i32::from(fingers))
                        & 0xffff,
                };
                let mut out = Vec::new();
                for (slot, x) in s.xs(at.x) {
                    out.extend([
                        ev(EV_ABS, ABS_MT_SLOT, slot),
                        ev(
                            EV_ABS,
                            ABS_MT_TRACKING_ID,
                            (self.next_tracking_id + slot) & 0xffff,
                        ),
                        ev(EV_ABS, ABS_MT_POSITION_X, x),
                        ev(EV_ABS, ABS_MT_POSITION_Y, at.y),
                    ]);
                }
                out.extend([ev(EV_KEY, BTN_TOUCH, 1), ev(EV_KEY, tool(fingers), 1)]);
                out.extend(s.single(at.x, at.y));
                (s, out)
            }
            PadFrame::Move { at } => {
                let mut out = Vec::new();
                for (slot, x) in self.xs(at.x) {
                    out.extend([
                        ev(EV_ABS, ABS_MT_SLOT, slot),
                        ev(EV_ABS, ABS_MT_POSITION_X, x),
                        ev(EV_ABS, ABS_MT_POSITION_Y, at.y),
                    ]);
                }
                out.extend(self.single(at.x, at.y));
                (self, out)
            }
            PadFrame::Lift => {
                let mut out = Vec::new();
                for slot in 0..i32::from(self.fingers) {
                    out.extend([
                        ev(EV_ABS, ABS_MT_SLOT, slot),
                        ev(EV_ABS, ABS_MT_TRACKING_ID, -1),
                    ]);
                }
                if self.fingers > 0 {
                    out.extend([
                        ev(EV_KEY, BTN_TOUCH, 0),
                        ev(EV_KEY, tool(self.fingers), 0),
                        SYN,
                    ]);
                }
                (Self { fingers: 0, ..self }, out)
            }
        }
    }

    /// Slot numbers and x positions: contacts spread evenly around the centre.
    fn xs(self, centre: i32) -> impl Iterator<Item = (i32, i32)> {
        let n = i32::from(self.fingers);
        let spacing = self.spacing;
        (0..n).map(move |i| (i, centre + (2 * i - (n - 1)) * spacing / 2))
    }

    /// The single-touch axes follow slot 0, then the report.
    fn single(self, centre: i32, y: i32) -> [InputEvent; 3] {
        let x0 = self.xs(centre).next().map_or(centre, |(_, x)| x);
        [ev(EV_ABS, ABS_X, x0), ev(EV_ABS, ABS_Y, y), SYN]
    }
}

fn tool(fingers: u8) -> u16 {
    match fingers {
        0 | 1 => BTN_TOOL_FINGER,
        2 => BTN_TOOL_DOUBLETAP,
        3 => BTN_TOOL_TRIPLETAP,
        _ => BTN_TOOL_QUADTAP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Key;
    use crate::outputs::pad::{CENTRE, PadPoint};

    #[test]
    fn keys_press_and_release_in_order() {
        let got = key_events(&KeyCombo::alt(Key::LEFT));
        assert_eq!(
            got,
            [
                ev(EV_KEY, 56, 1),
                ev(EV_KEY, 105, 1),
                SYN,
                ev(EV_KEY, 105, 0),
                ev(EV_KEY, 56, 0),
                SYN
            ]
        );
    }

    #[test]
    fn two_contacts_never_one_and_both_move_together() {
        let (p, land) = PadEncoder::default().encode(&PadFrame::Land {
            fingers: 2,
            at: CENTRE,
            spacing: 800,
        });
        let xs: Vec<i32> = land
            .iter()
            .filter(|e| e.code == ABS_MT_POSITION_X)
            .map(|e| e.value)
            .collect();
        assert_eq!(xs, [1600, 2400]);
        assert!(land.contains(&ev(EV_KEY, BTN_TOOL_DOUBLETAP, 1)));
        assert!(!land.iter().any(|e| e.code == BTN_LEFT), "never a click");
        let (p, mv) = p.encode(&PadFrame::Move {
            at: PadPoint { x: 2000, y: 1500 },
        });
        let ys: Vec<i32> = mv
            .iter()
            .filter(|e| e.code == ABS_MT_POSITION_Y)
            .map(|e| e.value)
            .collect();
        assert_eq!(ys, [1500, 1500], "identical delta: no pinch");
        let (p, lift) = p.encode(&PadFrame::Lift);
        assert_eq!(
            lift.iter()
                .filter(|e| e.code == ABS_MT_TRACKING_ID && e.value == -1)
                .count(),
            2
        );
        assert_eq!(lift.last(), Some(&SYN));
        let (_, again) = p.encode(&PadFrame::Lift);
        assert!(again.is_empty(), "lifting nothing writes nothing");
    }

    #[test]
    fn event_bytes_layout() {
        let b = ev(EV_KEY, BTN_LEFT, -1).to_bytes();
        assert_eq!(&b[..16], &[0; 16]);
        assert_eq!(&b[16..], &[1, 0, 0x10, 0x01, 0xff, 0xff, 0xff, 0xff]);
    }
}
