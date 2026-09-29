//! Strokes for the gesture acceptance cases, in the kernel frame.

use super::{Script, finger};
use palmrest::sensor::magic_mouse::report::{Buttons, Motion};
use palmrest::touch::RawTouch;

/// A two-finger horizontal stroke: `steps` reports 12 ms apart moving `per` units each.
pub fn pair_stroke(
    s: Script,
    start: f64,
    steps: usize,
    per: (i32, i32),
    from: (i32, i32),
) -> Script {
    (0..=steps).fold(s, |s, i| {
        let (dx, dy) = (per.0 * i as i32, per.1 * i as i32);
        s.at(
            start + 12.0 * i as f64,
            vec![
                finger(1, from.0 - 200 + dx, from.1 + dy),
                finger(2, from.0 + 200 + dx, from.1 + dy),
            ],
        )
    })
}

pub fn swipe_199() -> Script {
    let s = pair_stroke(Script::default(), 0.0, 24, (8, 0), (-100, 0));
    let s = s.at(
        300.0,
        vec![finger(1, -300 + 199, 0), finger(2, 100 + 199, 0)],
    );
    s.lift(312.0)
}

pub fn swipe_vertical_then_horizontal() -> Script {
    let s = pair_stroke(Script::default(), 0.0, 20, (0, 8), (-300, -900));
    let s = pair_stroke(s, 252.0, 20, (8, 0), (-300, -740));
    let s = pair_stroke(s, 504.0, 25, (8, 0), (-140, -740));
    s.lift(820.0)
}

/// A tap of `n` fingers from `start`, `down` ms long, reports every 10 ms.
pub fn tap(
    s: Script,
    start: f64,
    down: f64,
    n: u8,
    x: i32,
    moved: i32,
    buttons: Buttons,
) -> Script {
    let frames = (down / 10.0) as usize;
    let s = (0..frames).fold(s, |s, i| {
        let shift = moved * i as i32 / (frames as i32 - 1).max(1);
        let touches = (0..n)
            .map(|k| finger(k + 1, x + i32::from(k) * 400 + shift, 0))
            .collect();
        s.report(start + 10.0 * i as f64, touches, buttons, Motion::default())
    });
    s.lift(start + down)
}

pub fn taps(n: u8, gap: f64, first_moved: i32, first_buttons: Buttons) -> Script {
    let s = tap(
        Script::default(),
        0.0,
        120.0,
        n,
        -200,
        first_moved,
        first_buttons,
    );
    tap(s, 120.0 + gap, 120.0, n, -200, 0, Buttons::NONE)
}

pub fn one_finger_swipe(duration_ms: f64) -> Script {
    let steps = (duration_ms / 10.0) as i32;
    let s = (0..=steps).fold(Script::default(), |s, i| {
        s.at(
            10.0 * f64::from(i),
            vec![finger(1, -175 + 350 * i / steps, 100)],
        )
    });
    s.lift(duration_ms + 8.0)
}

pub fn rest_side_band() -> Script {
    (0..40)
        .fold(Script::default(), |s, i| {
            s.at(
                10.0 * f64::from(i),
                vec![finger(1, -1000, 300), finger(2, 0, -800 + 8 * i)],
            )
        })
        .lift(400.0)
}

pub fn click_during_scroll() -> Script {
    let s = (0..20).fold(Script::default(), |s, i| {
        s.at(10.0 * f64::from(i), vec![finger(1, 0, -800 + 8 * i)])
    });
    let s = (20..30).fold(s, |s, i| {
        s.report(
            10.0 * f64::from(i),
            vec![finger(1, 0, -800 + 8 * i)],
            Buttons::LEFT,
            Motion::default(),
        )
    });
    s.lift(300.0)
}

pub fn palm_during_scroll() -> Script {
    let s = (0..20).fold(Script::default(), |s, i| {
        s.at(10.0 * f64::from(i), vec![finger(1, 0, -800 + 8 * i)])
    });
    let s = (20..30).fold(s, |s, i| {
        s.at(
            10.0 * f64::from(i),
            vec![RawTouch {
                major: 250,
                ..finger(1, 0, -800 + 8 * i)
            }],
        )
    });
    s.lift(300.0)
}

pub fn mouse_moving() -> Script {
    // The finger drifts 60 units while the mouse moves fast: under the doubled threshold.
    (0..20)
        .fold(Script::default(), |s, i| {
            s.report(
                10.0 * f64::from(i),
                vec![finger(1, 0, -800 + 3 * i)],
                Buttons::NONE,
                Motion { dx: 10, dy: 0 },
            )
        })
        .lift(200.0)
}
