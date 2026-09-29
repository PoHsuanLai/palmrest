//! Acceptance 12.8 item 3: the reference gesture cases, twice: against the matching palmrest
//! unit (`AxisLock`, `SwipeG`) with the case's own parameters, and end to end through `replay`
//! of the committed fixture with the same points. The cases and their expected values are
//! frozen from the retired Python daemon's test suite; points are in its unsigned reading.

mod support;

use palmrest::action::{Action, Desktop, Effect, plan};
use palmrest::config::{Config, Count, ForeignOutput, Fraction, Rejection};
use palmrest::feel::{
    AxisLock, CurveParams, Hold, LockEvent, LockParams, Sample, Threshold, WheelDelta,
};
use palmrest::gestures::{FingerDirection, SwipeEvent, SwipeG};
use palmrest::replay::{Timing, outs, replay};
use palmrest::touch::{Point, TouchId};
use palmrest::{Mono, Out};
use std::time::Duration;

// ---- the case helpers ---------------------------------------------------------------------

/// `touch(x, y, id)` in the unsigned reading, as a lock sample in the kernel frame.
fn touch(x: i32, y: i32, id: u8) -> Sample {
    let t = support::unsigned_finger(id, x, y);
    Sample { id: t.id, at: t.at }
}

/// `ScrollAxisLock(speed=32, acceleration_max=1.0, lock_threshold=40)`.
const PARAMS: LockParams = LockParams {
    threshold: Threshold::both(40),
    ratio: 1.5,
    timeout: Duration::from_millis(250),
    speed: 32,
    velocity_gain_max: 2.0,
    repeat_gain_max: 1.0,
    detent_px: 60.0,
    curve: CurveParams::Classic,
};

/// `drag(filt, points, start=10.0, step=0.008)`: the summed hi-res (h, v) in the wheel's frame.
fn drag(
    lock: AxisLock,
    points: &[Vec<Sample>],
    start_ms: u64,
    step_ms: u64,
) -> (AxisLock, (i32, i32)) {
    points
        .iter()
        .enumerate()
        .fold((lock, (0, 0)), |(l, (h, v)), (i, p)| {
            let (l, ev) = l.step(
                p,
                Mono::from_millis(start_ms + step_ms * i as u64),
                &PARAMS,
                Hold::Open,
            );
            let (dh, dv) = hi_res(&ev);
            (l, (h + dh, v + dv))
        })
}

fn hi_res(ev: &[LockEvent]) -> (i32, i32) {
    ev.iter().fold((0, 0), |(h, v), e| match e {
        LockEvent::Moved(m) => {
            let w = WheelDelta::on(m.axis, m.quanta);
            (h + w.horizontal_hi_res, v + w.vertical_hi_res)
        }
        _ => (h, v),
    })
}

fn points(n: i32, f: impl Fn(i32) -> Vec<Sample>) -> Vec<Vec<Sample>> {
    (0..n).map(f).collect()
}

/// The config the cases construct, for the end-to-end runs.
fn reference_config() -> Config {
    let mut c = Config {
        rejection: Rejection::Off,
        foreign_output: ForeignOutput::Wheel,
        ..Config::default()
    };
    c.scroll.speed = Count(32);
    c.scroll.repeat_gain_max = Fraction(1000);
    c
}

/// The wheel hi-res totals (h, v) and the workspace actions of a fixture.
fn run(name: &str) -> ((i32, i32), Vec<Action>) {
    let all = outs(&replay(
        &support::load(name),
        reference_config(),
        Timing::Arrival,
    ));
    let wheel = all.iter().fold((0, 0), |(h, v), o| match o {
        Out::Wheel(w) => (h + w.horizontal_hi_res, v + w.vertical_hi_res),
        _ => (h, v),
    });
    let actions = all
        .iter()
        .filter_map(|o| {
            if let Out::Action(a) = o {
                Some(a.clone())
            } else {
                None
            }
        })
        .collect();
    (wheel, actions)
}

// ---- ScrollAxisLock ---------------------------------------------------------------------

#[test]
fn one_finger_vertical_scrolls_only_vertically() {
    let (_, (h, v)) = drag(
        AxisLock::default(),
        &points(40, |i| vec![touch(1000, 1000 + i * 6, 1)]),
        10_000,
        8,
    );
    assert!(v != 0 && h == 0);
    let ((h, v), _) = run("case_one_vertical");
    assert!(v != 0 && h == 0);
}

