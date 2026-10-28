//! G6/G7: one- and two-finger double-taps (12.4 "Tap").
//!
//! A tap is a touchdown whose peak contact count is 1 or 2, that lifts completely within
//! `MAX`, in which no contact moves out of the `MAX_MOVE` ellipse, with no click. A double-tap
//! is a second tap of the same count landing within `GAP` of the first lifting and inside the
//! `CENTROID_MAX` ellipse around where it was. Taps never scroll: their motion stays under the
//! lock threshold.

use crate::config::{Ms, Um};
use crate::feel::{Hold, Sample};
use crate::sensor::lattice::Ellipse;
use crate::time::Mono;
use crate::touch::Point;
use palmrest_protocol::TapKind;

/// The longest touch that still counts as a tap.
const MAX: Ms = Ms(200);
/// The longest pause between a double-tap's taps.
const GAP: Ms = Ms(350);
/// No contact of a tap moves this far from where it landed (an ellipse: sideways, lengthways).
const MAX_MOVE: (Um, Um) = (Um(1538), Um(571));
/// The second tap lands this close to the first (an ellipse: sideways, lengthways).
const CENTROID_MAX: (Um, Um) = (Um(11538), Um(4286));

/// A touchdown in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct Touchdown {
    since: Mono,
    peak: usize,
    count: usize,
    /// Whether a finger has lifted since landing.
    shrunk: bool,
    origins: Vec<Sample>,
    centroid: Point,
    /// Whether this touchdown can still be a tap.
    valid: bool,
}

/// A completed first tap waiting for its second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    fingers: usize,
    lifted: Mono,
    origin: Point,
}

/// The tap detector between reports.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TapG {
    #[default]
    Idle,
    Down {
        first: Option<Gap>,
        td: Touchdown,
    },
    Gap(Gap),
}

impl TapG {
    /// Consumes one report's contacts; returns a double-tap when one completes.
    pub fn step(self, contacts: &[Sample], now: Mono, hold: Hold) -> (Self, Option<TapKind>) {
        match (self, contacts.is_empty()) {
            (Self::Down { first, td }, true) => lift(first, td, now),
            (Self::Down { first, td }, false) => (
                Self::Down {
                    first,
                    td: td.update(contacts, now, hold),
                },
                None,
            ),
            (Self::Gap(g), false) => (
                Self::Down {
                    first: live(g, now),
                    td: Touchdown::land(contacts, now, hold),
                },
                None,
            ),
            (Self::Idle, false) => (
                Self::Down {
                    first: None,
                    td: Touchdown::land(contacts, now, hold),
                },
                None,
            ),
            (Self::Gap(g), true) => (live(g, now).map_or(Self::Idle, Self::Gap), None),
            (Self::Idle, true) => (Self::Idle, None),
        }
    }
}

fn live(g: Gap, now: Mono) -> Option<Gap> {
    (now.since(g.lifted) <= GAP.duration()).then_some(g)
}

fn lift(first: Option<Gap>, td: Touchdown, now: Mono) -> (TapG, Option<TapKind>) {
    if !td.is_tap(now) {
        return (TapG::Idle, None);
    }
    let second = first.filter(|g| {
        g.fingers == td.peak
            && td.since.since(g.lifted) <= GAP.duration()
            && Ellipse::of(CENTROID_MAX.0, CENTROID_MAX.1)
                .contains(td.centroid.delta_from(g.origin))
    });
    match (second, td.peak) {
        (Some(_), 1) => (TapG::Idle, Some(TapKind::One)),
        (Some(_), _) => (TapG::Idle, Some(TapKind::Two)),
        (None, _) => (
            TapG::Gap(Gap {
                fingers: td.peak,
                lifted: now,
                origin: td.centroid,
            }),
            None,
        ),
    }
}

impl Touchdown {
    fn land(contacts: &[Sample], now: Mono, hold: Hold) -> Self {
        Self {
            since: now,
            peak: contacts.len(),
            count: contacts.len(),
            shrunk: false,
            origins: contacts.to_vec(),
            centroid: centroid(contacts),
            valid: hold != Hold::Held,
        }
    }

