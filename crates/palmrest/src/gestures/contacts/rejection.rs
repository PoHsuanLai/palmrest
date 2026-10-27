//! Palm and rest rejection P1-P3 (12.3.3): how each track is classed when it lands and as it
//! moves. Palm and Resting are for life: a thumb resting on the side stays rejected even if it
//! later shuffles.

use crate::config::{Ms, Um, Units};
use crate::gestures::bands::Bands;
use crate::sensor::Shape;
use crate::sensor::lattice::Ellipse;
use crate::time::Mono;
use crate::touch::{Point, TouchId};

/// P1: a Magic Mouse contact smaller than this is a light touch.
const LIGHT_TOUCH_SIZE_MAX: Units = Units(2);
/// P2: a band contact that moves this far sideways is a finger (an ellipse with the next).
const RESTING_MOVE_FLOOR_X: Um = Um(1154);
/// P2: a band contact that moves this far up or down is a finger.
const RESTING_MOVE_FLOOR_Y: Um = Um(429);
/// P2: a band contact that has not moved for this long is resting.
const RESTING_WINDOW: Ms = Ms(150);
/// P3: a Magic Mouse contact at least this long on its major axis is a palm.
const LARGE_CONTACT_MAJOR: Units = Units(240);

/// A frame contact on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Touch {
    pub(super) id: TouchId,
    pub(super) at: Point,
    pub(super) shape: Shape,
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

/// One contact over its life: where it landed, where it is, how it is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Track {
    pub(super) id: TouchId,
    pub(super) landed: Mono,
    pub(super) origin: Point,
    pub(super) at: Point,
    pub(super) class: Contact,
}

/// A new track, classed as it landed.
pub(super) fn land(t: &Touch, now: Mono, bands: &Bands) -> Track {
    let k = Track {
        id: t.id,
        landed: now,
        origin: t.at,
        at: t.at,
        class: Contact::Firm,
    };
    let class = if is_palm(t) {
        Contact::Palm
    } else if bands.contains(t.at) {
        Contact::Watched
    } else {
        by_size(t)
    };
    Track { class, ..k }
}

/// A known track moved to `t`, reclassed.
pub(super) fn advance(k: Track, t: &Touch, now: Mono) -> Track {
    let k = Track { at: t.at, ..k };
    let class = match k.class {
        Contact::Palm | Contact::Resting => k.class,
        _ if is_palm(t) => Contact::Palm,
        Contact::Watched => watched(k, t, now),
        _ => by_size(t),
    };
    Track { class, ..k }
}

/// P2: a watched contact that moves far enough is a finger; one that sits out its window rests.
fn watched(k: Track, t: &Touch, now: Mono) -> Contact {
    let floor = Ellipse::of(RESTING_MOVE_FLOOR_X, RESTING_MOVE_FLOOR_Y);
    if floor.reached(k.at.delta_from(k.origin)) {
        by_size(t)
    } else if now.since(k.landed) >= RESTING_WINDOW.duration() {
        Contact::Resting
    } else {
        Contact::Watched
    }
}

/// P1 on the Magic Mouse's size; a sensor without one reports firm touches only.
fn by_size(t: &Touch) -> Contact {
    match t.shape {
        Shape::MagicMouse(m) if i32::from(m.size) < LIGHT_TOUCH_SIZE_MAX.0 => Contact::Light,
        Shape::MagicMouse(_) | Shape::Unmeasured => Contact::Firm,
    }
}

/// P3 on the Magic Mouse's major axis; a sensor without one never sees a palm.
fn is_palm(t: &Touch) -> bool {
    match t.shape {
        Shape::MagicMouse(m) => i32::from(m.major) >= LARGE_CONTACT_MAJOR.0,
        Shape::Unmeasured => false,
    }
}