#[test]
fn one_finger_horizontal_scrolls_only_horizontally() {
    let (_, (h, v)) = drag(
        AxisLock::default(),
        &points(40, |i| vec![touch(1000 + i * 6, 1000, 1)]),
        10_000,
        8,
    );
    assert!(h != 0 && v == 0);
    let ((h, v), _) = run("case_one_horizontal");
    assert!(h != 0 && v == 0);
}

#[test]
fn two_fingers_never_scroll_horizontally() {
    let pts = points(40, |i| {
        vec![touch(900 + i * 6, 1000, 1), touch(1300 + i * 6, 1000, 2)]
    });
    assert_eq!(drag(AxisLock::default(), &pts, 10_000, 8).1, (0, 0));
    assert_eq!(run("case_two_never_horizontal").0, (0, 0));
}

#[test]
fn two_fingers_scroll_vertically() {
    let pts = points(40, |i| {
        vec![touch(900, 1000 + i * 6, 1), touch(1300, 1000 + i * 6, 2)]
    });
    let (_, (h, v)) = drag(AxisLock::default(), &pts, 10_000, 8);
    assert!(v != 0 && h == 0);
    let ((h, v), _) = run("case_two_vertical");
    assert!(v != 0 && h == 0);
}

#[test]
fn three_fingers_do_nothing() {
    let pts = points(40, |i| {
        (1..=3)
            .map(|k| touch(500 + 400 * i32::from(k), 1000 + i * 6, k))
            .collect()
    });
    assert_eq!(drag(AxisLock::default(), &pts, 10_000, 8).1, (0, 0));
    let all = outs(&replay(
        &support::load("case_three_nothing"),
        reference_config(),
        Timing::Arrival,
    ));
    assert!(
        support::scrolls(&all).is_empty(),
        "no scroll phases at all: {all:?}"
    );
    assert!(!all.iter().any(|o| matches!(o, Out::Wheel(_) | Out::Pad(_))));
}

#[test]
fn finger_count_change_does_not_jump() {
    let (lock, _) = drag(
        AxisLock::default(),
        &points(20, |i| vec![touch(1000, 1000 + i * 6, 1)]),
        10_000,
        8,
    );
    let (_, ev) = lock.step(
        &[touch(1000, 1120, 1), touch(3000, 3000, 2)],
        Mono::from_millis(20_000),
        &PARAMS,
        Hold::Open,
    );
    assert_eq!(hi_res(&ev), (0, 0));
    // End to end: nothing is emitted at the report where the second finger lands.
    let steps = replay(
        &support::load("case_count_change"),
        reference_config(),
        Timing::Arrival,
    );
    let last = steps.last().unwrap();
    assert!(!last.outs.iter().any(|o| matches!(o, Out::Wheel(_))));
    assert!(
        support::scrolls(&last.outs)
            .iter()
            .all(|s| s.3 == 0.0 && s.4 == 0.0)
    );
}

#[test]
fn scroll_steps_are_finer_than_a_detent_tenth() {
    let mut lock = AxisLock::default();
    let (l, _) = lock.step(
        &[touch(1000, 1000, 1)],
        Mono::from_millis(10_000),
        &PARAMS,
        Hold::Open,
    );
    lock = l;
    let mut emitted = Vec::new();
    for step in 1..40u64 {
        let (l, ev) = lock.step(
            &[touch(1000, 1000 + step as i32 * 6, 1)],
            Mono::from_millis(10_000 + step * 8),
            &PARAMS,
            Hold::Open,
        );
        lock = l;
        let (_, v) = hi_res(&ev);
        if v != 0 {
            emitted.push(v);
        }
    }
    assert!(!emitted.is_empty(), "expected high-resolution scroll");
    assert!(emitted.iter().any(|v| v.abs() % 12 != 0));
    // End to end, the wheel and the socket both step finer than a tenth of a detent.
    let all = outs(&replay(
        &support::load("case_fine_steps"),
        reference_config(),
        Timing::Arrival,
    ));
    let wheel: Vec<i32> = all
        .iter()
        .filter_map(|o| {
            if let Out::Wheel(w) = o {
                Some(w.vertical_hi_res)
            } else {
                None
            }
        })
        .collect();
    assert!(wheel.iter().any(|v| v.abs() % 12 != 0));
    let px: Vec<f32> = support::scrolls(&all)
        .iter()
        .map(|s| s.4)
        .filter(|v| *v != 0.0)
        .collect();
    assert!(
        px.iter().any(|v| v.abs() < 6.0),
        "a socket step below 6 px (a tenth of a 60 px detent)"
    );
}

