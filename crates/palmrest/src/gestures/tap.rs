//! G6/G7: one- and two-finger double-taps (12.4 "Tap").
//!
//! A tap is a touchdown whose peak contact count is 1 or 2, that lifts completely within
//! `tap.max_ms`, in which no contact moves out of the `tap.max_move_*_um` ellipse, with no
//! click. A double-tap is a second tap of the same count landing within `tap.gap_ms` of the
//! first lifting and inside the `tap.centroid_max_*_um` ellipse around where it was. Taps never scroll: their motion stays under the
//! lock threshold.

use crate::config::Config;
use crate::feel::{Hold, Sample};
use crate::protocol::TapKind;
use crate::sensor::lattice::Ellipse;
use crate::time::Mono;
use crate::touch::Point;

/// Whether a touchdown can still be a tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    Valid,
    Void,
}

/// A touchdown in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct Touchdown {
    since: Mono,
    peak: usize,
    count: usize,
    shrunk: Shrunk,
    origins: Vec<Sample>,
    centroid: Point,
    validity: Validity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shrunk {
    No,
    Yes,
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
    pub fn step(
        self,
        contacts: &[Sample],
        now: Mono,
        cfg: &Config,
        hold: Hold,
    ) -> (Self, Option<TapKind>) {
        match (self, contacts.is_empty()) {
            (Self::Down { first, td }, true) => lift(first, td, now, cfg),
            (Self::Down { first, td }, false) => (
                Self::Down {
                    first,
                    td: td.update(contacts, now, cfg, hold),
                },
                None,
            ),
            (Self::Gap(g), false) => (
                Self::Down {
                    first: live(g, now, cfg),
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
            (Self::Gap(g), true) => (live(g, now, cfg).map_or(Self::Idle, Self::Gap), None),
            (Self::Idle, true) => (Self::Idle, None),
        }
    }
}

fn live(g: Gap, now: Mono, cfg: &Config) -> Option<Gap> {
    (now.since(g.lifted) <= cfg.tap.gap_ms.duration()).then_some(g)
}

fn lift(first: Option<Gap>, td: Touchdown, now: Mono, cfg: &Config) -> (TapG, Option<TapKind>) {
    if !td.is_tap(now, cfg) {
        return (TapG::Idle, None);
    }
    let second = first.filter(|g| {
        g.fingers == td.peak
            && td.since.since(g.lifted) <= cfg.tap.gap_ms.duration()
            && Ellipse::of(cfg.tap.centroid_max_x_um, cfg.tap.centroid_max_y_um)
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
            shrunk: Shrunk::No,
            origins: contacts.to_vec(),
            centroid: centroid(contacts),
            validity: if hold == Hold::Held {
                Validity::Void
            } else {
                Validity::Valid
            },
        }
    }

    fn update(mut self, contacts: &[Sample], now: Mono, cfg: &Config, hold: Hold) -> Self {
        let n = contacts.len();
        let regrew = self.shrunk == Shrunk::Yes && n > self.count;
        if n < self.count {
            self.shrunk = Shrunk::Yes;
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
        let max_move = Ellipse::of(cfg.tap.max_move_x_um, cfg.tap.max_move_y_um);
        let moved = contacts.iter().any(|c| {
            let o = self
                .origins
                .iter()
                .find(|o| o.id == c.id)
                .map_or(c.at, |o| o.at);
            max_move.reached(c.at.delta_from(o))
        });
        let late = now.since(self.since) > cfg.tap.max_ms.duration();
        if moved || late || regrew || hold == Hold::Held || self.peak > 2 {
            self.validity = Validity::Void;
        }
        self
    }

    fn is_tap(&self, lift: Mono, cfg: &Config) -> bool {
        self.validity == Validity::Valid
            && (1..=2).contains(&self.peak)
            && lift.since(self.since) <= cfg.tap.max_ms.duration()
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
        let cfg = Config::default();
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
                let (n, k) = t.clone().step(c, Mono::from_millis(*ms), &cfg, Hold::Open);
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
        let cfg = Config::default();
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
                let (n, k) = t.clone().step(c, Mono::from_millis(*ms), &cfg, Hold::Open);
                *t = n;
                Some(k)
            })
            .collect();
        assert_eq!(got.last(), Some(&Some(TapKind::Two)));
    }
}
