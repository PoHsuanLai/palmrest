//! Every synthesised fixture, by name. The committed `.bin` files are these, byte for byte
//! (`tests/fixtures_current.rs` checks it; `PALMREST_REGEN_FIXTURES=1` rewrites them).
//!
//! Coordinates are the kernel frame (+x right, +y toward the user) unless a scenario ports a
//! Python test, which states its points in the Python daemon's unsigned reading.

use super::{Script, finger, python_finger};
use palmrest::encode::encode_double;
use palmrest::recording::Record;
use palmrest::report::{Buttons, Motion};
use palmrest::touch::RawTouch;

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

/// A deterministic jitter source, so "recorded-like" sessions are reproducible.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    /// An integer in `-k..=k`.
    fn jitter(&mut self, k: i64) -> i32 {
        ((self.next() % (2 * k as u64 + 1)) as i64 - k) as i32
    }

    /// A report interval around 11 ms (the mouse's rate is unpublished; 90 Hz is assumed).
    fn interval(&mut self) -> f64 {
        9.0 + (self.next() % 5) as f64
    }
}

/// A one- or two-finger stroke with jittered timing, from `from` moving `v` units per report.
fn stroke(
    s: Script,
    rng: &mut Lcg,
    start: f64,
    n: usize,
    fingers: &[(u8, i32, i32)],
    v: (i32, i32),
) -> (Script, f64) {
    let mut t = start;
    let mut s = s;
    for i in 0..n {
        let i = i as i32;
        let touches = fingers
            .iter()
            .map(|&(id, x, y)| finger(id, x + v.0 * i + rng.jitter(1), y + v.1 * i + rng.jitter(1)))
            .collect();
        s = s.at(t, touches);
        t += rng.interval();
    }
    (s.lift(t), t)
}

fn parity_slow() -> Script {
    let mut rng = Lcg(1);
    let (s, t) = stroke(
        Script::default(),
        &mut rng,
        1000.0,
        120,
        &[(1, 0, -900)],
        (0, 2),
    );
    // A pause long enough to unlock, then the same finger moves on.
    let (s, t) = stroke(s, &mut rng, t + 40.0, 40, &[(2, 0, -600)], (0, 3));
    let s = s
        .at(t + 300.0, vec![finger(2, 0, -480)])
        .at(t + 311.0, vec![finger(2, 0, -470)]);
    let (s, _) = stroke(s, &mut rng, t + 322.0, 60, &[(2, 0, -460)], (0, -2));
    s
}

fn parity_fast() -> Script {
    let mut rng = Lcg(2);
    (0..4).fold(Script::default(), |s, k| {
        let (s, _) = stroke(
            s,
            &mut rng,
            1000.0 + 1000.0 * f64::from(k),
            18,
            &[(1, 100, -1200)],
            (0, 40 + 5 * k),
        );
        s
    })
}

fn parity_flicks() -> Script {
    let mut rng = Lcg(3);
    let mut t = 1000.0;
    let mut s = Script::default();
    for k in 0..7 {
        let (next, end) = stroke(
            s,
            &mut rng,
            t,
            10,
            &[(1 + (k % 3) as u8, -100, -1000)],
            (0, 25),
        );
        s = next;
        t = end + 150.0;
    }
    s
}

fn parity_diagonal() -> Script {
    let mut rng = Lcg(4);
    let (s, t) = stroke(
        Script::default(),
        &mut rng,
        1000.0,
        50,
        &[(1, -600, -1000)],
        (6, 5),
    );
    let (s, t) = stroke(s, &mut rng, t + 600.0, 50, &[(1, -600, -1000)], (7, 3));
    let (s, t) = stroke(s, &mut rng, t + 600.0, 50, &[(1, 600, 500)], (-4, -6));
    let (s, _) = stroke(s, &mut rng, t + 600.0, 60, &[(1, 800, 800)], (-9, 1));
    s
}

fn parity_fingers() -> Script {
    let mut rng = Lcg(5);
    let mut s = Script::default();
    let mut t = 1000.0;
    // One finger, then a second lands mid-stroke, then the first lifts.
    for i in 0..90 {
        let y = -1000 + 4 * i;
        let mut touches = vec![];
        if i < 60 {
            touches.push(finger(1, 0, y + rng.jitter(1)));
        }
        if i >= 30 {
            touches.push(finger(2, 400, y + 50 + rng.jitter(1)));
        }
        s = s.at(t, touches);
        t += rng.interval();
    }
    let s = s.lift(t);
    // Two fingers vertical, two fingers sideways (Blocked), three fingers.
    let (s, t) = stroke(
        s,
        &mut rng,
        t + 700.0,
        40,
        &[(3, -300, -800), (4, 200, -800)],
        (0, 6),
    );
    let (s, t) = stroke(
        s,
        &mut rng,
        t + 700.0,
        40,
        &[(5, -500, 0), (6, -100, 0)],
        (8, 1),
    );
    let (s, _) = stroke(
        s,
        &mut rng,
        t + 700.0,
        30,
        &[(7, -500, 0), (8, 0, 0), (9, 500, 0)],
        (0, 6),
    );
    s
}

/// The Python test's drag: samples 8 ms apart from t = 10 s.
fn python(points: impl Fn(i32) -> Vec<RawTouch>, n: i32) -> Script {
    (0..n).fold(Script::default(), |s, i| {
        s.at(10_000.0 + 8.0 * f64::from(i), points(i))
    })
}

