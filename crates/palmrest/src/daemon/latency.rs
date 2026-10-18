//! Acceptance 11, replay-only: the time from a read's bytes to the bytes written to the socket
//! and to uinput, measured on the daemon's own path (decode, stamp, step, encode) without the
//! device or the kernel. The live figure needs the mouse (FINDINGS).

use crate::config::Config;
use crate::outputs::events::{PadEncoder, key_events, wheel_events};
use crate::recording::{Record, RecordedSource};
use crate::replay::{Timing, feed_from};
use crate::stamp::StampClock;
use crate::tracker::{Out, Tracker};
use palmrest_protocol as protocol;
use std::time::{Duration, Instant};

/// Percentiles of one path's per-read latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Percentiles {
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub max: Duration,
}

/// Latency to the socket bytes and to the uinput bytes, over `n` reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyReport {
    pub n: usize,
    pub socket: Percentiles,
    pub uinput: Percentiles,
}

/// Replays `records` read from `source`, timing each read.
pub fn measure(source: &RecordedSource, records: &[Record], cfg: Config) -> LatencyReport {
    let mut sensor = source.sensor();
    let mut tracker = Tracker::on(cfg, sensor.surface());
    let mut clock = StampClock::new();
    let mut encoder = PadEncoder::default();
    let (mut socket, mut uinput) = (
        Vec::with_capacity(records.len()),
        Vec::with_capacity(records.len()),
    );
    for r in records {
        let start = Instant::now();
        let (t, c, step) = feed_from(&mut sensor, tracker, clock, r, Timing::Calibrated);
        tracker = t;
        clock = c;
        let mut bytes = 0usize;
        for o in &step.outs {
            if let Out::Host(m) = o {
                bytes += protocol::encode(m).len();
            }
        }
        socket.push(start.elapsed());
        for o in &step.outs {
            let events = match o {
                Out::Pad(f) => {
                    let (e, ev) = encoder.encode(f);
                    encoder = e;
                    ev
                }
                Out::Wheel(w) => wheel_events(w),
                Out::Key(k) => key_events(k),
                Out::Host(_) | Out::Deferred(_) => vec![],
            };
            bytes += events.iter().map(|e| e.to_bytes().len()).sum::<usize>();
        }
        uinput.push(start.elapsed());
        std::hint::black_box(bytes);
    }
    LatencyReport {
        n: records.len(),
        socket: percentiles(socket),
        uinput: percentiles(uinput),
    }
}

fn percentiles(mut v: Vec<Duration>) -> Percentiles {
    v.sort();
    let at = |q: f64| {
        v.get(((v.len().max(1) - 1) as f64 * q).round() as usize)
            .copied()
            .unwrap_or_default()
    };
    Percentiles {
        p50: at(0.5),
        p95: at(0.95),
        p99: at(0.99),
        max: v.last().copied().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests;
