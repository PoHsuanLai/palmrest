//! The two opt-in feel presets, end to end through replay and the `RecordingEmitter`:
//! `feel.curve = "mac_like"` and `foreign_momentum = "ours"` (FINDINGS F22-F24).

mod support;

use palmrest::compare::compare_curves;
use palmrest::config::{Config, Curve, ForeignMomentum, ForeignOutput};
use palmrest::feel::{Drag, ReleaseRule, ReleaseSamples};
use palmrest::outputs::pad::{CENTRE, PadFrame};
use palmrest::outputs::{Emitted, Emitter, RecordingEmitter};
use palmrest::protocol::{Over, Phase, ToHost};
use palmrest::replay::{Step, Timing, feed, replay};
use palmrest::stamp::StampClock;
use palmrest::{Mono, Out, Tracker};

fn ours(output: ForeignOutput) -> Config {
    Config {
        foreign_output: output,
        foreign_momentum: ForeignMomentum::Ours,
        ..Config::default()
    }
}

/// One frame a virtual device was asked to write, and when.
struct Written {
    at: Mono,
    frame: Emitted,
}

/// Every foreign output of `steps` written through a `RecordingEmitter`, with its step's time,
/// and the time the last locked scroll gesture Ended.
fn written(steps: &[Step]) -> (Vec<Written>, Mono) {
    let mut e = RecordingEmitter::default();
    let mut at = Vec::new();
    let mut ended = Mono(0);
    for s in steps {
        for o in &s.outs {
            match o {
                Out::Pad(f) => e.pad(f).unwrap(),
                Out::Wheel(w) => e.wheel(w).unwrap(),
                Out::Key(k) => e.key(k).unwrap(),
                Out::Host(ToHost::Scroll {
                    phase: Phase::Ended,
                    axis: Some(_),
                    ..
                }) => ended = s.at,
                _ => continue,
            }
            at.resize(e.emitted.len(), s.at);
        }
    }
    let frames = e
        .emitted
        .into_iter()
        .zip(at)
        .map(|(frame, at)| Written { at, frame })
        .collect();
    (frames, ended)
}

/// The release velocity of the scroll stream, computed from the socket messages.
fn release(steps: &[Step], cfg: &Config, end: Mono) -> f64 {
    let rule = ReleaseRule::from_settings(&cfg.momentum);
    let samples = steps.iter().flat_map(|s| s.outs.iter()).fold(
        ReleaseSamples::default(),
        |acc, o| match o {
            Out::Host(ToHost::Scroll {
                phase: Phase::Changed,
                dy_px,
                t_ns,
                ..
            }) => acc.push(Mono(*t_ns), f64::from(*dy_px), &rule),
            _ => acc,
        },
    );
    samples.velocity(end, &rule)
}

/// The px the pad's contacts travelled on y after `from` (a Land puts them at the centre).
fn pad_travel_px(frames: &[Written], from: Mono, cfg: &Config) -> f64 {
    let scale = cfg.touchpad.mm_per_px.ratio() * 40.0;
    let mut pos = None;
    let mut units = 0;
    for w in frames {
        match w.frame {
            Emitted::Pad(PadFrame::Land { at, .. }) => pos = Some(at.y),
            Emitted::Pad(PadFrame::Move { at }) => {
                if w.at > from {
                    units += at.y - pos.expect("a move while down");
                }
                pos = Some(at.y);
            }
            Emitted::Pad(PadFrame::Lift) => pos = None,
            _ => {}
        }
    }
    f64::from(units) / scale
}

// ---- foreign_momentum = "ours", touchpad --------------------------------------------------

#[test]
fn with_toolkit_momentum_the_pad_lifts_at_ended() {
    let cfg = Config::default();
    let steps = replay(&support::load("flick_two_vertical"), cfg, Timing::Arrival);
    let (frames, ended) = written(&steps);
    let after: Vec<&Written> = frames.iter().filter(|w| w.at >= ended).collect();
    assert_eq!(after.len(), 1, "one frame after Ended");
    assert_eq!(after[0].frame, Emitted::Pad(PadFrame::Lift));
}

