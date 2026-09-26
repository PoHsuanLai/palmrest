//! Table tests over synthetic event sequences (no device).

use super::*;
use crate::outputs::events::{
    ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_SLOT, ABS_MT_TRACKING_ID, BTN_LEFT, EV_ABS,
    EV_KEY, EV_REL, EV_SYN, InputEvent, SYN_REPORT,
};
use crate::sensor::frame::{Buttons, FrameStamp, Motion};
use crate::sensor::lattice;
use crate::time::Mono;
use crate::touch::{Point, TouchId};
use event::{REL_X, REL_Y, SYN_DROPPED};

/// The Magic Mouse's node as the kernel describes it.
fn mm_spec() -> EvdevSpec {
    let axis = |min, max, resolution| AbsAxis {
        min,
        max,
        resolution,
    };
    EvdevSpec::new(axis(-1100, 1258, 26), axis(-1589, 2047, 70), 15).unwrap()
}

/// One step of a script: an event at a millisecond.
type Ev = (u64, u16, u16, i32);

const fn slot(ms: u64, n: i32) -> Ev {
    (ms, EV_ABS, ABS_MT_SLOT, n)
}
const fn track(ms: u64, id: i32) -> Ev {
    (ms, EV_ABS, ABS_MT_TRACKING_ID, id)
}
const fn x(ms: u64, v: i32) -> Ev {
    (ms, EV_ABS, ABS_MT_POSITION_X, v)
}
const fn y(ms: u64, v: i32) -> Ev {
    (ms, EV_ABS, ABS_MT_POSITION_Y, v)
}
const fn syn(ms: u64) -> Ev {
    (ms, EV_SYN, SYN_REPORT, 0)
}

fn bytes(script: &[Ev]) -> Vec<u8> {
    script
        .iter()
        .flat_map(|&(ms, kind, code, value)| {
            TimedEvent {
                at: Mono::from_millis(ms),
                event: InputEvent { kind, code, value },
            }
            .to_bytes()
        })
        .collect()
}

/// A frame as (ms, [(id, grid x, grid y)]).
type Seen = (u64, Vec<(u8, i32, i32)>);

fn frames(sensor: &mut EvdevMultitouch, script: &[Ev]) -> Vec<Seen> {
    sensor
        .decode(&bytes(script))
        .unwrap()
        .iter()
        .map(|f| {
            let FrameStamp::Kernel(t) = f.stamp else {
                panic!("evdev frames carry the kernel's time")
            };
            let c = f
                .contacts
                .iter()
                .map(|c| {
                    let p = lattice::point(c.at);
                    (c.id.0, p.x, p.y)
                })
                .collect();
            (t.0 / 1_000_000, c)
        })
        .collect()
}

