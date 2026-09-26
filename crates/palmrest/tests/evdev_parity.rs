//! The evdev sensor against the hidraw one: every fixture's reports, re-expressed as the
//! `ABS_MT_*` events the kernel driver would send for them, replay to the same outputs through
//! `EvdevMultitouch` as the raw bytes do through `MagicMouseHid`, except where the Magic
//! Mouse's contact size and major axis decide (P1, P3), which evdev does not carry.
//!
//! Synthetic: the kernel is modelled as reporting only touches in MakeTouch or Touching, and
//! its fuzz filter is not modelled (FINDINGS F25).

mod support;

use palmrest::Config;
use palmrest::outputs::events::{
    ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_SLOT, ABS_MT_TRACKING_ID, BTN_LEFT, EV_ABS,
    EV_KEY, EV_REL, EV_SYN, InputEvent, SYN_REPORT,
};
use palmrest::recording::{Record, RecordedSource};
use palmrest::replay::{Step, Timing, replay_from};
use palmrest::report::{Buttons, Report};
use palmrest::sensor::evdev::{AbsAxis, EvdevSpec, TimedEvent};
use palmrest::touch::TouchState;
use palmrest::{Mono, decode};
use std::collections::BTreeMap;

const BTN_RIGHT: u16 = 0x111;
const REL_X: u16 = 0;
const REL_Y: u16 = 1;

/// The Magic Mouse's node as the kernel describes it.
fn spec() -> EvdevSpec {
    let axis = |min, max, resolution| AbsAxis {
        min,
        max,
        resolution,
    };
    EvdevSpec::new(axis(-1100, 1258, 26), axis(-1589, 2047, 70), 15).unwrap()
}

/// What the kernel last reported, so only changes are sent (as the input core does).
#[derive(Default)]
struct Kernel {
    live: BTreeMap<u8, (i32, i32)>,
    buttons: Buttons,
}

fn ev(kind: u16, code: u16, value: i32) -> InputEvent {
    InputEvent { kind, code, value }
}

impl Kernel {
    fn frame(&mut self, r: &Report) -> Vec<InputEvent> {
        let mut out = Vec::new();
        let now: BTreeMap<u8, (i32, i32)> = r
            .touches
            .iter()
            .filter(|t| matches!(t.state, TouchState::MakeTouch | TouchState::Touching))
            .map(|t| (t.id.0, (t.at.x, t.at.y)))
            .collect();
        for id in self.live.keys().filter(|id| !now.contains_key(id)) {
            out.extend([
                ev(EV_ABS, ABS_MT_SLOT, i32::from(*id)),
                ev(EV_ABS, ABS_MT_TRACKING_ID, -1),
            ]);
        }
        for (id, &(x, y)) in &now {
            out.push(ev(EV_ABS, ABS_MT_SLOT, i32::from(*id)));
            let before = self.live.get(id).copied();
            if before.is_none() {
                out.push(ev(EV_ABS, ABS_MT_TRACKING_ID, i32::from(*id)));
            }
            if before.map(|b| b.0) != Some(x) {
                out.push(ev(EV_ABS, ABS_MT_POSITION_X, x));
            }
            if before.map(|b| b.1) != Some(y) {
                out.push(ev(EV_ABS, ABS_MT_POSITION_Y, y));
            }
        }
        for (bit, code) in [(Buttons::LEFT, BTN_LEFT), (Buttons::RIGHT, BTN_RIGHT)] {
            let (was, is) = (self.buttons.0 & bit.0, r.buttons.0 & bit.0);
            if was != is {
                out.push(ev(EV_KEY, code, i32::from(is != 0)));
            }
        }
        for (code, v) in [(REL_X, r.motion.dx), (REL_Y, r.motion.dy)] {
            if v != 0 {
                out.push(ev(EV_REL, code, i32::from(v)));
            }
        }
        out.push(ev(EV_SYN, SYN_REPORT, 0));
        self.live = now;
        self.buttons = r.buttons;
        out
    }
}

/// A hidraw recording as the evdev node would have delivered it.
fn as_evdev(records: &[Record]) -> Vec<Record> {
    let mut kernel = Kernel::default();
    records
        .iter()
        .map(|r| {
            let events = decode(&r.bytes).unwrap_or_default();
            let bytes = events
                .iter()
                .flat_map(|rep| kernel.frame(rep))
                .flat_map(|event| TimedEvent { at: r.at, event }.to_bytes())
                .collect();
            Record { at: r.at, bytes }
        })
        .collect()
}

/// The outputs with the time they happened at.
fn timed(steps: &[Step]) -> Vec<(Mono, String)> {
    steps
        .iter()
        .flat_map(|s| s.outs.iter().map(move |o| (s.at, format!("{o:?}"))))
        .collect()
}

/// Fixtures whose outcome rests on the Magic Mouse's size or major axis, which evdev does not
/// carry: a palm landing mid-scroll (P3) cancels on hidraw and scrolls on through evdev.
const SHAPE_DEPENDENT: &[&str] = &["palm_during_scroll"];

#[test]
fn every_fixture_replays_the_same_through_evdev() {
    let dir = support::fixture_path("x");
    let mut names: Vec<String> = std::fs::read_dir(dir.parent().unwrap())
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "bin").then(|| p.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    names.sort();
    assert!(names.len() > 30, "found the fixtures");
    let mut differ = Vec::new();
    let mut compared = 0;
    for name in &names {
        let hid = support::load(name);
        let cfg = Config::default();
        let (_, a) = replay_from(
            &RecordedSource::MagicMouseHid,
            &hid,
            cfg.clone(),
            Timing::Arrival,
        );
        let (_, b) = replay_from(
            &RecordedSource::Evdev(spec()),
            &as_evdev(&hid),
            cfg,
            Timing::Arrival,
        );
        compared += timed(&a).len();
        if timed(&a) != timed(&b) {
            differ.push(name.as_str());
        }
    }
    assert_eq!(differ, SHAPE_DEPENDENT);
    assert!(
        compared > 500,
        "the fixtures produce outputs to compare ({compared})"
    );
}
