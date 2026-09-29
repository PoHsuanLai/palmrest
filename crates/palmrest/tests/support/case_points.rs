//! The Python tests' strokes, in the Python daemon's unsigned reading.

use super::{Script, unsigned_finger};
use palmrest::touch::RawTouch;

/// The Python test's drag: samples 8 ms apart from t = 10 s.
pub fn drag(points: impl Fn(i32) -> Vec<RawTouch>, n: i32) -> Script {
    (0..n).fold(Script::default(), |s, i| {
        s.at(10_000.0 + 8.0 * f64::from(i), points(i))
    })
}

pub fn case_long_drag() -> Script {
    (0..120).fold(Script::default(), |s, i| {
        s.at(
            10_000.0 + 10.0 * f64::from(i),
            vec![
                unsigned_finger(1, 500 + i * 10, 1000),
                unsigned_finger(2, 900 + i * 10, 1000),
            ],
        )
    })
}

pub fn case_count_change() -> Script {
    drag(|i| vec![unsigned_finger(1, 1000, 1000 + i * 6)], 20).at(
        20_000.0,
        vec![unsigned_finger(1, 1000, 1120), unsigned_finger(2, 3000, 3000)],
    )
}
