//! Acceptance 12.8 item 1: decoder goldens.

use palmrest::sensor::magic_mouse::decode::{DecodeError, decode};
use palmrest::sensor::magic_mouse::report::{Buttons, Motion, Report, Stamp};
use palmrest::touch::{Point, RawTouch, TouchId, TouchState, wrap_delta};

fn one(name: &str) -> Result<Vec<Report>, DecodeError> {
    let records = crate::support::load(name);
    assert_eq!(records.len(), 1, "{name}");
    decode(&records[0].bytes)
}

const A: RawTouch = RawTouch {
    id: TouchId(6),
    at: Point::new(1100, 1217),
    major: 64,
    minor: 48,
    size: 12,
    orientation: -15,
    state: TouchState::Touching,
};
const B: RawTouch = RawTouch {
    id: TouchId(0),
    at: Point::new(-2048, 2048),
    major: 16,
    minor: 16,
    size: 5,
    orientation: 0,
    state: TouchState::MakeTouch,
};
const C: RawTouch = RawTouch {
    id: TouchId(12),
    at: Point::new(-1, -487),
    major: 5,
    minor: 4,
    size: 63,
    orientation: 30,
    state: TouchState::OutOfRange,
};

fn header(touches: Vec<RawTouch>) -> Report {
    Report {
        buttons: Buttons::LEFT,
        motion: Motion { dx: 5, dy: -3 },
        stamp: Some(Stamp::Ticks(3147)),
        touches,
    }
}

#[test]
fn goldens_decode_to_exact_fields_or_errors() {
    let cases: Vec<(&str, Result<Vec<Report>, DecodeError>)> = vec![
        ("decode_n0", Ok(vec![header(vec![])])),
        ("decode_n1", Ok(vec![header(vec![A])])),
        ("decode_n2", Ok(vec![header(vec![A, B])])),
        ("decode_n3", Ok(vec![header(vec![A, B, C])])),
        (
            "decode_header_only",
            Ok(vec![Report {
                buttons: Buttons::RIGHT,
                motion: Motion { dx: -1, dy: 1 },
                stamp: None,
                touches: vec![],
            }]),
        ),
        (
            "decode_double",
            Ok(vec![header(vec![A]), header(vec![A, B])]),
        ),
        ("decode_len13", Err(DecodeError::Length { len: 13 })),
        ("decode_len15", Err(DecodeError::Length { len: 15 })),
        ("decode_len29", Err(DecodeError::Length { len: 29 })),
    ];
    for (name, want) in cases {
        assert_eq!(one(name), want, "{name}");
    }
}

/// The reference parser's reading of the same bytes, from the frozen goldens
/// (`tests/fixtures/reference`).
fn reference_touches(name: &str) -> Vec<(u8, i32, i32, u8, u8)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/reference/{name}.golden"));
    let text = std::fs::read_to_string(path).unwrap();
    let line = text.lines().next().unwrap_or("");
    line.split(';')
        .filter(|s| !s.is_empty())
        .map(|t| {
            let v: Vec<i32> = t.split(' ').map(|n| n.parse().unwrap()).collect();
            (v[0] as u8, v[1], v[2], v[3] as u8, v[4] as u8)
        })
        .collect()
}

#[test]
fn kernel_reading_agrees_with_the_reference_parser() {
    for name in ["decode_n1", "decode_n2", "decode_n3"] {
        let ours = one(name).unwrap().remove(0).touches;
        let theirs = reference_touches(name);
        assert_eq!(ours.len(), theirs.len(), "{name}");
        for (o, &(id, x, y, state, size)) in ours.iter().zip(&theirs) {
            assert_eq!(
                (o.id.0, o.state.nibble(), o.size),
                (id, state, size),
                "{name}"
            );
            assert_eq!(
                o.at.x & 0xfff,
                x,
                "{name}: x is the unsigned reading, sign-extended"
            );
            assert_eq!(
                (-o.at.y) & 0xfff,
                y,
                "{name}: y is the unsigned reading, sign-extended and negated"
            );
        }
        // Deltas agree after the unsigned reading's wrap fix (and the y negation).
        for (p, q) in ours.iter().zip(ours.iter().skip(1)) {
            let (pp, qq) = (
                theirs.iter().find(|t| t.0 == p.id.0).unwrap(),
                theirs.iter().find(|t| t.0 == q.id.0).unwrap(),
            );
            assert_eq!(
                wrap_delta(q.at.x, p.at.x),
                wrap_delta(qq.1, pp.1),
                "{name}: dx"
            );
            assert_eq!(
                wrap_delta(q.at.y, p.at.y),
                -wrap_delta(qq.2, pp.2),
                "{name}: dy"
            );
        }
    }
}
