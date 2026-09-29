//! Acceptance 12.8 item 2 and 11.8 item 16: the reference scroll feel.
//!
//! Five sessions replayed through palmrest (rejection Off, wheel output) against the goldens in
//! `tests/fixtures/reference/feel_*.golden`: identical lock decisions per report, identical
//! cumulative v120 on the wheel, and cumulative socket px equal to v120 x 0.5 within one v120
//! unit on every report. The goldens are frozen: they were produced once by the retired Python
//! daemon, and nothing regenerates them.
//!
//! The px comparison is cumulative per locked stroke (from each Began). The reference lock
//! throws away its sub-unit remainder at every lock and unlock; the socket carries exact px and
//! has nothing to throw away, so over a whole session the two drift apart by up to one unit per
//! stroke even though every stroke agrees.

mod support;

use palmrest::Out;
use palmrest::config::{Config, ForeignOutput, Rejection};
use palmrest::feel::Lock;
use palmrest::replay::{Timing, replay};
use palmrest_protocol::{Phase, ToHost};

const SESSIONS: [&str; 5] = [
    "feel_slow",
    "feel_fast",
    "feel_flicks",
    "feel_diagonal",
    "feel_fingers",
];

struct GoldenLine {
    axis: String,
    h: i32,
    v: i32,
    hh: i32,
    vh: i32,
}

fn golden(name: &str) -> Vec<GoldenLine> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/reference/{name}.golden"));
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            let n = |i: usize| f[i].parse::<i32>().unwrap();
            GoldenLine {
                axis: f[0].to_owned(),
                h: n(1),
                v: n(2),
                hh: n(3),
                vh: n(4),
            }
        })
        .collect()
}

fn axis_name(l: Lock) -> &'static str {
    match l {
        Lock::Free { .. } => "none",
        Lock::X => "x",
        Lock::Y => "y",
        Lock::Blocked => "blocked",
    }
}

fn reference_config() -> Config {
    Config {
        rejection: Rejection::Off,
        foreign_output: ForeignOutput::Wheel,
        ..Config::default()
    }
}

#[test]
fn lock_decisions_and_wheel_units_match_the_reference_per_report() {
    for name in SESSIONS {
        let steps = replay(&support::load(name), reference_config(), Timing::Arrival);
        let py = golden(name);
        assert_eq!(steps.len(), py.len(), "{name}: record count");
        let (mut ours, mut theirs) = ([0i64; 4], [0i64; 4]);
        let (mut px_x, mut px_y) = (0.0f64, 0.0f64);
        let mut stroke_start = [0i64; 4];
        let mut locks = std::collections::BTreeSet::new();
        for (i, (s, p)) in steps.iter().zip(&py).enumerate() {
            assert_eq!(
                axis_name(s.lock),
                p.axis,
                "{name}: lock decision at record {i}"
            );
            locks.insert(p.axis.clone());
            for o in &s.outs {
                match o {
                    Out::Wheel(w) => {
                        for (acc, v) in ours.iter_mut().zip([
                            w.horizontal,
                            w.vertical,
                            w.horizontal_hi_res,
                            w.vertical_hi_res,
                        ]) {
                            *acc += i64::from(v);
                        }
                    }
                    Out::Host(ToHost::Scroll {
                        phase: Phase::Began,
                        ..
                    }) => {
                        (px_x, px_y) = (0.0, 0.0);
                        stroke_start = ours;
                    }
                    Out::Host(ToHost::Scroll { dx_px, dy_px, .. }) => {
                        px_x += f64::from(*dx_px);
                        px_y += f64::from(*dy_px);
                    }
                    _ => {}
                }
            }
            for (acc, v) in theirs.iter_mut().zip([p.h, p.v, p.hh, p.vh]) {
                *acc += i64::from(v);
            }
            assert_eq!(ours, theirs, "{name}: cumulative wheel units at record {i}");
            // Socket px are finger direction in the kernel frame; the wheel's y is the unsigned
            // reading, the other way up.
            let stroke = |k: usize| (theirs[k] - stroke_start[k]) as f64 * 0.5;
            let (want_x, want_y) = (stroke(2), -stroke(3));
            assert!(
                (px_x - want_x).abs() <= 0.5 + 1e-3,
                "{name}: px x {px_x} vs {want_x} at record {i}"
            );
            assert!(
                (px_y - want_y).abs() <= 0.5 + 1e-3,
                "{name}: px y {px_y} vs {want_y} at record {i}"
            );
        }
        assert!(
            theirs[2] != 0 || theirs[3] != 0,
            "{name}: the session scrolled"
        );
        eprintln!(
            "{name}: {} records, locks seen {locks:?}, v120 h {} v {}",
            steps.len(),
            theirs[2],
            theirs[3]
        );
    }
}

#[test]
fn sessions_cover_every_lock_decision() {
    let seen: std::collections::BTreeSet<String> = SESSIONS
        .iter()
        .flat_map(|n| golden(n))
        .map(|l| l.axis)
        .collect();
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        ["blocked", "none", "x", "y"]
    );
}
