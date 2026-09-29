//! Acceptance 12.8 items 4-7, and the replay half of 10, with the shipped configuration
//! (rejection on, touchpad output): swipe trigger, double-taps, one-finger swipe, rejection,
//! suppression.

mod support;

use palmrest::action::Action;
use palmrest::config::{Config, Rejection};
use palmrest::keys::{Key, KeyCombo};
use palmrest::recording::Record;
use palmrest::replay::{Timing, feed, outs, replay};
use palmrest::stamp::StampClock;
use palmrest::{Out, Tracker};
use palmrest_protocol::{Axis, Over, Phase, TapKind, ToHost};

fn run(name: &str) -> Vec<Out> {
    outs(&replay(
        &support::load(name),
        Config::default(),
        Timing::Arrival,
    ))
}

/// Replays with a host holding the pointer from the start.
fn run_claimed(name: &str) -> Vec<Out> {
    let (mut t, _) = Tracker::new(Config::default()).claim(Over::Ours);
    let mut clock = StampClock::new();
    let mut all = Vec::new();
    for r in &support::load(name) {
        let (nt, nc, step) = feed(t, clock, r, Timing::Arrival);
        t = nt;
        clock = nc;
        all.extend(step.outs);
    }
    all
}

fn actions(outs: &[Out]) -> Vec<Action> {
    outs.iter()
        .filter_map(|o| {
            if let Out::Action(a) = o {
                Some(a.clone())
            } else {
                None
            }
        })
        .collect()
}

fn keys(outs: &[Out]) -> Vec<KeyCombo> {
    outs.iter()
        .filter_map(|o| {
            if let Out::Key(k) = o {
                Some(k.clone())
            } else {
                None
            }
        })
        .collect()
}

fn taps(outs: &[Out]) -> Vec<TapKind> {
    support::host(outs)
        .into_iter()
        .filter_map(|h| {
            if let ToHost::Tap { kind, .. } = h {
                Some(kind)
            } else {
                None
            }
        })
        .collect()
}

// ---- 4. Swipe trigger ---------------------------------------------------------------------

#[test]
fn swipe_trigger() {
    const CASES: &[(&str, &[Action])] = &[
        ("swipe_200_right", &[Action::WorkspacePrev]),
        ("swipe_200_left", &[Action::WorkspaceNext]),
        ("swipe_199", &[]),
        ("swipe_vertical_then_horizontal", &[Action::WorkspacePrev]),
        ("swipe_600_fast", &[Action::WorkspacePrev]),
    ];
    for &(name, want) in CASES {
        assert_eq!(actions(&run(name)), want, "{name}");
    }
}

#[test]
fn vertical_then_horizontal_fires_only_after_the_further_200() {
    let steps = replay(
        &support::load("swipe_vertical_then_horizontal"),
        Config::default(),
        Timing::Arrival,
    );
    let fired_at: Vec<u64> = steps
        .iter()
        .filter(|s| !actions(&s.outs).is_empty())
        .map(|s| s.at.0 / 1_000_000)
        .collect();
    // 160 vertical (to 240 ms), 160 horizontal (to 492 ms): nothing. The further 200 completes
    // 40 units into the third stroke, at 504 + 5 x 12 ms.
    assert_eq!(fired_at, [564]);
}

#[test]
fn a_two_finger_swipe_never_scrolls_sideways() {
    let all = run("swipe_600_fast");
    assert!(support::scrolls(&all).iter().all(|s| s.2 != Some(Axis::X)));
    assert!(
        !all.iter().any(|o| matches!(o, Out::Pad(_))),
        "Blocked contacts never reach the touchpad"
    );
}

// ---- 5. Double-taps -----------------------------------------------------------------------

#[test]
fn two_finger_double_tap() {
    const CASES: &[(&str, &[Action])] = &[
        ("tap_two_double", &[Action::WorkspaceOverview]),
        ("tap_two_gap400", &[]),
        ("tap_two_moving45", &[]),
        ("tap_two_click", &[]),
    ];
    for &(name, want) in CASES {
        assert_eq!(actions(&run(name)), want, "{name}");
    }
}

