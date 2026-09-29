//! Per-touch tracking and palm/rest rejection P1-P3 (12.3.2, 12.3.3).
//!
//! Each id gets a track for the life of its contact. A track's class decides whether it counts
//! toward the contact set the gestures see. Palm and Resting are for life: a thumb resting on
//! the side stays rejected even if it later shuffles.

use super::bands::Bands;
use crate::config::{Config, ContactNibblePolicy, Rejection};
use crate::feel::Sample;
use crate::sensor::lattice::{self, Ellipse};
use crate::sensor::{FrameContact, Shape};
use crate::time::Mono;
use crate::touch::{Point, TouchId, TouchState};

/// A frame contact on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Touch {
    id: TouchId,
    at: Point,
    shape: Shape,
}

/// How a contact is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contact {
    /// P1: small; continues a gesture it is in, never starts or joins one.
    Light,
    /// Counts.
    Firm,
    /// P2: landed in a band; undecided until it moves or its window passes.
    Watched,
    /// P2: sat still in a band. Rejected for life.
    Resting,
    /// P3: too large to be a finger. Rejected for life.
    Palm,
}

/// Whether a contact of the running set turned Palm this report (it cancels the scroll).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PalmTurn {
    None,
    InSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Track {
    id: TouchId,
    landed: Mono,
    origin: Point,
    at: Point,
    class: Contact,
}

/// Every live track and the contact set of the previous report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contacts {
    tracks: Vec<Track>,
    set: Vec<TouchId>,
}

/// One report's view: the contacts the gestures see, in report order, and any palm turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub samples: Vec<Sample>,
    pub palm: PalmTurn,
}

/// Whether the frame's state makes this touch a contact at all (12.3.2): the state nibble, and
/// on the Magic Mouse a non-zero size.
pub fn is_contact(t: &FrameContact, policy: ContactNibblePolicy) -> bool {
    let counted = match policy {
        ContactNibblePolicy::OneToFour => (1..=4).contains(&t.state.nibble()),
        ContactNibblePolicy::KernelOnly => {
            matches!(t.state, TouchState::MakeTouch | TouchState::Touching)
        }
    };
    let sized = match t.shape {
        Shape::MagicMouse(m) => m.size > 0,
        Shape::Unmeasured => true,
    };
    counted && sized
}

impl Contacts {
    /// The ids counted in the last report, sorted.
    pub fn set(&self) -> &[TouchId] {
        &self.set
    }

    /// Consumes one frame's contacts, with the bands of the surface they are on.
    pub fn step(
        self,
        touches: &[FrameContact],
        now: Mono,
        cfg: &Config,
        bands: &Bands,
    ) -> (Self, Seen) {
        let live: Vec<Touch> = touches
            .iter()
            .filter(|t| is_contact(t, cfg.gestures.contact_nibble_policy))
            .map(|t| Touch {
                id: t.id,
                at: lattice::point(t.at),
                shape: t.shape,
            })
            .collect();
        let tracks: Vec<Track> = live
            .iter()
            .map(|t| match self.tracks.iter().find(|k| k.id == t.id) {
                Some(k) => advance(*k, t, now, cfg),
                None => land(t, now, cfg, bands),
            })
            .collect();
        let counted = |k: &Track| match k.class {
            Contact::Firm => true,
            Contact::Light => self.set.contains(&k.id),
            Contact::Watched | Contact::Resting | Contact::Palm => false,
        };
        let samples: Vec<Sample> = tracks
            .iter()
            .filter(|k| counted(k))
            .map(|k| Sample { id: k.id, at: k.at })
            .collect();
        let palm = if tracks
            .iter()
            .any(|k| k.class == Contact::Palm && self.set.contains(&k.id))
        {
            PalmTurn::InSet
        } else {
            PalmTurn::None
        };
        let mut set: Vec<TouchId> = samples.iter().map(|s| s.id).collect();
        set.sort();
        (Self { tracks, set }, Seen { samples, palm })
    }
}

