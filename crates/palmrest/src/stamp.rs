//! Mapping the mouse's 18-bit timestamp onto the monotonic clock (12.3.1, acceptance 12).
//!
//! Arrival times carry Bluetooth jitter; the device's own counter does not, but its unit is not
//! published. So the first 10 s of reports fit `mono = k * ticks + c`, and device time is used
//! only if that fit explains arrival times to within 1 ms at p95. Otherwise, and until the fit
//! exists, the arrival time is the event time.

use crate::sensor::FrameStamp;
use crate::time::Mono;
use std::time::Duration;

/// Ticks per wrap of the 18-bit counter.
pub const TICK_SPAN: u64 = 1 << 18;
/// How long the first fit collects samples.
pub const FIT_WINDOW: Duration = Duration::from_secs(10);
/// How often an accepted fit is redone.
pub const REFIT_EVERY: Duration = Duration::from_secs(60);
/// The p95 residual below which device time is trusted.
pub const RESIDUAL_P95_MAX: Duration = Duration::from_millis(1);
/// A pause longer than this while collecting loses track of wraps, so collection restarts.
pub const COLLECT_GAP_MAX: Duration = Duration::from_secs(1);

/// A fitted line from unwrapped ticks to monotonic nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    /// Nanoseconds per tick.
    pub k: f64,
    pub c: f64,
    /// p95 of |fit - arrival| over the fitted samples.
    pub residual_p95: Duration,
}

/// Which clock events are stamped with.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Still collecting (or the fit was rejected): arrival time.
    Arrival,
    /// The fit passed: device time.
    Device(Fit),
}

/// The calibration state, advanced once per report.
#[derive(Debug, Clone, PartialEq)]
pub struct StampClock {
    source: Source,
    samples: Vec<(u64, u64)>,
    window_start: Option<Mono>,
    last_ticks: Option<u64>,
    last_arrival: Option<Mono>,
    last_out: Mono,
    rejected: Option<Fit>,
}

impl Default for StampClock {
    fn default() -> Self {
        Self::new()
    }
}

impl StampClock {
    pub fn new() -> Self {
        Self {
            source: Source::Arrival,
            samples: Vec::new(),
            window_start: None,
            last_ticks: None,
            last_arrival: None,
            last_out: Mono(0),
            rejected: None,
        }
    }

    /// Where event times currently come from.
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The most recent fit that failed the residual test, for the log.
    pub fn rejected(&self) -> Option<Fit> {
        self.rejected
    }

    /// Takes one frame's stamp and its arrival time; returns the event time. The Magic Mouse's
    /// ticks go through the fit; a kernel stamp is already on the monotonic clock and stands in
    /// for the arrival time; no stamp is the arrival time. Event time never runs backwards.
    pub fn observe(self, stamp: FrameStamp, arrival: Mono) -> (Self, Mono) {
        let raw = match stamp {
            FrameStamp::Ticks(raw) => raw,
            FrameStamp::Kernel(at) => return self.emit(at),
            FrameStamp::Absent => return self.emit(arrival),
        };
        let ticks = self.unwrap(raw, arrival);
        let s = self.record(ticks, arrival).maybe_fit(arrival);
        let t = match &s.source {
            Source::Device(fit) => Mono(predict(fit, ticks).max(0.0) as u64),
            Source::Arrival => arrival,
        };
        s.emit(t)
    }

    fn emit(mut self, t: Mono) -> (Self, Mono) {
        let t = t.max(self.last_out);
        self.last_out = t;
        (self, t)
    }

    /// Unwraps an 18-bit reading: near the fitted prediction when there is a fit, else by
    /// counting wraps between consecutive reports.
    fn unwrap(&self, raw: u32, arrival: Mono) -> u64 {
        let raw = u64::from(raw) % TICK_SPAN;
        match (&self.source, self.last_ticks) {
            (Source::Device(fit), _) => {
                nearest(raw, ((arrival.0 as f64 - fit.c) / fit.k).max(0.0) as u64)
            }
            (Source::Arrival, Some(last)) => {
                let base = last - last % TICK_SPAN;
                if raw >= last % TICK_SPAN {
                    base + raw
                } else {
                    base + TICK_SPAN + raw
                }
            }
            (Source::Arrival, None) => raw,
        }
    }

    fn record(mut self, ticks: u64, arrival: Mono) -> Self {
        let gap = self
            .last_arrival
            .map(|l| arrival.since(l))
            .unwrap_or_default();
        if matches!(self.source, Source::Arrival) && gap > COLLECT_GAP_MAX {
            self.samples.clear();
            self.window_start = None;
        }
        self.window_start.get_or_insert(arrival);
        self.samples.push((ticks, arrival.0));
        self.last_ticks = Some(ticks);
        self.last_arrival = Some(arrival);
        self
    }

