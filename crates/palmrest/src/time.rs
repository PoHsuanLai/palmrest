//! Monotonic instants as plain nanoseconds.
//!
//! The pure core never reads a clock: every `step` is handed `now`. A `Mono` is a
//! `CLOCK_MONOTONIC` reading (or device time mapped onto it, see `stamp`), carried as an integer
//! so fixtures are exact and the socket can send it as `t_ns` without conversion.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A point on the monotonic clock, in nanoseconds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Mono(pub u64);

impl Mono {
    /// An instant `ms` milliseconds after the clock's origin; for tests and fixtures.
    pub const fn from_millis(ms: u64) -> Self {
        Self(ms * 1_000_000)
    }

    /// The time from `earlier` to `self`, zero if `earlier` is later (clocks never run backwards,
    /// but a fixture may repeat an instant).
    pub fn since(self, earlier: Mono) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(earlier.0))
    }

    /// The instant `d` after `self`.
    pub fn after(self, d: Duration) -> Mono {
        let ns = u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        Mono(self.0.saturating_add(ns))
    }

    /// Seconds as a float, the unit the scroll feel formulas are written in.
    pub fn seconds_since(self, earlier: Mono) -> f64 {
        self.since(earlier).as_secs_f64()
    }

    /// The instant as float seconds.
    pub fn secs(self) -> f64 {
        self.0 as f64 / 1e9
    }

    /// `self - earlier` as a difference of float seconds. The scroll feel uses this wherever a
    /// result is truncated, because the reference-feel goldens were computed this way and a
    /// differently rounded `dt` moves a truncation by one unit.
    pub fn float_since(self, earlier: Mono) -> f64 {
        self.secs() - earlier.secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_saturates_and_after_adds() {
        let a = Mono::from_millis(10);
        let b = Mono::from_millis(35);
        assert_eq!(b.since(a), Duration::from_millis(25));
        assert_eq!(a.since(b), Duration::ZERO);
        assert_eq!(a.after(Duration::from_millis(25)), b);
        assert!((b.seconds_since(a) - 0.025).abs() < 1e-12);
    }
}
