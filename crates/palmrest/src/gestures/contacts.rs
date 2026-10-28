//! Per-touch tracking (12.3.2): each id gets a track for the life of its contact, and a
//! track's class (`rejection`, P1-P3) decides whether it counts toward the contact set the
//! gestures see.

mod rejection;

use super::bands::Bands;
use crate::feel::Sample;
use crate::sensor::FrameContact;
use crate::sensor::lattice;
use crate::time::Mono;
use crate::touch::TouchId;
pub use rejection::Contact;
use rejection::{Touch, Track, advance, land};

/// Every live track and the contact set of the previous report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contacts {
    tracks: Vec<Track>,
    set: Vec<TouchId>,
}

/// One report's view: the contacts the gestures see, in report order, and whether a contact of
/// the running set turned Palm this report (it cancels the scroll).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub samples: Vec<Sample>,
    pub palm_turned: bool,
}

/// Whether the frame's state makes this touch a contact at all (12.3.2): a state nibble from
/// 1 to 4 (in range, making touch or touching), and on the Magic Mouse a non-zero size.
pub fn is_contact(t: &FrameContact) -> bool {
    let counted = (1..=4).contains(&t.state.nibble());
    counted && t.shape.is_none_or(|m| m.size > 0)
}

impl Contacts {
    /// The ids counted in the last report, sorted.
    pub fn set(&self) -> &[TouchId] {
        &self.set
    }

    /// Consumes one frame's contacts, with the bands of the surface they are on.
    pub fn step(self, touches: &[FrameContact], now: Mono, bands: &Bands) -> (Self, Seen) {
        let live: Vec<Touch> = touches
            .iter()
            .filter(|t| is_contact(t))
            .map(|t| Touch {
                id: t.id,
                at: lattice::point(t.at),
                shape: t.shape,
            })
            .collect();
        let tracks: Vec<Track> = live
            .iter()
            .map(|t| match self.tracks.iter().find(|k| k.id == t.id) {
                Some(k) => advance(*k, t, now),
                None => land(t, now, bands),
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
        let palm_turned = tracks
            .iter()
            .any(|k| k.class == Contact::Palm && self.set.contains(&k.id));
        let mut set: Vec<TouchId> = samples.iter().map(|s| s.id).collect();
        set.sort();
        (
            Self { tracks, set },
            Seen {
                samples,
                palm_turned,
            },
        )
    }
}

#[cfg(test)]
mod tests;
