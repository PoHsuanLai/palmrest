//! Every synthesised fixture, by name. The committed `.bin` files are these, byte for byte
//! (`tests/fixtures_current.rs` checks it; `PALMREST_REGEN_FIXTURES=1` rewrites them).
//!
//! Coordinates are the kernel frame (+x right, +y toward the user) unless a scenario is a
//! reference case (`case_*`), which states its points in the unsigned reading.

use super::case_points::{case_count_change, case_long_drag, drag};
use super::sessions::{feel_diagonal, feel_fast, feel_fingers, feel_flicks, feel_slow};
use super::strokes::{
    click_during_scroll, mouse_moving, one_finger_swipe, pair_stroke, palm_during_scroll,
    rest_side_band, swipe_199, swipe_vertical_then_horizontal, taps,
};
use super::{Script, finger, unsigned_finger};
use palmrest::recording::Record;
use palmrest::sensor::magic_mouse::encode::encode_double;
use palmrest::sensor::magic_mouse::report::Buttons;

/// Header of the decoder goldens: left button, dx 5, dy -3, ticks 3147.
pub const GOLDEN_HEADER: [u8; 14] = [
    0x12, 0x01, 0x05, 0x00, 0xfd, 0xff, 0, 0, 0, 0, 0, 0xc0, 0x12, 0x03,
];
/// id 6 at (1100, 1217), major 64, minor 48, size 12, orientation -15, Touching.
pub const GOLDEN_A: [u8; 8] = [0x4c, 0xf4, 0xb3, 0x40, 0x30, 0x8c, 0x45, 0x40];
/// id 0 at (-2048, 2048), major 16, minor 16, size 5, orientation 0, MakeTouch.
pub const GOLDEN_B: [u8; 8] = [0x00, 0x08, 0x80, 0x10, 0x10, 0x05, 0x80, 0x30];
/// id 12 at (-1, -487), major 5, minor 4, size 63, orientation 30, OutOfRange.
pub const GOLDEN_C: [u8; 8] = [0xff, 0x7f, 0x1e, 0x05, 0x04, 0x3f, 0xfb, 0x70];

fn golden(touches: &[[u8; 8]]) -> Vec<u8> {
    let mut b = GOLDEN_HEADER.to_vec();
    touches.iter().for_each(|t| b.extend(t));
    b
}

/// A fast two-finger flick toward the user: 80 units every 12 ms (6667 units/s), then lift.
fn flick() -> Script {
    pair_stroke(Script::default(), 0.0, 16, (0, 80), (0, -1200)).lift(204.0)
}

