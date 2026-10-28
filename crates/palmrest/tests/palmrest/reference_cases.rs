//! Acceptance 12.8 item 3: the reference gesture cases, against the matching palmrest unit
//! (`AxisLock`, `SwipeG`) with the case's own parameters. The cases and their expected values are
//! frozen from the retired Python daemon's test suite; points are in its unsigned reading.

use palmrest::Mono;
use palmrest::action::{Action, Desktop, Gesture, plan};
use palmrest::config::Config;
use palmrest::feel::{AxisLock, Hold, LockEvent, LockParams, Sample, Threshold};
use palmrest::gestures::{FingerDirection, SwipeEvent, SwipeG};
use palmrest_protocol::HostAction;
use std::time::Duration;

// ---- the case helpers ---------------------------------------------------------------------

/// `touch(x, y, id)` in the unsigned reading, as a lock sample in the kernel frame.
fn touch(x: i32, y: i32, id: u8) -> Sample {
    let t = crate::support::unsigned_finger(id, x, y);
    Sample { id: t.id, at: t.at }
}

/// `ScrollAxisLock(speed=32, lock_threshold=40)`.
const PARAMS: LockParams = LockParams {
    threshold: Threshold::both(40),
    ratio: 1.5,
    timeout: Duration::from_millis(250),
    speed: 32,
    detent_px: 60.0,
};

/// `drag(filt, points, start=10.0, step=0.008)`: the summed px (horizontal, vertical).
fn drag(
    lock: AxisLock,
    points: &[Vec<Sample>],
    start_ms: u64,
    step_ms: u64,
) -> (AxisLock, (f64, f64)) {
    points
        .iter()
        .enumerate()
        .fold((lock, (0.0, 0.0)), |(l, (h, v)), (i, p)| {
            let (l, ev) = l.step(
                p,
                Mono::from_millis(start_ms + step_ms * i as u64),
                &PARAMS,
                Hold::Open,
            );
            let (dh, dv) = scrolled_px(&ev);
            (l, (h + dh, v + dv))
        })
}

fn scrolled_px(ev: &[LockEvent]) -> (f64, f64) {
    ev.iter().fold((0.0, 0.0), |(h, v), e| match e {
        LockEvent::Moved(m) if m.axis == palmrest_protocol::Axis::X => (h + m.px, v),
        LockEvent::Moved(m) => (h, v + m.px),
        _ => (h, v),
    })
}

fn points(n: i32, f: impl Fn(i32) -> Vec<Sample>) -> Vec<Vec<Sample>> {
    (0..n).map(f).collect()
}

// ---- ScrollAxisLock ---------------------------------------------------------------------

/// What a drag must scroll: one axis only, or nothing at all.
#[derive(Clone, Copy)]
enum Scrolls {
    OnlyVertical,
    OnlyHorizontal,
    Nothing,
}

impl Scrolls {
    fn holds(self, (h, v): (f64, f64)) -> bool {
        match self {
            Scrolls::OnlyVertical => v != 0.0 && h == 0.0,
            Scrolls::OnlyHorizontal => h != 0.0 && v == 0.0,
            Scrolls::Nothing => (h, v) == (0.0, 0.0),
        }
    }
}

#[test]
fn the_axis_lock_scrolls_the_dominant_axis_only() {
    // (name, the fingers at step i, what the drag scrolls)
    type Case = (&'static str, fn(i32) -> Vec<Sample>, Scrolls);
    const AXIS_CASES: &[Case] = &[
        (
            "one finger vertical",
            |i| vec![touch(1000, 1000 + i * 6, 1)],
            Scrolls::OnlyVertical,
        ),
        (
            "one finger horizontal",
            |i| vec![touch(1000 + i * 6, 1000, 1)],
            Scrolls::OnlyHorizontal,
        ),
        (
            "two fingers never horizontal",
            |i| vec![touch(900 + i * 6, 1000, 1), touch(1300 + i * 6, 1000, 2)],
            Scrolls::Nothing,
        ),
        (
            "two fingers vertical",
            |i| vec![touch(900, 1000 + i * 6, 1), touch(1300, 1000 + i * 6, 2)],
            Scrolls::OnlyVertical,
        ),
    ];
    for &(name, fingers, want) in AXIS_CASES {
        let (_, scrolled) = drag(AxisLock::default(), &points(40, fingers), 10_000, 8);
        assert!(
            want.holds(scrolled),
            "{name}: the unit scrolled {scrolled:?}"
        );
    }
}

#[test]
fn three_fingers_do_nothing() {
    let pts = points(40, |i| {
        (1..=3)
            .map(|k| touch(500 + 400 * i32::from(k), 1000 + i * 6, k))
            .collect()
    });
    assert_eq!(drag(AxisLock::default(), &pts, 10_000, 8).1, (0.0, 0.0));
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
    assert_eq!(scrolled_px(&ev), (0.0, 0.0));
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
        let (_, v) = scrolled_px(&ev);
        if v != 0.0 {
            emitted.push(v);
        }
    }
    assert!(!emitted.is_empty(), "expected high-resolution scroll");
    assert!(
        emitted.iter().any(|v| v.abs() < 6.0),
        "a step below 6 px (a tenth of a 60 px detent)"
    );
}

// ---- DesktopSwipe -------------------------------------------------------------------------

fn swipe_run(frames: &[(u64, Vec<Sample>)]) -> Vec<FingerDirection> {
    let mut s = SwipeG::default();
    let mut fired = Vec::new();
    for (ms, pair) in frames {
        let (n, ev) = s.step(pair, Mono::from_millis(*ms), Hold::Open);
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
        Some(HostAction::Command { argv }) => argv.last().cloned().unwrap(),
        other => panic!("{other:?}"),
    };
    let map = Config::default().gestures.gesture_action_map;
    assert_eq!(
        kwin(map.action_for(Gesture::TwoFingerSwipeRight)),
        "Switch One Desktop to the Left"
    );
    assert_eq!(
        kwin(map.action_for(Gesture::TwoFingerSwipeLeft)),
        "Switch One Desktop to the Right"
    );
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
}

#[test]
fn these_drags_do_not_switch_desktop() {
    // (name, the fingers at step i)
    type Case = (&'static str, fn(i32) -> Vec<Sample>);
    const NO_SWITCH_CASES: &[Case] = &[
        ("two-finger vertical drag", |i| {
            vec![
                touch(900 + i * 3, 1000 + i * 10, 1),
                touch(1300 + i * 3, 1000 + i * 10, 2),
            ]
        }),
        ("one finger never switches", |i| {
            vec![touch(500 + i * 20, 1000, 1)]
        }),
    ];
    for &(name, fingers) in NO_SWITCH_CASES {
        let frames: Vec<_> = (0..60)
            .map(|i| (10_000 + 8 * i as u64, fingers(i)))
            .collect();
        assert!(swipe_run(&frames).is_empty(), "{name}: the unit switched");
    }
}
