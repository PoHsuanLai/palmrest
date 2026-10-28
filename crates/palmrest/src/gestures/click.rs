//! P4: a physical click is not a gesture (R7).
//!
//! From the press until `click_guard_ms` after the release, no gesture starts, taps are void,
//! and the running scroll is Cancelled at the press.

use crate::feel::Hold;
use crate::sensor::magic_mouse::report::Buttons;
use crate::time::Mono;
use std::time::Duration;

/// The guard between reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClickGuard {
    #[default]
    Idle,
    Pressed,
    /// Released; guarding until this instant.
    Until(Mono),
}

impl ClickGuard {
    /// Advances by one report's button bits. The flag is whether this report is the one where a
    /// button went down.
    pub fn step(self, buttons: Buttons, now: Mono, guard: Duration) -> (Self, bool) {
        match (self, buttons.any()) {
            (Self::Pressed, true) => (Self::Pressed, false),
            (_, true) => (Self::Pressed, true),
            (Self::Pressed, false) => (Self::Until(now.after(guard)), false),
            (Self::Until(t), false) if now < t => (self, false),
            (_, false) => (Self::Idle, false),
        }
    }

    /// Whether gestures are held off at `now`.
    pub fn hold(self, now: Mono) -> Hold {
        match self {
            Self::Pressed => Hold::Held,
            Self::Until(t) if now < t => Hold::Held,
            _ => Hold::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_sequence() {
        let g = Duration::from_millis(200);
        let steps: &[(u64, Buttons, bool, Hold)] = &[
            (0, Buttons::NONE, false, Hold::Open),
            (10, Buttons::LEFT, true, Hold::Held),
            (20, Buttons::LEFT, false, Hold::Held),
            (30, Buttons::NONE, false, Hold::Held),
            (229, Buttons::NONE, false, Hold::Held),
            (230, Buttons::NONE, false, Hold::Open),
            (240, Buttons::RIGHT, true, Hold::Held),
        ];
        let mut s = ClickGuard::Idle;
        for &(ms, b, press, hold) in steps {
            let now = Mono::from_millis(ms);
            let (n, p) = s.step(b, now, g);
            s = n;
            assert_eq!((p, s.hold(now)), (press, hold), "at {ms} ms");
        }
    }
}