    fn update(mut self, contacts: &[Sample], now: Mono, hold: Hold) -> Self {
        let n = contacts.len();
        let regrew = self.shrunk && n > self.count;
        if n < self.count {
            self.shrunk = true;
        }
        for c in contacts {
            if !self.origins.iter().any(|o| o.id == c.id) {
                self.origins.push(*c);
            }
        }
        if n > self.peak {
            self.peak = n;
            self.centroid = centroid(contacts);
        }
        self.count = n;
        let max_move = Ellipse::of(MAX_MOVE.0, MAX_MOVE.1);
        let moved = contacts.iter().any(|c| {
            let o = self
                .origins
                .iter()
                .find(|o| o.id == c.id)
                .map_or(c.at, |o| o.at);
            max_move.reached(c.at.delta_from(o))
        });
        let late = now.since(self.since) > MAX.duration();
        if moved || late || regrew || hold == Hold::Held || self.peak > 2 {
            self.valid = false;
        }
        self
    }

    fn is_tap(&self, lift: Mono) -> bool {
        self.valid && (1..=2).contains(&self.peak) && lift.since(self.since) <= MAX.duration()
    }
}

fn centroid(contacts: &[Sample]) -> Point {
    let n = contacts.len().max(1) as i32;
    let (sx, sy) = contacts
        .iter()
        .fold((0, 0), |(x, y), c| (x + c.at.x, y + c.at.y));
    Point::new(sx / n, sy / n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::touch::TouchId;

    fn fingers(n: u8, x: i32) -> Vec<Sample> {
        (0..n)
            .map(|i| Sample {
                id: TouchId(i),
                at: Point::new(x + i32::from(i) * 500, 0),
            })
            .collect()
    }

    /// Two taps of `n` fingers, `down` ms each, `gap` ms apart, the second shifted by `shift`.
    fn double(n: u8, down: u64, gap: u64, shift: i32) -> Vec<Option<TapKind>> {
        let frames = [
            (0, fingers(n, 0)),
            (down, vec![]),
            (down + gap, fingers(n, shift)),
            (2 * down + gap, vec![]),
        ];
        let mut t = TapG::Idle;
        frames
            .iter()
            .map(|(ms, c)| {
                let (n, k) = t.clone().step(c, Mono::from_millis(*ms), Hold::Open);
                t = n;
                k
            })
            .collect()
    }

    #[test]
    fn double_tap_table() {
        /// fingers, down ms, gap ms, second-tap shift, expected, name.
        type Case = (u8, u64, u64, i32, Option<TapKind>, &'static str);
        const CASES: &[Case] = &[
            (2, 120, 250, 0, Some(TapKind::Two), "two fingers"),
            (1, 120, 250, 0, Some(TapKind::One), "one finger"),
            (2, 120, 400, 0, None, "gap too long"),
            (2, 250, 100, 0, None, "held too long"),
            (2, 120, 250, 350, None, "second tap elsewhere"),
            (3, 120, 250, 0, None, "three fingers"),
        ];
        for &(n, down, gap, shift, want, name) in CASES {
            let got = double(n, down, gap, shift);
            assert_eq!(&got[..3], &[None, None, None], "{name}");
            assert_eq!(got[3], want, "{name}");
        }
    }

    #[test]
    fn staggered_landing_uses_the_peak_centroid() {
        let one = fingers(1, 0);
        let two = fingers(2, 0);
        let frames = [
            (0, one.clone()),
            (10, two.clone()),
            (100, vec![]),
            (300, one),
            (310, two),
            (400, vec![]),
        ];
        let got: Vec<_> = frames
            .iter()
            .scan(TapG::Idle, |t, (ms, c)| {
                let (n, k) = t.clone().step(c, Mono::from_millis(*ms), Hold::Open);
                *t = n;
                Some(k)
            })
            .collect();
        assert_eq!(got.last(), Some(&Some(TapKind::Two)));
    }
}