#[test]
fn one_finger_double_tap_zooms_only_under_a_claim() {
    let free = run("tap_one_double");
    assert!(
        taps(&free).is_empty() && actions(&free).is_empty() && keys(&free).is_empty(),
        "{free:?}"
    );
    assert_eq!(taps(&run_claimed("tap_one_double")), [TapKind::One]);
}

#[test]
fn taps_never_scroll() {
    for name in ["tap_two_double", "tap_one_double"] {
        let all = run(name);
        assert!(
            support::scrolls(&all)
                .iter()
                .all(|s| matches!(s.0, Phase::MayBegin | Phase::Ended)),
            "{name}"
        );
        assert!(
            !all.iter().any(|o| matches!(o, Out::Pad(_))),
            "{name}: no touchpad contact for a tap"
        );
    }
}

// ---- 6. One-finger swipe ------------------------------------------------------------------

#[test]
fn one_finger_swipe() {
    let fast = run("g4_fast");
    assert_eq!(
        keys(&fast),
        [KeyCombo::alt(Key::LEFT)],
        "fingers moved right: Back"
    );
    assert!(
        support::scrolled(&fast).0 > 0.0,
        "the horizontal scroll is still emitted"
    );
    let slow = run("g4_slow");
    assert!(keys(&slow).is_empty());
    assert!(support::scrolled(&slow).0 > 0.0);
    assert!(
        keys(&run_claimed("g4_fast")).is_empty(),
        "no G4 keys while a host holds the pointer"
    );
}

// ---- 7. Rejection -------------------------------------------------------------------------

#[test]
fn a_resting_side_contact_is_ignored_only_with_rejection_on() {
    let records: Vec<Record> = support::load("rest_side_band");
    let on = outs(&replay(&records, Config::default(), Timing::Arrival));
    let off = outs(&replay(
        &records,
        Config {
            rejection: Rejection::Off,
            ..Config::default()
        },
        Timing::Arrival,
    ));
    let fingers = |o: &[Out]| support::scrolls(o).iter().map(|s| s.1).max();
    assert_eq!(fingers(&on), Some(1));
    assert_eq!(fingers(&off), Some(2));
    // One finger scrolls the whole stroke; two fingers' centroid moves half as far, and
    // locks later.
    let (on_y, off_y) = (support::scrolled(&on).1, support::scrolled(&off).1);
    assert!(
        on_y > 1.5 * off_y && off_y > 0.0,
        "on {on_y} px, off {off_y} px"
    );
}

#[test]
fn a_click_cancels_the_scroll_and_holds_gestures_off() {
    let all = run("click_during_scroll");
    let phases: Vec<Phase> = support::scrolls(&all).iter().map(|s| s.0).collect();
    let cancel = phases
        .iter()
        .position(|p| *p == Phase::Cancelled)
        .expect("cancelled at the press");
    assert!(
        phases[cancel + 1..].iter().all(|p| *p != Phase::Changed),
        "no scroll while the button is held: {phases:?}"
    );
    assert!(phases.iter().filter(|p| **p == Phase::Began).count() == 1);
    let pad: Vec<&Out> = all.iter().filter(|o| matches!(o, Out::Pad(_))).collect();
    assert!(matches!(
        pad.last(),
        Some(Out::Pad(palmrest::outputs::pad::PadFrame::Lift))
    ));
}

#[test]
fn a_palm_cancels_the_scroll() {
    let phases: Vec<Phase> = support::scrolls(&run("palm_during_scroll"))
        .iter()
        .map(|s| s.0)
        .collect();
    assert!(phases.contains(&Phase::Cancelled), "{phases:?}");
    let after = &phases[phases.iter().position(|p| *p == Phase::Cancelled).unwrap() + 1..];
    assert!(!after.contains(&Phase::Changed));
}