fn land(t: &Touch, now: Mono, cfg: &Config, bands: &Bands) -> Track {
    let k = Track {
        id: t.id,
        landed: now,
        origin: t.at,
        at: t.at,
        class: Contact::Firm,
    };
    let class = match cfg.rejection {
        Rejection::Off => Contact::Firm,
        Rejection::On if is_palm(t, cfg) => Contact::Palm,
        Rejection::On if bands.contains(t.at) => Contact::Watched,
        Rejection::On => by_size(t, cfg),
    };
    Track { class, ..k }
}

fn advance(k: Track, t: &Touch, now: Mono, cfg: &Config) -> Track {
    let k = Track { at: t.at, ..k };
    let class = match (cfg.rejection, k.class) {
        (Rejection::Off, _) => Contact::Firm,
        (_, Contact::Palm) | (_, Contact::Resting) => k.class,
        _ if is_palm(t, cfg) => Contact::Palm,
        (_, Contact::Watched) => watched(k, t, now, cfg),
        _ => by_size(t, cfg),
    };
    Track { class, ..k }
}

/// P2: a watched contact that moves far enough is a finger; one that sits out its window rests.
fn watched(k: Track, t: &Touch, now: Mono, cfg: &Config) -> Contact {
    let p = &cfg.palm_rejection;
    let floor = Ellipse::of(p.resting_move_floor_x_um, p.resting_move_floor_y_um);
    if floor.reached(k.at.delta_from(k.origin)) {
        by_size(t, cfg)
    } else if now.since(k.landed) >= p.resting_window_ms.duration() {
        Contact::Resting
    } else {
        Contact::Watched
    }
}

/// P1 on the Magic Mouse's size; a sensor without one reports firm touches only.
fn by_size(t: &Touch, cfg: &Config) -> Contact {
    match t.shape {
        Shape::MagicMouse(m) if i32::from(m.size) < cfg.palm_rejection.light_touch_size_max.0 => {
            Contact::Light
        }
        Shape::MagicMouse(_) | Shape::Unmeasured => Contact::Firm,
    }
}

