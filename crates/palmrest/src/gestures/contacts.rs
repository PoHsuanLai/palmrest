//! Per-touch tracking and palm/rest rejection P1-P3 (12.3.2, 12.3.3).
//!
//! Each id gets a track for the life of its contact. A track's class decides whether it counts
//! toward the contact set the gestures see. Palm and Resting are for life: a thumb resting on
//! the side stays rejected even if it later shuffles.

use crate::config::{Config, ContactNibblePolicy, Rejection};
use crate::feel::Sample;
use crate::time::Mono;
use crate::touch::{Point, RawTouch, TouchId, TouchState};

/// The kernel's y span, used to place the rear band (R16).
pub const Y_MIN: i32 = -1589;
pub const Y_MAX: i32 = 2047;

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

/// Whether the report's state nibble makes this touch a contact at all (12.3.2).
pub fn is_contact(t: &RawTouch, policy: ContactNibblePolicy) -> bool {
    let counted = match policy {
        ContactNibblePolicy::PythonParity => (1..=4).contains(&t.state.nibble()),
        ContactNibblePolicy::KernelOnly => {
            matches!(t.state, TouchState::MakeTouch | TouchState::Touching)
        }
    };
    counted && t.size > 0
}

impl Contacts {
    /// The ids counted in the last report, sorted.
    pub fn set(&self) -> &[TouchId] {
        &self.set
    }

    /// Consumes one report's touches.
    pub fn step(self, touches: &[RawTouch], now: Mono, cfg: &Config) -> (Self, Seen) {
        let live: Vec<&RawTouch> = touches
            .iter()
            .filter(|t| is_contact(t, cfg.gestures.contact_nibble_policy))
            .collect();
        let tracks: Vec<Track> = live
            .iter()
            .map(|t| match self.tracks.iter().find(|k| k.id == t.id) {
                Some(k) => advance(*k, t, now, cfg),
                None => land(t, now, cfg),
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

fn land(t: &RawTouch, now: Mono, cfg: &Config) -> Track {
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
        Rejection::On if in_band(t.at, cfg) => Contact::Watched,
        Rejection::On => by_size(t, cfg),
    };
    Track { class, ..k }
}

fn advance(k: Track, t: &RawTouch, now: Mono, cfg: &Config) -> Track {
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
fn watched(k: Track, t: &RawTouch, now: Mono, cfg: &Config) -> Contact {
    let p = &cfg.palm_rejection;
    let moved = k.at.delta_from(k.origin).length();
    if moved >= f64::from(p.resting_move_floor_units.0) {
        by_size(t, cfg)
    } else if now.since(k.landed) >= p.resting_window_ms.duration() {
        Contact::Resting
    } else {
        Contact::Watched
    }
}

fn by_size(t: &RawTouch, cfg: &Config) -> Contact {
    if i32::from(t.size) < cfg.palm_rejection.light_touch_size_max.0 {
        Contact::Light
    } else {
        Contact::Firm
    }
}

fn is_palm(t: &RawTouch, cfg: &Config) -> bool {
    i32::from(t.major) >= cfg.palm_rejection.large_contact_major_units.0
}

/// P2's bands: the two sides and the rear (the +y end, toward the palm).
pub fn in_band(at: Point, cfg: &Config) -> bool {
    let p = &cfg.palm_rejection;
    let rear_from = f64::from(Y_MAX) - p.rear_band_percent.share() * f64::from(Y_MAX - Y_MIN);
    at.x < p.side_band_x_min.0 || at.x > p.side_band_x_max.0 || f64::from(at.y) > rear_from
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(id: u8, x: i32, y: i32, size: u8, major: u8) -> RawTouch {
        RawTouch {
            id: TouchId(id),
            at: Point::new(x, y),
            major,
            minor: 20,
            size,
            orientation: 0,
            state: TouchState::Touching,
        }
    }

    fn ids(seen: &Seen) -> Vec<u8> {
        seen.samples.iter().map(|s| s.id.0).collect()
    }

    fn run(cfg: &Config, frames: &[(u64, Vec<RawTouch>)]) -> Vec<Vec<u8>> {
        let mut c = Contacts::default();
        frames
            .iter()
            .map(|(ms, t)| {
                let (n, seen) = c.clone().step(t, Mono::from_millis(*ms), cfg);
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
                is_contact(&t, ContactNibblePolicy::PythonParity),
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
        t.size = 0;
        assert!(
            !is_contact(&t, ContactNibblePolicy::PythonParity),
            "size 0 is not a contact"
        );
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
        let mut c = Contacts::default();
        let (n, _) = c.step(&[raw(1, 0, 0, 12, 20)], Mono(0), &cfg);
        c = n;
        let (n, seen) = c.step(&[raw(1, 0, 0, 12, 70)], Mono::from_millis(10), &cfg);
        assert_eq!(seen.palm, PalmTurn::InSet);
        assert!(seen.samples.is_empty());
        let (_, seen) = n.step(&[raw(1, 0, 0, 12, 20)], Mono::from_millis(20), &cfg);
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
    fn band_edges() {
        let cfg = Config::default();
        const CASES: &[(i32, i32, bool)] = &[
            (-900, 0, false),
            (-901, 0, true),
            (1060, 0, false),
            (1061, 0, true),
            (0, 1501, false),
            (0, 1502, true),
            (0, -1589, false),
        ];
        for &(x, y, want) in CASES {
            assert_eq!(in_band(Point::new(x, y), &cfg), want, "({x}, {y})");
        }
    }
}