/// Every fixture, by file name.
pub fn all() -> Vec<(&'static str, Vec<Record>)> {
    let s = Script::default;
    let one_vertical = drag(|i| vec![unsigned_finger(1, 1000, 1000 + i * 6)], 40);
    let one_horizontal = drag(|i| vec![unsigned_finger(1, 1000 + i * 6, 1000)], 40);
    let two_horizontal = drag(
        |i| {
            vec![
                unsigned_finger(1, 900 + i * 6, 1000),
                unsigned_finger(2, 1300 + i * 6, 1000),
            ]
        },
        40,
    );
    let swipe_right = drag(
        |i| {
            vec![
                unsigned_finger(1, 900 + i * 10, 1000),
                unsigned_finger(2, 1300 + i * 10, 1000),
            ]
        },
        40,
    )
    .lift(11_000.0);
    let vertical_drag = drag(
        |i| {
            vec![
                unsigned_finger(1, 900 + i * 3, 1000 + i * 10),
                unsigned_finger(2, 1300 + i * 3, 1000 + i * 10),
            ]
        },
        60,
    );
    let one_never = drag(|i| vec![unsigned_finger(1, 500 + i * 20, 1000)], 60);
    let two_vertical = drag(
        |i| {
            vec![
                unsigned_finger(1, 900, 1000 + i * 6),
                unsigned_finger(2, 1300, 1000 + i * 6),
            ]
        },
        40,
    );
    let three = drag(
        |i| {
            (1..=3)
                .map(|k| unsigned_finger(k, 500 + 400 * i32::from(k), 1000 + i * 6))
                .collect()
        },
        40,
    );
    let fine = drag(|i| vec![unsigned_finger(1, 1000, 1000 + i * 6)], 40);
    let double = encode_double(&golden(&[GOLDEN_A]), &golden(&[GOLDEN_A, GOLDEN_B]));
    let fixtures: Vec<(&'static str, Script)> = vec![
        ("decode_n0", s().raw(0.0, golden(&[]))),
        ("decode_n1", s().raw(0.0, golden(&[GOLDEN_A]))),
        ("decode_n2", s().raw(0.0, golden(&[GOLDEN_A, GOLDEN_B]))),
        (
            "decode_n3",
            s().raw(0.0, golden(&[GOLDEN_A, GOLDEN_B, GOLDEN_C])),
        ),
        (
            "decode_header_only",
            s().raw(0.0, vec![0x12, 0x02, 0xff, 0xff, 0x01, 0x00, 0x00, 0x00]),
        ),
        ("decode_double", s().raw(0.0, double)),
        ("decode_len13", s().raw(0.0, GOLDEN_HEADER[..13].to_vec())),
        (
            "decode_len15",
            s().raw(0.0, golden(&[GOLDEN_A])[..15].to_vec()),
        ),
        (
            "decode_len29",
            s().raw(0.0, golden(&[GOLDEN_A, GOLDEN_B])[..29].to_vec()),
        ),
        ("feel_slow", feel_slow()),
        ("feel_fast", feel_fast()),
        ("feel_flicks", feel_flicks()),
        ("feel_diagonal", feel_diagonal()),
        ("feel_fingers", feel_fingers()),
        ("case_one_vertical", one_vertical),
        ("case_one_horizontal", one_horizontal),
        ("case_two_never_horizontal", two_horizontal),
        ("case_swipe_right", swipe_right),
        ("case_long_drag", case_long_drag()),
        ("case_vertical_no_switch", vertical_drag),
        ("case_one_never_switches", one_never),
        ("case_two_vertical", two_vertical),
        ("case_three_nothing", three),
        ("case_count_change", case_count_change()),
        ("case_fine_steps", fine),
        (
            "swipe_200_right",
            pair_stroke(s(), 0.0, 25, (8, 0), (-100, 0)).lift(312.0),
        ),
        (
            "swipe_200_left",
            pair_stroke(s(), 0.0, 25, (-8, 0), (100, 0)).lift(312.0),
        ),
        ("swipe_199", swipe_199()),
        (
            "swipe_vertical_then_horizontal",
            swipe_vertical_then_horizontal(),
        ),
        (
            "swipe_600_fast",
            pair_stroke(s(), 0.0, 33, (18, 0), (-300, 0))
                .at(
                    400.0,
                    vec![finger(1, -500 + 600, 0), finger(2, -100 + 600, 0)],
                )
                .lift(410.0),
        ),
        ("tap_two_double", taps(2, 250.0, 0, Buttons::NONE)),
        ("tap_two_gap400", taps(2, 400.0, 0, Buttons::NONE)),
        ("tap_two_moving45", taps(2, 250.0, 45, Buttons::NONE)),
        ("tap_two_click", taps(2, 250.0, 0, Buttons::LEFT)),
        ("tap_one_double", taps(1, 250.0, 0, Buttons::NONE)),
        ("g4_fast", one_finger_swipe(250.0)),
        ("g4_slow", one_finger_swipe(600.0)),
        ("rest_side_band", rest_side_band()),
        ("click_during_scroll", click_during_scroll()),
        ("palm_during_scroll", palm_during_scroll()),
        ("mouse_moving", mouse_moving()),
        ("flick_two_vertical", flick()),
        (
            "flick_then_touch",
            flick()
                .frames(400.0, 12.0, 10, |_| {
                    vec![finger(1, -200, 0), finger(2, 200, 0)]
                })
                .lift(532.0),
        ),
    ];
    fixtures.into_iter().map(|(n, s)| (n, s.records)).collect()
}