/// P3 on the Magic Mouse's major axis; a sensor without one never sees a palm.
fn is_palm(t: &Touch, cfg: &Config) -> bool {
    match t.shape {
        Shape::MagicMouse(m) => {
            i32::from(m.major) >= cfg.palm_rejection.large_contact_major_units.0
        }
        Shape::Unmeasured => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sensor::MagicShape;
    use crate::sensor::magic_mouse::{self, RESOLUTION};

    fn raw(id: u8, x: i32, y: i32, size: u8, major: u8) -> FrameContact {
        FrameContact {
            id: TouchId(id),
            at: RESOLUTION.point(x, y),
            state: TouchState::Touching,
            shape: Shape::MagicMouse(MagicShape {
                major,
                minor: 20,
                size,
                orientation: 0,
            }),
        }
    }

    fn bands(cfg: &Config) -> Bands {
        Bands::resolve(&cfg.palm_rejection, &magic_mouse::surface())
    }

    fn ids(seen: &Seen) -> Vec<u8> {
        seen.samples.iter().map(|s| s.id.0).collect()
    }

    fn run(cfg: &Config, frames: &[(u64, Vec<FrameContact>)]) -> Vec<Vec<u8>> {
        let mut c = Contacts::default();
        let b = bands(cfg);
        frames
            .iter()
            .map(|(ms, t)| {
                let (n, seen) = c.clone().step(t, Mono::from_millis(*ms), cfg, &b);
                c = n;
                ids(&seen)
            })
            .collect()
    }

    #[test]
    fn nibble_policy_table() {
        let mut t = raw(1, 0, 0, 10, 20);
        const CASES: &[(u8, bool, bool)] = &[
            (0, false, false),
            (1, true, false),
            (2, true, false),
            (3, true, true),
            (4, true, true),
            (5, false, false),
            (7, false, false),
        ];
        for &(n, python, kernel) in CASES {
            t.state = TouchState::from_nibble(n);
            assert_eq!(
                is_contact(&t, ContactNibblePolicy::OneToFour),
                python,
                "nibble {n}"
            );
            assert_eq!(
                is_contact(&t, ContactNibblePolicy::KernelOnly),
                kernel,
                "nibble {n}"
            );
        }
        t.state = TouchState::Touching;
        t.shape = Shape::MagicMouse(MagicShape {
            size: 0,
            ..shape(&t)
        });
        assert!(
            !is_contact(&t, ContactNibblePolicy::OneToFour),
            "size 0 is not a contact"
        );
        t.shape = Shape::Unmeasured;
        assert!(
            is_contact(&t, ContactNibblePolicy::KernelOnly),
            "an unmeasured touch is a contact"
        );
    }

    fn shape(t: &FrameContact) -> MagicShape {
        match t.shape {
            Shape::MagicMouse(m) => m,
            Shape::Unmeasured => panic!("a Magic Mouse touch"),
        }
    }

    #[test]
    fn light_touch_never_joins_but_may_continue() {
        let cfg = Config::default();
        let got = run(
            &cfg,
            &[
                (0, vec![raw(1, 0, 0, 5, 20)]),
                (10, vec![raw(1, 0, 0, 12, 20)]),
                (20, vec![raw(1, 0, 0, 4, 20)]),
                (30, vec![raw(1, 0, 0, 4, 20), raw(2, 300, 0, 4, 20)]),
            ],
        );
        assert_eq!(got, [vec![], vec![1], vec![1], vec![1]]);
    }

    #[test]
    fn a_still_contact_in_the_side_band_rests() {
        let cfg = Config::default();
        let got = run(
            &cfg,
            &[
                (0, vec![raw(1, -1000, 0, 12, 20), raw(2, 0, 0, 12, 20)]),
                (100, vec![raw(1, -1005, 5, 12, 20), raw(2, 0, 50, 12, 20)]),
                (160, vec![raw(1, -1005, 5, 12, 20), raw(2, 0, 100, 12, 20)]),
                (400, vec![raw(1, -900, 100, 12, 20), raw(2, 0, 150, 12, 20)]),
            ],
        );
        assert_eq!(
            got,
            [vec![2], vec![2], vec![2], vec![2]],
            "rested for life, even after moving"
        );
    }

    #[test]
    fn a_moving_contact_in_a_band_is_a_finger() {
        let cfg = Config::default();
        let got = run(
            &cfg,
            &[
                (0, vec![raw(1, 1100, 0, 12, 20)]),
                (50, vec![raw(1, 1100, 40, 12, 20)]),
            ],
        );
        assert_eq!(got, [vec![], vec![1]]);
    }

    #[test]
    fn palm_is_rejected_and_reported_when_it_was_counted() {
        let cfg = Config::default();
        let b = bands(&cfg);
        let mut c = Contacts::default();
        let (n, _) = c.step(&[raw(1, 0, 0, 12, 20)], Mono(0), &cfg, &b);
        c = n;
        let (n, seen) = c.step(&[raw(1, 0, 0, 12, 70)], Mono::from_millis(10), &cfg, &b);
        assert_eq!(seen.palm, PalmTurn::InSet);
        assert!(seen.samples.is_empty());
        let (_, seen) = n.step(&[raw(1, 0, 0, 12, 20)], Mono::from_millis(20), &cfg, &b);
        assert!(seen.samples.is_empty(), "palm for life");
        assert_eq!(seen.palm, PalmTurn::None, "reported once");
    }

    #[test]
    fn rejection_off_counts_everything() {
        let cfg = Config {
            rejection: Rejection::Off,
            ..Config::default()
        };
        let got = run(
            &cfg,
            &[(0, vec![raw(1, -1000, 0, 2, 90), raw(2, 0, 2000, 12, 20)])],
        );
        assert_eq!(got, [vec![1, 2]]);
    }

    #[test]
    fn a_sensor_without_shape_counts_firm_and_never_palm() {
        let cfg = Config::default();
        let bare = |id: u8, x: i32| FrameContact {
            shape: Shape::Unmeasured,
            ..raw(id, x, 0, 0, 0)
        };
        let got = run(&cfg, &[(0, vec![bare(1, 0), bare(2, 300)])]);
        assert_eq!(got, [vec![1, 2]]);
    }
}
