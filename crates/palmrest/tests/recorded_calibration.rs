//! Palm rejection calibrated on the development machine's Magic Mouse 2 (Bluetooth), 2026-09-30.
//!
//! Two sessions recorded with `palmrest record`: `recorded_scroll` (ordinary one-finger
//! scrolling, slow and fast, both axes) and `recorded_rest` (holding, moving, clicking and
//! resting the hand on the mouse; the first second is the end of a scroll). The default
//! rejection must keep the scrolling and drop the resting: on this device fingers measure 120 to
//! 223 major units and resting contacts 70 to 196, so size cannot tell them apart and only
//! motion does (P2 and the axis lock).

mod support;

use palmrest::Out;
use palmrest::config::{Config, ForeignOutput, Rejection};
use palmrest::replay::{Timing, replay};

/// Summed hi-res wheel units, all of them and those after `skip_ms` from the first record.
fn wheel_units(name: &str, rejection: Rejection, skip_ms: u64) -> (i64, i64) {
    let records = support::load(name);
    let start = records.first().map_or(0, |r| r.at.0);
    let cfg = Config {
        rejection,
        foreign_output: ForeignOutput::Wheel,
        ..Config::default()
    };
    let (mut all, mut late) = (0i64, 0i64);
    for step in replay(&records, cfg, Timing::Arrival) {
        for out in &step.outs {
            if let Out::Wheel(w) = out {
                let units =
                    i64::from(w.horizontal_hi_res.abs()) + i64::from(w.vertical_hi_res.abs());
                all += units;
                if step.at.0.saturating_sub(start) >= skip_ms * 1_000_000 {
                    late += units;
                }
            }
        }
    }
    (all, late)
}

#[test]
fn default_rejection_keeps_recorded_scrolling() {
    let (off, _) = wheel_units("recorded_scroll", Rejection::Off, 0);
    let (on, _) = wheel_units("recorded_scroll", Rejection::On, 0);
    assert!(off > 10_000, "the recording scrolls: {off}");
    assert!(on * 100 >= off * 95, "rejection keeps {on} of {off} units");
}

#[test]
fn default_rejection_drops_a_resting_hand() {
    // The first second is the tail of a scroll, not rest.
    let (_, off) = wheel_units("recorded_rest", Rejection::Off, 1000);
    let (_, on) = wheel_units("recorded_rest", Rejection::On, 1000);
    assert!(
        on <= 40,
        "a resting hand leaks {on} units (rejection off: {off})"
    );
    assert!(on < off, "rejection drops something: {on} vs {off}");
}
