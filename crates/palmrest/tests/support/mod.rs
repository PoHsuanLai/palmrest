//! Shared by the integration tests: building strokes as reports, the fixture files, and
//! reading outputs back.
#![allow(dead_code)]

mod case_points;
pub mod scenarios;
mod sessions;
mod strokes;

use palmrest::recording::{self, Record};
use palmrest::sensor::magic_mouse::encode::encode;
use palmrest::sensor::magic_mouse::report::{Buttons, Motion, Report, Stamp};
use palmrest::touch::{Point, RawTouch, TouchId, TouchState};
use palmrest::{Mono, Out, ToHost};
use std::path::PathBuf;

/// A firm, finger-sized contact at `(x, y)` in the kernel frame.
pub fn finger(id: u8, x: i32, y: i32) -> RawTouch {
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

/// A contact given in the Python daemon's unsigned reading, converted to the kernel frame.
pub fn unsigned_finger(id: u8, x: i32, y: i32) -> RawTouch {
    let k = |v: i32| {
        if (v & 0xfff) >= 0x800 {
            (v & 0xfff) - 4096
        } else {
            v & 0xfff
        }
    };
    finger(id, k(x), -k(y))
}

/// Builds a recording frame by frame.
#[derive(Debug, Clone, Default)]
pub struct Script {
    pub records: Vec<Record>,
}

impl Script {
    /// A report at `ms` with these touches and no buttons.
    pub fn at(self, ms: f64, touches: Vec<RawTouch>) -> Self {
        self.report(ms, touches, Buttons::NONE, Motion::default())
    }

    /// A report at `ms` with everything specified.
    pub fn report(
        mut self,
        ms: f64,
        touches: Vec<RawTouch>,
        buttons: Buttons,
        motion: Motion,
    ) -> Self {
        let ns = (ms * 1e6).round() as u64;
        let report = Report {
            buttons,
            motion,
            stamp: Stamp::Ticks(((ns / 1000) % (1 << 18)) as u32),
            touches,
        };
        self.records.push(Record {
            at: Mono(ns),
            bytes: encode(&report),
        });
        self
    }

    /// Raw bytes as read, at `ms`.
    pub fn raw(mut self, ms: f64, bytes: Vec<u8>) -> Self {
        self.records.push(Record {
            at: Mono((ms * 1e6).round() as u64),
            bytes,
        });
        self
    }

    /// The empty report the mouse sends when the last finger lifts.
    pub fn lift(self, ms: f64) -> Self {
        self.at(ms, vec![])
    }

    /// `n + 1` reports from `start` every `step` ms, touches from `f(i)`.
    pub fn frames(
        self,
        start: f64,
        step: f64,
        n: usize,
        f: impl Fn(usize) -> Vec<RawTouch>,
    ) -> Self {
        (0..=n).fold(self, |s, i| s.at(start + step * i as f64, f(i)))
    }

    /// The time of the last record, in ms.
    pub fn end_ms(&self) -> f64 {
        self.records.last().map_or(0.0, |r| r.at.0 as f64 / 1e6)
    }
}

/// `tests/fixtures/<name>.bin`.
pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.bin"))
}

/// A committed fixture, parsed.
pub fn load(name: &str) -> Vec<Record> {
    let bytes = std::fs::read(fixture_path(name)).unwrap_or_else(|e| {
        panic!("fixture {name}: {e}; run PALMREST_REGEN_FIXTURES=1 cargo test")
    });
    recording::parse(&bytes).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// Host messages among the outputs.
pub fn host(outs: &[Out]) -> Vec<ToHost> {
    outs.iter()
        .filter_map(|o| {
            if let Out::Host(h) = o {
                Some(h.clone())
            } else {
                None
            }
        })
        .collect()
}

/// `(phase, fingers, axis, dx, dy)` of every Scroll message.
pub fn scrolls(
    outs: &[Out],
) -> Vec<(
    palmrest::protocol::Phase,
    u8,
    Option<palmrest::protocol::Axis>,
    f32,
    f32,
)> {
    host(outs)
        .into_iter()
        .filter_map(|h| match h {
            ToHost::Scroll {
                phase,
                fingers,
                axis,
                dx_px,
                dy_px,
                ..
            } => Some((phase, fingers, axis, dx_px, dy_px)),
            _ => None,
        })
        .collect()
}

/// Total scrolled px (finger direction) over the Changed messages.
pub fn scrolled(outs: &[Out]) -> (f64, f64) {
    scrolls(outs).iter().fold((0.0, 0.0), |(x, y), s| {
        (x + f64::from(s.3), y + f64::from(s.4))
    })
}
