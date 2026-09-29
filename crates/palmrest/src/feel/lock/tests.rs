//! The lock machine over short report sequences.

use super::{AxisLock, FREE};
use crate::feel::curve::CurveParams;
use crate::feel::lock_event::{Hold, LockEvent, Sample};
use crate::feel::rule::{Lock, LockParams, Threshold};
use crate::time::Mono;
use crate::touch::{Point, TouchId};
use std::time::Duration;

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

fn one(x: i32, y: i32) -> Vec<Sample> {
    vec![Sample {
        id: TouchId(1),
        at: Point::new(x, y),
    }]
}

fn run(lock: AxisLock, frames: &[(u64, Vec<Sample>)]) -> (AxisLock, Vec<LockEvent>) {
    frames.iter().fold((lock, vec![]), |(l, mut all), (ms, c)| {
        let (l, ev) = l.step(c, Mono::from_millis(*ms), &PARAMS, Hold::Open);
        all.extend(ev);
        (l, all)
    })
}

#[test]
fn locking_drops_the_pending_motion() {
    let frames: Vec<_> = (0..10)
        .map(|i| (10 + i * 8, one(0, i as i32 * 10)))
        .collect();
    let (l, ev) = run(AxisLock::default(), &frames);
    assert_eq!(l.lock(), Lock::Y);
    let raw: Vec<i32> = ev
        .iter()
        .filter_map(|e| {
            if let LockEvent::Moved(m) = e {
                Some(m.raw)
            } else {
                None
            }
        })
        .collect();
    // Frames 1..=4 accumulate 40 and lock; frames 5..=9 scroll 10 each.
    assert_eq!(raw, [10; 5]);
    assert!(matches!(ev[0], LockEvent::Start { fingers: 1 }));
    assert_eq!(ev[1], LockEvent::Locked(Lock::Y));
}

#[test]
fn a_pause_unlocks_on_the_next_delta() {
    let frames = vec![
        (0, one(0, 0)),
        (8, one(0, 50)),
        (16, one(0, 60)),
        (400, one(0, 70)),
    ];
    let (l, ev) = run(AxisLock::default(), &frames);
    assert_eq!(
        l.lock(),
        Lock::Free {
            pend_x: 0,
            pend_y: 10
        }
    );
    assert_eq!(ev.last(), Some(&LockEvent::Unlocked));
}

#[test]
fn zero_delta_does_not_refresh_motion_time() {
    // Nothing moved: the motion time is not refreshed.
    let frames = vec![
        (0, one(0, 0)),
        (8, one(0, 50)),
        (200, one(0, 50)),
        (300, one(0, 60)),
    ];
    let (_, ev) = run(AxisLock::default(), &frames);
    assert_eq!(ev.last(), Some(&LockEvent::Unlocked));
}

#[test]
fn held_click_accumulates_nothing() {
    let (l, _) = AxisLock::default().step(&one(0, 0), Mono(0), &PARAMS, Hold::Open);
    let (l, ev) = l.step(&one(0, 100), Mono::from_millis(8), &PARAMS, Hold::Held);
    assert!(ev.is_empty());
    assert_eq!(l.lock(), FREE);
    let (_, ev) = l.step(&one(0, 110), Mono::from_millis(16), &PARAMS, Hold::Open);
    assert!(ev.is_empty(), "only the 10 units after the hold count");
}

#[test]
fn three_contacts_reset_once() {
    let three: Vec<Sample> = (1..=3)
        .map(|i| Sample {
            id: TouchId(i),
            at: Point::new(0, 0),
        })
        .collect();
    let frames = vec![(0, one(0, 0)), (8, three.clone()), (16, three)];
    let (l, ev) = run(AxisLock::default(), &frames);
    assert_eq!(l.set(), None);
    assert_eq!(ev, [LockEvent::Start { fingers: 1 }, LockEvent::Reset]);
}
