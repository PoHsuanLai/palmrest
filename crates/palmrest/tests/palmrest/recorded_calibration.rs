//! Palm rejection calibrated on the development machine's Magic Mouse 2 (Bluetooth), 2026-09-30.
//!
//! Two sessions recorded with `palmrest record`: `recorded_scroll` (ordinary one-finger
//! scrolling, slow and fast, both axes) and `recorded_rest` (holding, moving, clicking and
//! resting the hand on the mouse; the first second is the end of a scroll). The default
//! rejection must keep the scrolling and drop the resting: on this device fingers measure 120 to
//! 223 major units and resting contacts 70 to 196, so size cannot tell them apart and only
//! motion does (P2 and the axis lock).

use palmrest::config::Config;
use palmrest::replay::{Timing, replay};

/// Summed scrolled px (both axes), all of them and those after `skip_ms` from the first record.
fn scrolled_px(name: &str, skip_ms: u64) -> (f64, f64) {
    let records = crate::support::load(name);
    let start = records.first().map_or(0, |r| r.at.0);
    let (mut all, mut late) = (0.0, 0.0);
    for step in replay(&records, Config::default(), Timing::Arrival) {
        let (x, y) = crate::support::scrolled(&step.outs);
        let px = x.abs() + y.abs();
        all += px;
        if step.at.0.saturating_sub(start) >= skip_ms * 1_000_000 {
            late += px;
        }
    }
    (all, late)
}

#[test]
fn default_rejection_keeps_recorded_scrolling_and_drops_a_resting_hand() {
    // The recorded scroll survives rejection: it is long, and any acceleration only adds to it.
    let (scroll, _) = scrolled_px("recorded_scroll", 0);
    assert!(scroll > 1_000.0, "the recording scrolls: {scroll} px");

    // A resting hand is dropped. The first second is the tail of a scroll, not rest.
    let (_, rest) = scrolled_px("recorded_rest", 1000);
    assert!(rest <= 40.0, "a resting hand leaks {rest} px");
}