#[test]
fn a_moving_mouse_needs_twice_the_travel_to_scroll() {
    assert!(
        support::scrolls(&run("mouse_moving"))
            .iter()
            .all(|s| s.0 != Phase::Began)
    );
    let off = outs(&replay(
        &support::load("mouse_moving"),
        Config {
            rejection: Rejection::Off,
            ..Config::default()
        },
        Timing::Arrival,
    ));
    assert!(support::scrolls(&off).iter().any(|s| s.0 == Phase::Began));
}

// ---- phase bookkeeping, over every fixture ------------------------------------------------

#[test]
fn every_may_begin_is_closed_exactly_once() {
    for (name, records) in support::scenarios::all() {
        let all = outs(&replay(&records, Config::default(), Timing::Arrival));
        let mut open: Option<palmrest_protocol::GestureId> = None;
        for h in support::host(&all) {
            if let ToHost::Scroll { gesture, phase, .. } = h {
                match phase {
                    Phase::MayBegin => {
                        assert_eq!(open, None, "{name}: MayBegin while {open:?} is open");
                        open = Some(gesture);
                    }
                    Phase::Began | Phase::Changed => assert_eq!(open, Some(gesture), "{name}"),
                    Phase::Ended | Phase::Cancelled => {
                        assert_eq!(
                            open,
                            Some(gesture),
                            "{name}: closing a gesture that is not open"
                        );
                        open = None;
                    }
                }
            }
        }
    }
}

#[test]
fn touches_down_and_up_alternate() {
    for (name, records) in support::scenarios::all() {
        let all = outs(&replay(&records, Config::default(), Timing::Arrival));
        let t: Vec<_> = support::host(&all)
            .into_iter()
            .filter_map(|h| {
                if let ToHost::Touches { active } = h {
                    Some(active)
                } else {
                    None
                }
            })
            .collect();
        assert!(t.windows(2).all(|w| w[0] != w[1]), "{name}: {t:?}");
    }
}

// ---- 10 (replay half). Suppression -------------------------------------------------------

#[test]
fn a_claimed_pointer_silences_every_foreign_output() {
    use palmrest::config::ForeignOutput;
    for name in ["g4_fast", "feel_fast", "click_during_scroll"] {
        let free = run(name);
        assert!(
            free.iter().any(|o| matches!(o, Out::Pad(_))),
            "{name}: the touchpad is used when nobody claims"
        );
        let claimed = run_claimed(name);
        assert!(
            !claimed
                .iter()
                .any(|o| matches!(o, Out::Pad(_) | Out::Wheel(_) | Out::Key(_))),
            "{name}"
        );
        assert_eq!(
            support::scrolls(&claimed),
            support::scrolls(&free),
            "{name}: the socket stream is unchanged"
        );
    }
    // The wheel output is silenced the same way.
    let wheel = Config {
        foreign_output: ForeignOutput::Wheel,
        ..Config::default()
    };
    let (mut t, _) = Tracker::new(wheel.clone()).claim(Over::Ours);
    let mut all = Vec::new();
    for r in &support::load("feel_fast") {
        let (nt, _, step) = feed(t, StampClock::new(), r, Timing::Arrival);
        t = nt;
        all.extend(step.outs);
    }
    assert!(!all.iter().any(|o| matches!(o, Out::Wheel(_))));
    assert!(
        outs(&replay(&support::load("feel_fast"), wheel, Timing::Arrival))
            .iter()
            .any(|o| matches!(o, Out::Wheel(_)))
    );
}

#[test]
fn claiming_mid_stroke_lifts_the_touchpad() {
    let records = support::load("g4_slow");
    let (mut t, mut clock) = (Tracker::new(Config::default()), StampClock::new());
    for r in &records[..31] {
        let (nt, nc, _) = feed(t, clock, r, Timing::Arrival);
        (t, clock) = (nt, nc);
    }
    let (_, lifted) = t.claim(Over::Ours);
    assert_eq!(lifted, [Out::Pad(palmrest::outputs::pad::PadFrame::Lift)]);
}
