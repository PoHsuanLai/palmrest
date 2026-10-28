//! Acceptance 11, replay-only. Ignored by default because timing depends on the machine:
//! `cargo test --release -p palmrest --lib latency -- --ignored --nocapture`.

use super::measure;
use crate::config::Config;
use crate::recording::{Record, RecordedSource};
use crate::sensor::magic_mouse::encode::encode;
use crate::sensor::magic_mouse::report::{Buttons, Motion, Report, Stamp};
use crate::time::Mono;
use crate::touch::{Point, RawTouch, TouchId, TouchState};

/// A firm, finger-sized contact at `(x, y)` in the kernel frame.
fn finger(id: u8, x: i32, y: i32) -> RawTouch {
    RawTouch {
        id: TouchId(id),
        at: Point::new(x, y),
        major: 30,
        minor: 25,
        size: 12,
        orientation: 0,
        state: TouchState::Touching,
    }
}

/// A hidraw read at `ns` carrying these touches and no buttons.
fn record(ns: u64, touches: Vec<RawTouch>) -> Record {
    let report = Report {
        buttons: Buttons::NONE,
        motion: Motion::default(),
        stamp: Some(Stamp::Ticks(((ns / 1000) % (1 << 18)) as u32)),
        touches,
    };
    Record {
        at: Mono(ns),
        bytes: encode(&report),
    }
}

#[test]
#[ignore]
fn ten_thousand_reports_stay_under_two_ms_at_p95() {
    // Strokes of one and two fingers, 11 ms apart, lifting every 60 reports.
    let records: Vec<Record> = (0..10_000u64)
        .map(|i| {
            let k = (i % 61) as i32;
            let touches = match (k, (i / 61) % 2) {
                (60, _) => vec![],
                (_, 0) => vec![finger(1, 0, -900 + 6 * k)],
                _ => vec![
                    finger(1, -300 + 5 * k, -900 + 2 * k),
                    finger(2, 100 + 5 * k, -900 + 2 * k),
                ],
            };
            record(i * 11_000_000, touches)
        })
        .collect();
    let r = measure(&RecordedSource::MagicMouseHid, &records, Config::default());
    println!("reads {}", r.n);
    println!(
        "socket  p50 {:?} p95 {:?} p99 {:?} max {:?}",
        r.socket.p50, r.socket.p95, r.socket.p99, r.socket.max
    );
    println!(
        "uinput  p50 {:?} p95 {:?} p99 {:?} max {:?}",
        r.uinput.p50, r.uinput.p95, r.uinput.p99, r.uinput.max
    );
    assert_eq!(r.n, 10_000);
    assert!(r.socket.p95.as_micros() <= 2000 && r.uinput.p95.as_micros() <= 2000);
}