#[test]
fn ours_glides_the_contacts_along_the_drag_curve_then_lifts() {
    let cfg = ours(ForeignOutput::Touchpad);
    let steps = replay(
        &support::load("flick_two_vertical"),
        cfg.clone(),
        Timing::Arrival,
    );
    let (frames, ended) = written(&steps);
    let glide: Vec<&Written> = frames.iter().filter(|w| w.at > ended).collect();
    let v0 = release(&steps, &cfg, ended);
    assert!(v0 > 1500.0, "a flick: {v0} px/s");
    let drag = Drag::from_settings(&cfg.momentum);

    let travel = pad_travel_px(&frames, ended, &cfg);
    let want = drag.total(v0);
    assert!(
        (travel - want).abs() / want < 0.05,
        "{travel} px, want {want}"
    );
    assert!(travel > 0.0, "the glide continues the stroke's direction");

    let last = glide.last().unwrap();
    assert_eq!(last.frame, Emitted::Pad(PadFrame::Lift), "lifts at the end");
    let lasted = last.at.seconds_since(ended);
    assert!(
        (lasted - drag.duration(v0)).abs() < 0.009,
        "{lasted} s, want {}",
        drag.duration(v0)
    );
    // The lift comes at the stop speed: the last move is a unit or less.
    let moves: Vec<i32> = glide
        .iter()
        .filter_map(|w| match w.frame {
            Emitted::Pad(PadFrame::Move { at }) => Some(at.y),
            _ => None,
        })
        .collect();
    let n = moves.len();
    assert!(n > 40, "frame by frame: {n}");
    assert!(
        (moves[n - 1] - moves[n - 2]).abs() <= 1,
        "{:?}",
        &moves[n - 5..]
    );
}

#[test]
fn near_the_edge_the_glide_relands_with_no_pause() {
    let cfg = ours(ForeignOutput::Touchpad);
    let steps = replay(&support::load("flick_two_vertical"), cfg, Timing::Arrival);
    let (frames, ended) = written(&steps);
    let glide: Vec<&Written> = frames.iter().filter(|w| w.at > ended).collect();
    let lifts: Vec<usize> = (0..glide.len())
        .filter(|&i| glide[i].frame == Emitted::Pad(PadFrame::Lift))
        .collect();
    assert!(
        lifts.len() >= 3,
        "a 0.25 mm/px glide of ~500 px re-lands: {lifts:?}"
    );
    for &i in &lifts[..lifts.len() - 1] {
        let next = glide[i + 1];
        assert!(
            matches!(next.frame, Emitted::Pad(PadFrame::Land { at, fingers: 2, .. }) if at == CENTRE),
            "a re-land is a lift then a landing at the centre"
        );
        assert_eq!(next.at, glide[i].at, "in the same frame, no pause");
    }
}

#[test]
fn a_new_touch_stops_the_glide_at_once() {
    let cfg = ours(ForeignOutput::Touchpad);
    let records = support::load("flick_then_touch");
    let touch = Mono::from_millis(400);
    let steps = replay(&records, cfg.clone(), Timing::Arrival);
    let (frames, ended) = written(&steps);
    assert_eq!(ended, Mono::from_millis(204), "only the flick locks");
    let at_touch: Vec<&Written> = frames.iter().filter(|w| w.at == touch).collect();
    assert_eq!(
        at_touch.last().map(|w| &w.frame),
        Some(&Emitted::Pad(PadFrame::Lift)),
        "the touch lifts the gliding contacts"
    );
    assert!(
        frames.iter().all(|w| w.at <= touch),
        "nothing moves after the touch"
    );
    let v0 = release(&steps, &cfg, ended);
    let drag = Drag::from_settings(&cfg.momentum);
    assert!(drag.duration(v0) > 0.3, "the glide would have run on");
}

