//! P5: while the mouse itself is moving, scrolling needs a firmer intent.
//!
//! Gripping the mouse to move it drags fingers across the surface; doubling the lock threshold
//! while the sensor reports motion keeps that from scrolling.

use crate::sensor::magic_mouse::report::Motion;
use crate::time::Mono;
use std::collections::VecDeque;
use std::time::Duration;

/// Recent motion samples within the window.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MotionWindow {
    samples: VecDeque<(Mono, f64)>,
}

impl MotionWindow {
    /// Adds this report's motion and drops samples older than `window`.
    pub fn step(mut self, m: Motion, now: Mono, window: Duration) -> Self {
        let counts = f64::from(m.dx).hypot(f64::from(m.dy));
        if counts > 0.0 {
            self.samples.push_back((now, counts));
        }
        while self
            .samples
            .front()
            .is_some_and(|&(t, _)| now.since(t) > window)
        {
            self.samples.pop_front();
        }
        self
    }

    /// Whether the counts in the window exceed `threshold` counts per second.
    pub fn moving(&self, threshold: f64, window: Duration) -> bool {
        let total: f64 = self.samples.iter().map(|&(_, c)| c).sum();
        let secs = window.as_secs_f64();
        secs > 0.0 && total / secs > threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_over_the_window() {
        let w = Duration::from_millis(100);
        let mut m = MotionWindow::default();
        for i in 0..5 {
            m = m.step(Motion { dx: 3, dy: 4 }, Mono::from_millis(i * 10), w);
        }
        // 25 counts in 100 ms is 250 counts/s.
        assert!(m.moving(200.0, w));
        assert!(!m.moving(300.0, w));
        let m = m.step(Motion::default(), Mono::from_millis(125), w);
        // Samples at 30 and 40 ms remain: 10 counts, 100 counts/s.
        assert!(!m.moving(200.0, w));
    }
}
