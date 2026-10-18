//! Palm and rest rejection P1-P3 (12.3.3): how each track is classed when it lands and as it
//! moves. Palm and Resting are for life: a thumb resting on the side stays rejected even if it
//! later shuffles.

use crate::config::{Config, Rejection};
use crate::gestures::bands::Bands;
use crate::sensor::Shape;
use crate::sensor::lattice::Ellipse;
use crate::time::Mono;
use crate::touch::{Point, TouchId};

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
pub(super) fn land(t: &Touch, now: Mono, cfg: &Config, bands: &Bands) -> Track {
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

/// A known track moved to `t`, reclassed.
pub(super) fn advance(k: Track, t: &Touch, now: Mono, cfg: &Config) -> Track {
    let k = Track { at: t.at, ..k };
    let class = match (cfg.rejection, k.class) {
        (Rejection::Off, _) => Contact::Firm,
        (_, Contact::Palm | Contact::Resting) => k.class,
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
