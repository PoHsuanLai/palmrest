//! O2: the scroll stream as two-finger strokes on a virtual touchpad (12.3.9).
//!
//! Foreign clients get momentum and their own scroll feel from libinput's two-finger scroll, so
//! the contacts must look like a real two-finger scroll and never like anything else: never one
//! contact (that would move the pointer), both contacts moved by the identical delta (never a
//! pinch), landing at Began (never at MayBegin, so a resting finger makes nothing), and a short
//! or tiny stroke held down until `min_down_ms` so libinput's tap timeout (180 ms) has passed.

use crate::config::TouchpadSettings;
use crate::time::Mono;
use serde::{Deserialize, Serialize};

/// Pad units per millimetre (resolution of ABS_MT_POSITION_*).
pub const UNITS_PER_MM: i32 = 40;
/// The pad's extent in units (100 x 70 mm).
pub const PAD_X_MAX: i32 = 4000;
pub const PAD_Y_MAX: i32 = 2800;
/// Contacts keep this far from an edge; past it the stroke is re-landed at the centre.
pub const EDGE_MARGIN: i32 = 200;

/// The centre of the contact pair, in pad units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadPoint {
    pub x: i32,
    pub y: i32,
}

/// The pad's centre, where every stroke lands.
pub const CENTRE: PadPoint = PadPoint {
    x: PAD_X_MAX / 2,
    y: PAD_Y_MAX / 2,
};

/// One frame for the virtual touchpad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum PadFrame {
    /// `fingers` contacts land around `at`, `spacing` units apart horizontally.
    Land {
        fingers: u8,
        at: PadPoint,
        spacing: i32,
    },
    /// Every contact moves so the pair's centre is at `at`.
    Move { at: PadPoint },
    /// Every contact lifts.
    Lift,
}

/// The pad between frames.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Pad {
    #[default]
    Up,
    Down {
        landed: Mono,
        at: PadPoint,
        exact: (f64, f64),
    },
    /// The stroke ended but the lift waits until `at`.
    Lifting { at: Mono },
}

impl Pad {
    /// A scroll gesture Began: land two contacts (lifting a pending pair first).
    pub fn land(self, now: Mono, cfg: &TouchpadSettings) -> (Self, Vec<PadFrame>) {
        let (_, mut frames) = self.lift_now();
        let spacing = i32::from(cfg.contact_spacing_mm.0) * UNITS_PER_MM;
        frames.push(PadFrame::Land {
            fingers: 2,
            at: CENTRE,
            spacing,
        });
        let exact = (f64::from(CENTRE.x), f64::from(CENTRE.y));
        (
            Pad::Down {
                landed: now,
                at: CENTRE,
                exact,
            },
            frames,
        )
    }

    /// Moves the pair by a finger-direction px delta.
    pub fn moved(
        self,
        dx_px: f64,
        dy_px: f64,
        now: Mono,
        cfg: &TouchpadSettings,
    ) -> (Self, Vec<PadFrame>) {
        let Pad::Down { landed, exact, .. } = self else {
            return (self, vec![]);
        };
        let scale = cfg.mm_per_px.ratio() * f64::from(UNITS_PER_MM);
        let exact = (exact.0 + dx_px * scale, exact.1 + dy_px * scale);
        let at = PadPoint {
            x: exact.0.round() as i32,
            y: exact.1.round() as i32,
        };
        if !inside(at) {
            // Out of pad: re-land at the centre. The client sees one stroke end and the next begin.
            let (_, mut frames) = Pad::Down { landed, at, exact }.lift_now();
            let (pad, land) = Pad::Up.land(now, cfg);
            frames.extend(land);
            return (pad, frames);
        }
        (Pad::Down { landed, at, exact }, vec![PadFrame::Move { at }])
    }