fn python_long_drag() -> Script {
    (0..120).fold(Script::default(), |s, i| {
        s.at(
            10_000.0 + 10.0 * f64::from(i),
            vec![
                python_finger(1, 500 + i * 10, 1000),
                python_finger(2, 900 + i * 10, 1000),
            ],
        )
    })
}

fn python_count_change() -> Script {
    python(|i| vec![python_finger(1, 1000, 1000 + i * 6)], 20).at(
        20_000.0,
        vec![python_finger(1, 1000, 1120), python_finger(2, 3000, 3000)],
    )
}

/// A two-finger horizontal stroke: `steps` reports 12 ms apart moving `per` units each.
fn pair_stroke(s: Script, start: f64, steps: usize, per: (i32, i32), from: (i32, i32)) -> Script {
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

fn swipe_199() -> Script {
    let s = pair_stroke(Script::default(), 0.0, 24, (8, 0), (-100, 0));
    let s = s.at(
        300.0,
        vec![finger(1, -300 + 199, 0), finger(2, 100 + 199, 0)],
    );
    s.lift(312.0)
}

fn swipe_vertical_then_horizontal() -> Script {
    let s = pair_stroke(Script::default(), 0.0, 20, (0, 8), (-300, -900));
    let s = pair_stroke(s, 252.0, 20, (8, 0), (-300, -740));
    let s = pair_stroke(s, 504.0, 25, (8, 0), (-140, -740));
    s.lift(820.0)
}

/// A tap of `n` fingers from `start`, `down` ms long, reports every 10 ms.
fn tap(s: Script, start: f64, down: f64, n: u8, x: i32, moved: i32, buttons: Buttons) -> Script {
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

fn taps(n: u8, gap: f64, first_moved: i32, first_buttons: Buttons) -> Script {
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

fn one_finger_swipe(duration_ms: f64) -> Script {
    let steps = (duration_ms / 10.0) as i32;
    let s = (0..=steps).fold(Script::default(), |s, i| {
        s.at(
            10.0 * f64::from(i),
            vec![finger(1, -175 + 350 * i / steps, 100)],
        )
    });
    s.lift(duration_ms + 8.0)
}

fn rest_side_band() -> Script {
    (0..40)
        .fold(Script::default(), |s, i| {
            s.at(
                10.0 * f64::from(i),
                vec![finger(1, -1000, 300), finger(2, 0, -800 + 8 * i)],
            )
        })
        .lift(400.0)
}

fn click_during_scroll() -> Script {
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

fn palm_during_scroll() -> Script {
    let s = (0..20).fold(Script::default(), |s, i| {
        s.at(10.0 * f64::from(i), vec![finger(1, 0, -800 + 8 * i)])
    });
    let s = (20..30).fold(s, |s, i| {
        s.at(
            10.0 * f64::from(i),
            vec![RawTouch {
                major: 90,
                ..finger(1, 0, -800 + 8 * i)
            }],
        )
    });
    s.lift(300.0)
}

fn mouse_moving() -> Script {
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

/// Every fixture, by file name.
pub fn all() -> Vec<(&'static str, Vec<Record>)> {
    let s = Script::default;
    let one_vertical = python(|i| vec![python_finger(1, 1000, 1000 + i * 6)], 40);
    let one_horizontal = python(|i| vec![python_finger(1, 1000 + i * 6, 1000)], 40);
    let two_horizontal = python(
        |i| {
            vec![
                python_finger(1, 900 + i * 6, 1000),
                python_finger(2, 1300 + i * 6, 1000),
            ]
        },
        40,
    );
    let swipe_right = python(
        |i| {
            vec![
                python_finger(1, 900 + i * 10, 1000),
                python_finger(2, 1300 + i * 10, 1000),
            ]
        },
        40,
    )
    .lift(11_000.0);
    let vertical_drag = python(
        |i| {
            vec![
                python_finger(1, 900 + i * 3, 1000 + i * 10),
                python_finger(2, 1300 + i * 3, 1000 + i * 10),
            ]
        },
        60,
    );
    let one_never = python(|i| vec![python_finger(1, 500 + i * 20, 1000)], 60);
    let two_vertical = python(
        |i| {
            vec![
                python_finger(1, 900, 1000 + i * 6),
                python_finger(2, 1300, 1000 + i * 6),
            ]
        },
        40,
    );
    let three = python(
        |i| {
            (1..=3)
                .map(|k| python_finger(k, 500 + 400 * i32::from(k), 1000 + i * 6))
                .collect()
        },
        40,
    );
    let fine = python(|i| vec![python_finger(1, 1000, 1000 + i * 6)], 40);
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
        ("parity_slow", parity_slow()),
        ("parity_fast", parity_fast()),
        ("parity_flicks", parity_flicks()),
        ("parity_diagonal", parity_diagonal()),
        ("parity_fingers", parity_fingers()),
        ("python_one_vertical", one_vertical),
        ("python_one_horizontal", one_horizontal),
        ("python_two_never_horizontal", two_horizontal),
        ("python_swipe_right", swipe_right),
        ("python_long_drag", python_long_drag()),
        ("python_vertical_no_switch", vertical_drag),
        ("python_one_never_switches", one_never),
        ("python_two_vertical", two_vertical),
        ("python_three_nothing", three),
        ("python_count_change", python_count_change()),
        ("python_fine_steps", fine),
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
    ];
    fixtures.into_iter().map(|(n, s)| (n, s.records)).collect()
}
