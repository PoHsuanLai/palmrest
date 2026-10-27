//! Tracking and rejection through `Contacts::step`.

use super::{Contacts, PalmTurn, Seen, is_contact};
use crate::gestures::bands::Bands;
use crate::sensor::MagicShape;
use crate::sensor::magic_mouse::{self, RESOLUTION};
use crate::sensor::{FrameContact, Shape};
use crate::time::Mono;
use crate::touch::{TouchId, TouchState};

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

fn bands() -> Bands {
    Bands::resolve(&magic_mouse::surface())
}

fn ids(seen: &Seen) -> Vec<u8> {
    seen.samples.iter().map(|s| s.id.0).collect()
}

fn run(frames: &[(u64, Vec<FrameContact>)]) -> Vec<Vec<u8>> {
    let mut c = Contacts::default();
    let b = bands();
    frames
        .iter()
        .map(|(ms, t)| {
            let (n, seen) = c.clone().step(t, Mono::from_millis(*ms), &b);
            c = n;
            ids(&seen)
        })
        .collect()
}

#[test]
fn nibble_table() {
    let mut t = raw(1, 0, 0, 10, 20);
    // Nibbles 1 to 4 (in range, making touch, touching) are contacts.
    const CASES: &[(u8, bool)] = &[
        (0, false),
        (1, true),
        (2, true),
        (3, true),
        (4, true),
        (5, false),
        (7, false),
    ];
    for &(n, counted) in CASES {
        t.state = TouchState::from_nibble(n);
        assert_eq!(is_contact(&t), counted, "nibble {n}");
    }
    t.state = TouchState::Touching;
    t.shape = Shape::MagicMouse(MagicShape {
        size: 0,
        ..shape(&t)
    });
    assert!(!is_contact(&t), "size 0 is not a contact");
    t.shape = Shape::Unmeasured;
    assert!(is_contact(&t), "an unmeasured touch is a contact");
}

fn shape(t: &FrameContact) -> MagicShape {
    match t.shape {
        Shape::MagicMouse(m) => m,
        Shape::Unmeasured => panic!("a Magic Mouse touch"),
    }
}

#[test]
fn light_touch_never_joins_but_may_continue() {
    let got = run(&[
        (0, vec![raw(1, 0, 0, 1, 20)]),
        (10, vec![raw(1, 0, 0, 12, 20)]),
        (20, vec![raw(1, 0, 0, 1, 20)]),
        (30, vec![raw(1, 0, 0, 1, 20), raw(2, 300, 0, 1, 20)]),
    ]);
    assert_eq!(got, [vec![], vec![1], vec![1], vec![1]]);
}

#[test]
fn a_still_contact_in_the_side_band_rests() {
    let got = run(&[
        (0, vec![raw(1, -1000, 0, 12, 20), raw(2, 0, 0, 12, 20)]),
        (100, vec![raw(1, -1005, 5, 12, 20), raw(2, 0, 50, 12, 20)]),
        (160, vec![raw(1, -1005, 5, 12, 20), raw(2, 0, 100, 12, 20)]),
        (400, vec![raw(1, -900, 100, 12, 20), raw(2, 0, 150, 12, 20)]),
    ]);
    assert_eq!(
        got,
        [vec![2], vec![2], vec![2], vec![2]],
        "rested for life, even after moving"
    );
}

#[test]
fn a_moving_contact_in_a_band_is_a_finger() {
    let got = run(&[
        (0, vec![raw(1, 1100, 0, 12, 20)]),
        (50, vec![raw(1, 1100, 40, 12, 20)]),
    ]);
    assert_eq!(got, [vec![], vec![1]]);
}

#[test]
fn palm_is_rejected_and_reported_when_it_was_counted() {
    let b = bands();
    let mut c = Contacts::default();
    let (n, _) = c.step(&[raw(1, 0, 0, 12, 20)], Mono(0), &b);
    c = n;
    let (n, seen) = c.step(&[raw(1, 0, 0, 12, 250)], Mono::from_millis(10), &b);
    assert_eq!(seen.palm, PalmTurn::InSet);
    assert!(seen.samples.is_empty());
    let (_, seen) = n.step(&[raw(1, 0, 0, 12, 20)], Mono::from_millis(20), &b);
    assert!(seen.samples.is_empty(), "palm for life");
    assert_eq!(seen.palm, PalmTurn::None, "reported once");
}

#[test]
fn a_sensor_without_shape_counts_firm_and_never_palm() {
    let bare = |id: u8, x: i32| FrameContact {
        shape: Shape::Unmeasured,
        ..raw(id, x, 0, 0, 0)
    };
    let got = run(&[(0, vec![bare(1, 0), bare(2, 300)])]);
    assert_eq!(got, [vec![1, 2]]);
}