    /// The gesture Ended or was Cancelled: lift now, or at `landed + min_down` if the stroke
    /// is younger than that. 12.3.9 also delays strokes that moved under 2 mm, but a stroke
    /// older than `min_down` has outlived the tap timeout whatever it moved, so both of its
    /// conditions reduce to this one.
    pub fn end(self, now: Mono, cfg: &TouchpadSettings) -> (Self, Vec<PadFrame>) {
        let Pad::Down { landed, .. } = self else {
            return (self, vec![]);
        };
        let hold_until = landed.after(cfg.min_down_ms.duration());
        if now < hold_until {
            (Pad::Lifting { at: hold_until }, vec![])
        } else {
            (Pad::Up, vec![PadFrame::Lift])
        }
    }

    /// Performs a delayed lift that has come due.
    pub fn tick(self, now: Mono) -> (Self, Vec<PadFrame>) {
        match self {
            Pad::Lifting { at } if now >= at => (Pad::Up, vec![PadFrame::Lift]),
            _ => (self, vec![]),
        }
    }

    /// Lifts whatever is down, immediately (claim changes, disconnect).
    pub fn lift_now(self) -> (Self, Vec<PadFrame>) {
        match self {
            Pad::Up => (Pad::Up, vec![]),
            _ => (Pad::Up, vec![PadFrame::Lift]),
        }
    }

    /// Where the pair's centre is, while it is down.
    pub fn at(&self) -> Option<PadPoint> {
        match self {
            Pad::Down { at, .. } => Some(*at),
            _ => None,
        }
    }

    /// When `tick` next has work.
    pub fn deadline(&self) -> Option<Mono> {
        match self {
            Pad::Lifting { at } => Some(*at),
            _ => None,
        }
    }
}

fn inside(p: PadPoint) -> bool {
    (EDGE_MARGIN..=PAD_X_MAX - EDGE_MARGIN).contains(&p.x)
        && (EDGE_MARGIN..=PAD_Y_MAX - EDGE_MARGIN).contains(&p.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> TouchpadSettings {
        TouchpadSettings::default()
    }

    #[test]
    fn a_stroke_lands_moves_identically_and_lifts() {
        let (p, f) = Pad::Up.land(Mono(0), &cfg());
        assert_eq!(
            f,
            [PadFrame::Land {
                fingers: 2,
                at: CENTRE,
                spacing: 800
            }]
        );
        let (p, f) = p.moved(0.0, 10.0, Mono::from_millis(100), &cfg());
        // 10 px * 0.25 mm/px * 40 units/mm = 100 units.
        assert_eq!(
            f,
            [PadFrame::Move {
                at: PadPoint { x: 2000, y: 1500 }
            }]
        );
        let (p, f) = p.end(Mono::from_millis(250), &cfg());
        assert_eq!((p, f), (Pad::Up, vec![PadFrame::Lift]));
    }

    #[test]
    fn a_short_stroke_holds_its_lift_past_the_tap_timeout() {
        let (p, _) = Pad::Up.land(Mono(0), &cfg());
        let (p, _) = p.moved(0.0, 20.0, Mono::from_millis(50), &cfg());
        let (p, f) = p.end(Mono::from_millis(80), &cfg());
        assert!(f.is_empty());
        assert_eq!(p.deadline(), Some(Mono::from_millis(200)));
        assert_eq!(p.tick(Mono::from_millis(199)).1, vec![]);
        assert_eq!(
            p.tick(Mono::from_millis(200)),
            (Pad::Up, vec![PadFrame::Lift])
        );
    }

    #[test]
    fn a_new_stroke_lifts_a_pending_one_first() {
        let (p, _) = Pad::Up.land(Mono(0), &cfg());
        let (p, _) = p.end(Mono::from_millis(10), &cfg());
        let (_, f) = p.land(Mono::from_millis(20), &cfg());
        assert_eq!(f[0], PadFrame::Lift);
        assert!(matches!(f[1], PadFrame::Land { .. }));
    }

    #[test]
    fn leaving_the_pad_relands_at_the_centre() {
        let (p, _) = Pad::Up.land(Mono(0), &cfg());
        let (_, f) = p.moved(0.0, 130.0, Mono::from_millis(300), &cfg());
        assert_eq!(
            f,
            [
                PadFrame::Lift,
                PadFrame::Land {
                    fingers: 2,
                    at: CENTRE,
                    spacing: 800
                }
            ]
        );
    }
}
