//! Acceptance 11, replay-only. Ignored by default because timing depends on the machine:
//! `cargo test --release --test latency -- --ignored --nocapture`.

mod support;

use palmrest::config::Config;
use palmrest::daemon::latency::measure;
use support::{Script, finger};

#[test]
#[ignore]
fn ten_thousand_reports_stay_under_two_ms_at_p95() {
    // Strokes of one and two fingers, 11 ms apart, lifting every 60 reports.
    let mut s = Script::default();
    let mut t = 0.0;
    for i in 0..10_000 {
        let k = i % 61;
        let touches = match (k, (i / 61) % 2) {
            (60, _) => vec![],
            (_, 0) => vec![finger(1, 0, -900 + 6 * k)],
            _ => vec![
                finger(1, -300 + 5 * k, -900 + 2 * k),
                finger(2, 100 + 5 * k, -900 + 2 * k),
            ],
        };
        s = s.at(t, touches);
        t += 11.0;
    }
    let r = measure(&s.records, Config::default());
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