#[test]
fn a_claim_stops_the_glide_and_a_claimed_flick_never_glides() {
    let cfg = ours(ForeignOutput::Touchpad);
    let records = support::load("flick_two_vertical");
    let (mut t, mut clock) = (Tracker::new(cfg.clone()), StampClock::new());
    for r in &records {
        let (nt, nc, _) = feed(t, clock, r, Timing::Arrival);
        (t, clock) = (nt, nc);
    }
    for _ in 0..5 {
        let d = t.deadline().expect("gliding");
        t = t.tick(d).0;
    }
    let (t, lifted) = t.claim(Over::Ours);
    assert_eq!(lifted, [Out::Pad(PadFrame::Lift)]);
    assert_eq!(t.deadline(), None, "no glide left to run");

    let (mut t, _) = Tracker::new(cfg).claim(Over::Ours);
    let mut all = Vec::new();
    for r in &records {
        let (nt, _, step) = feed(t, StampClock::new(), r, Timing::Arrival);
        t = nt;
        all.extend(step.outs);
    }
    assert_eq!(t.deadline(), None);
    assert!(!all.iter().any(|o| matches!(o, Out::Pad(_) | Out::Wheel(_))));
}

// ---- foreign_momentum = "ours", wheel -----------------------------------------------------

#[test]
fn on_the_wheel_the_glide_is_a_decaying_run_of_hi_res_frames() {
    let toolkit = Config {
        foreign_output: ForeignOutput::Wheel,
        ..Config::default()
    };
    let records = support::load("flick_two_vertical");
    let (frames, ended) = written(&replay(&records, toolkit, Timing::Arrival));
    assert!(
        frames.iter().all(|w| w.at <= ended),
        "the Python wheel has no glide"
    );

    let cfg = ours(ForeignOutput::Wheel);
    let steps = replay(&records, cfg.clone(), Timing::Arrival);
    let (frames, ended) = written(&steps);
    let glide: Vec<i32> = frames
        .iter()
        .filter(|w| w.at > ended)
        .map(|w| match w.frame {
            Emitted::Wheel(d) => d.vertical_hi_res,
            ref other => panic!("only wheel frames: {other:?}"),
        })
        .collect();
    let stroke_sign = frames
        .iter()
        .find_map(|w| match w.frame {
            Emitted::Wheel(d) if d.vertical_hi_res != 0 => Some(d.vertical_hi_res.signum()),
            _ => None,
        })
        .unwrap();
    assert!(glide.iter().all(|u| u.signum() == stroke_sign), "{glide:?}");
    let px = f64::from(glide.iter().sum::<i32>().abs()) * 60.0 / 120.0;
    let want = Drag::from_settings(&cfg.momentum).total(release(&steps, &cfg, ended));
    assert!((px - want).abs() / want < 0.05, "{px} px, want {want}");
    assert!(
        glide[0].abs() > 4 * glide[glide.len() - 1].abs(),
        "{glide:?}"
    );
}

// ---- feel.curve = "mac_like" --------------------------------------------------------------

#[test]
fn mac_like_is_precise_slow_and_faster_on_a_flick() {
    let cfg = Config::default();
    let slow = compare_curves(&support::load("parity_slow"), &cfg, Timing::Arrival);
    assert_eq!(slow.mac_like.peak_gain, 1.0, "slow motion is 1:1");
    assert!(slow.classic.peak_gain > 1.0);
    assert!(slow.mac_like.cumulative_px < slow.classic.cumulative_px);
    assert_eq!(
        slow.mac_like.strokes, slow.classic.strokes,
        "the lock is the same"
    );

    let flick = compare_curves(&support::load("flick_two_vertical"), &cfg, Timing::Arrival);
    assert!(flick.mac_like.peak_gain > 3.0, "{:?}", flick.mac_like);
    assert!(
        (flick.classic.peak_gain - 2.0).abs() < 1e-9,
        "{:?}",
        flick.classic
    );
    assert!(flick.mac_like.cumulative_px > 1.3 * flick.classic.cumulative_px);
    assert!(flick.table().contains("mac-like"));
}

#[test]
fn the_default_curve_is_the_python_feel() {
    // The parity suite (acceptance_2) runs with the defaults; here the explicit `Classic` is
    // shown to be those defaults, frame for frame.
    let mut explicit = Config::default();
    explicit.feel.curve = Curve::Classic;
    explicit.foreign_momentum = ForeignMomentum::Toolkit;
    for name in ["parity_fast", "parity_flicks", "flick_two_vertical"] {
        let records = support::load(name);
        assert_eq!(
            replay(&records, Config::default(), Timing::Arrival),
            replay(&records, explicit.clone(), Timing::Arrival),
            "{name}"
        );
    }
}