    fn maybe_fit(mut self, now: Mono) -> Self {
        let Some(start) = self.window_start else {
            return self;
        };
        let due = match self.source {
            Source::Arrival => FIT_WINDOW,
            Source::Device(_) => REFIT_EVERY,
        };
        if now.since(start) < due {
            return self;
        }
        let keep_from = now
            .0
            .saturating_sub(u64::try_from(FIT_WINDOW.as_nanos()).unwrap_or(u64::MAX));
        self.samples.retain(|&(_, a)| a >= keep_from);
        match fit(&self.samples) {
            Some(f) if f.residual_p95 < RESIDUAL_P95_MAX => self.source = Source::Device(f),
            other => {
                self.rejected = other;
                self.source = Source::Arrival;
            }
        }
        self.window_start = Some(now);
        self
    }
}

fn predict(fit: &Fit, ticks: u64) -> f64 {
    fit.k * ticks as f64 + fit.c
}

/// The unwrapped value congruent to `raw` that is nearest `expected`.
fn nearest(raw: u64, expected: u64) -> u64 {
    let base = expected - expected % TICK_SPAN;
    [
        base.checked_sub(TICK_SPAN),
        Some(base),
        Some(base + TICK_SPAN),
    ]
    .into_iter()
    .flatten()
    .map(|b| b + raw)
    .min_by_key(|v| v.abs_diff(expected))
    .unwrap_or(raw) // the array always has at least one element
}

/// Least-squares line through (ticks, arrival ns), with the p95 residual; `None` when the
/// samples cannot determine a slope.
pub fn fit(samples: &[(u64, u64)]) -> Option<Fit> {
    if samples.len() < 2 {
        return None;
    }
    let n = samples.len() as f64;
    let (t0, a0) = samples[0];
    let xs: Vec<f64> = samples.iter().map(|&(t, _)| t as f64 - t0 as f64).collect();
    let ys: Vec<f64> = samples.iter().map(|&(_, a)| a as f64 - a0 as f64).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let k = sxy / sxx;
    let c_local = my - k * mx;
    let mut residuals: Vec<f64> = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| (k * x + c_local - y).abs())
        .collect();
    residuals.sort_by(f64::total_cmp);
    let p95 = residuals[((residuals.len() - 1) as f64 * 0.95).round() as usize];
    Some(Fit {
        k,
        c: a0 as f64 + c_local - k * t0 as f64,
        residual_p95: Duration::from_nanos(p95 as u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD_NS: u64 = 11_111_111; // a 90 Hz report rate
    const TICK_NS: u64 = 1_000; // a hypothetical 1 µs tick

    fn run(jitter: impl Fn(u64) -> u64, reports: u64) -> (StampClock, Vec<Mono>) {
        let mut clock = StampClock::new();
        let mut out = Vec::new();
        for i in 0..reports {
            let true_ns = 5_000_000_000 + i * PERIOD_NS;
            let ticks = ((true_ns / TICK_NS) % TICK_SPAN) as u32;
            let (c, t) = clock.observe(FrameStamp::Ticks(ticks), Mono(true_ns + jitter(i)));
            clock = c;
            out.push(t);
        }
        (clock, out)
    }

    #[test]
    fn steady_ticks_are_trusted_after_ten_seconds() {
        let (clock, out) = run(|i| (i * 7919 % 5) * 100_000, 2000);
        let Source::Device(fit) = clock.source() else {
            panic!("fit rejected: {:?}", clock.rejected())
        };
        assert!((fit.k - TICK_NS as f64).abs() < 1.0, "k = {}", fit.k);
        // Device time follows the jitter-free clock through many 18-bit wraps.
        let last = *out.last().unwrap();
        let truth = 5_000_000_000 + 1999 * PERIOD_NS;
        assert!(last.0.abs_diff(truth) < 1_000_000, "{} vs {truth}", last.0);
    }

    #[test]
    fn noisy_arrival_rejects_the_fit() {
        let (clock, out) = run(|i| (i * 7919 % 11) * 1_000_000, 1200);
        assert_eq!(clock.source(), &Source::Arrival);
        assert!(clock.rejected().is_some());
        assert!(
            out.windows(2).all(|w| w[0] <= w[1]),
            "event time never runs backwards"
        );
    }

    #[test]
    fn absent_stamp_uses_arrival() {
        let (_, t) = StampClock::new().observe(FrameStamp::Absent, Mono(42));
        assert_eq!(t, Mono(42));
    }

    #[test]
    fn kernel_stamps_are_event_times_and_never_run_backwards() {
        let c = StampClock::new();
        let (c, t) = c.observe(FrameStamp::Kernel(Mono(50)), Mono(90));
        assert_eq!(t, Mono(50));
        let (_, t) = c.observe(FrameStamp::Kernel(Mono(40)), Mono(95));
        assert_eq!(t, Mono(50));
    }

    #[test]
    fn nearest_picks_the_right_wrap() {
        assert_eq!(nearest(5, TICK_SPAN * 3 + 2), TICK_SPAN * 3 + 5);
        assert_eq!(nearest(TICK_SPAN - 2, TICK_SPAN * 3 + 1), TICK_SPAN * 3 - 2);
        assert_eq!(nearest(1, 0), 1);
    }
}