#[test]
fn contact_frames_table() {
    type Case = (&'static str, Vec<Ev>, Vec<Seen>);
    let cases: Vec<Case> = vec![
        (
            "one finger lands, moves, lifts",
            vec![
                track(10, 7),
                x(10, 100),
                y(10, -200),
                syn(10),
                y(21, -150),
                syn(21),
                track(32, -1),
                syn(32),
            ],
            vec![
                (10, vec![(7, 100, -200)]),
                (21, vec![(7, 100, -150)]),
                (32, vec![]),
            ],
        ),
        (
            "two slots",
            vec![
                slot(10, 0),
                track(10, 1),
                x(10, 0),
                y(10, 0),
                slot(10, 1),
                track(10, 2),
                x(10, 500),
                y(10, 40),
                syn(10),
                slot(20, 0),
                x(20, 12),
                syn(20),
            ],
            vec![
                (10, vec![(1, 0, 0), (2, 500, 40)]),
                (20, vec![(1, 12, 0), (2, 500, 40)]),
            ],
        ),
        (
            "a new contact in a slot keeps the position the kernel did not resend",
            vec![
                track(10, 1),
                x(10, 300),
                y(10, 300),
                syn(10),
                track(20, -1),
                syn(20),
                track(30, 2),
                y(30, 310),
                syn(30),
            ],
            vec![
                (10, vec![(1, 300, 300)]),
                (20, vec![]),
                (30, vec![(2, 300, 310)]),
            ],
        ),
        (
            "tracking ids wrap into a byte",
            vec![track(10, 0x1_05), x(10, 1), y(10, 2), syn(10)],
            vec![(10, vec![(5, 1, 2)])],
        ),
        (
            "a slot out of range is ignored until the next slot event",
            vec![
                slot(10, 99),
                track(10, 4),
                x(10, 9),
                slot(10, 2),
                track(10, 3),
                syn(10),
            ],
            vec![(10, vec![(3, 0, 0)])],
        ),
        (
            "SYN_DROPPED drops up to the next report and lifts everything",
            vec![
                track(10, 1),
                x(10, 5),
                syn(10),
                (15, EV_SYN, SYN_DROPPED, 0),
                track(15, 9),
                syn(15),
                slot(20, 1),
                track(20, 4),
                x(20, 50),
                syn(20),
            ],
            vec![(10, vec![(1, 5, 0)]), (20, vec![(4, 50, 0)])],
        ),
    ];
    for (name, script, want) in cases {
        let mut s = EvdevMultitouch::new(mm_spec());
        assert_eq!(frames(&mut s, &script), want, "{name}");
    }
}

#[test]
fn a_frame_may_span_reads() {
    let mut s = EvdevMultitouch::new(mm_spec());
    let first = frames(&mut s, &[track(10, 1), x(10, 40)]);
    assert!(first.is_empty(), "no SYN_REPORT yet");
    let second = frames(&mut s, &[y(10, 60), syn(11)]);
    assert_eq!(second, [(11, vec![(1, 40, 60)])]);
}

#[test]
fn buttons_hold_and_motion_resets_per_frame() {
    let mut s = EvdevMultitouch::new(mm_spec());
    let script = [
        (10, EV_KEY, BTN_LEFT, 1),
        (10, EV_REL, REL_X, 3),
        (10, EV_REL, REL_X, 4),
        (10, EV_REL, REL_Y, -2),
        syn(10),
        syn(20),
        (30, EV_KEY, BTN_LEFT, 0),
        syn(30),
    ];
    let got: Vec<(Buttons, Motion)> = s
        .decode(&bytes(&script))
        .unwrap()
        .iter()
        .map(|f| (f.buttons, f.motion))
        .collect();
    assert_eq!(
        got,
        [
            (Buttons::LEFT, Motion { dx: 7, dy: -2 }),
            (Buttons::LEFT, Motion::default()),
            (Buttons::NONE, Motion::default()),
        ]
    );
}

#[test]
fn the_magic_mouse_node_lands_on_the_grid_it_reported() {
    let mut s = EvdevMultitouch::new(mm_spec());
    let points = [(-1100, -1589), (1258, 2047), (-1, 1), (777, -333)];
    for (i, &(px, py)) in points.iter().enumerate() {
        let ms = 10 * (i as u64 + 1);
        let got = s.decode(&bytes(&[track(ms, 1), x(ms, px), y(ms, py), syn(ms)]));
        let f = &got.unwrap()[0];
        assert_eq!(lattice::point(f.contacts[0].at), Point::new(px, py));
        assert_eq!(f.contacts[0].id, TouchId(1));
    }
}

#[test]
fn a_node_without_resolution_is_refused() {
    let a = AbsAxis {
        min: 0,
        max: 1000,
        resolution: 0,
    };
    assert_eq!(EvdevSpec::new(a, a, 9), Err(EvdevError::NoResolution));
    let s = mm_spec().surface();
    assert!((s.size().width.0 - 2358.0 / 26.0).abs() < 1e-9);
    assert_eq!(mm_spec().slots(), 16);
}

#[test]
fn a_partial_read_is_an_error() {
    let mut s = EvdevMultitouch::new(mm_spec());
    assert_eq!(
        s.decode(&[0; 25]),
        Err(SensorError::Evdev(EvdevError::Partial { len: 25 }))
    );
}