// ---- DesktopSwipe -------------------------------------------------------------------------

fn swipe_run(frames: &[(u64, Vec<Sample>)]) -> Vec<FingerDirection> {
    let cfg = Config::default();
    let mut s = SwipeG::default();
    let mut fired = Vec::new();
    for (ms, pair) in frames {
        let (n, ev) = s.step(pair, Mono::from_millis(*ms), &cfg, Hold::Open);
        s = n;
        fired.extend(ev.into_iter().filter_map(|e| {
            if let SwipeEvent::Fire(d) = e {
                Some(d)
            } else {
                None
            }
        }));
    }
    fired
}

#[test]
fn two_finger_swipe_right_switches_desktop_left() {
    let frames: Vec<_> = (0..40)
        .map(|i| {
            (
                10_000 + 8 * i as u64,
                vec![touch(900 + i * 10, 1000, 1), touch(1300 + i * 10, 1000, 2)],
            )
        })
        .collect();
    let mut with_lift = frames.clone();
    with_lift.push((11_000, vec![]));
    assert_eq!(swipe_run(&with_lift), [FingerDirection::Right]);
    // Fingers right: the desktop on the left; fingers left: the one on the right.
    let kwin = |a: &Action| match plan(a, Desktop::Kde) {
        Effect::Spawn(argv) => argv.last().cloned().unwrap(),
        other => panic!("{other:?}"),
    };
    let map = Config::default().gestures.gesture_action_map;
    assert_eq!(
        kwin(&map.two_finger_swipe_right),
        "Switch One Desktop to the Left"
    );
    assert_eq!(
        kwin(&map.two_finger_swipe_left),
        "Switch One Desktop to the Right"
    );
    assert_eq!(run("case_swipe_right").1, [Action::WorkspacePrev]);
}

#[test]
fn long_two_finger_drag_switches_again_after_cooldown() {
    let frames: Vec<_> = (0..120)
        .map(|i| {
            (
                10_000 + 10 * i as u64,
                vec![touch(500 + i * 10, 1000, 1), touch(900 + i * 10, 1000, 2)],
            )
        })
        .collect();
    assert_eq!(
        swipe_run(&frames),
        [FingerDirection::Right, FingerDirection::Right]
    );
    assert_eq!(
        run("case_long_drag").1,
        [Action::WorkspacePrev, Action::WorkspacePrev]
    );
}

#[test]
fn two_finger_vertical_drag_does_not_switch_desktop() {
    let frames: Vec<_> = (0..60)
        .map(|i| {
            (
                10_000 + 8 * i as u64,
                vec![
                    touch(900 + i * 3, 1000 + i * 10, 1),
                    touch(1300 + i * 3, 1000 + i * 10, 2),
                ],
            )
        })
        .collect();
    assert!(swipe_run(&frames).is_empty());
    assert!(run("case_vertical_no_switch").1.is_empty());
}

#[test]
fn one_finger_never_switches_desktop() {
    let frames: Vec<_> = (0..60)
        .map(|i| (10_000 + 8 * i as u64, vec![touch(500 + i * 20, 1000, 1)]))
        .collect();
    assert!(swipe_run(&frames).is_empty());
    assert!(run("case_one_never_switches").1.is_empty());
}

#[test]
fn the_unsigned_points_convert_to_the_kernel_frame() {
    // The conversion the ports rely on: unsigned 12-bit in, kernel frame out.
    assert_eq!(
        touch(3000, 3000, 2),
        Sample {
            id: TouchId(2),
            at: Point::new(3000 - 4096, 4096 - 3000)
        }
    );
    assert_eq!(touch(1000, 1000, 1).at, Point::new(1000, -1000));
}
