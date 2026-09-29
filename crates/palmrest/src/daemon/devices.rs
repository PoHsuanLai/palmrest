//! The virtual devices of a daemon run, created lazily and retried while uinput is not ready.
//!
//! At boot the uinput ACL can arrive after the user session starts. The reader keeps running
//! meanwhile, so the socket
//! stream to our own surfaces never waits on the virtual devices.

use super::discover::Backoff;
use crate::config::{Config, ForeignOutput};
use crate::feel::WheelDelta;
use crate::keys::KeyCombo;
use crate::outputs::pad::PadFrame;
use crate::outputs::{Emitter, OutputError};
use crate::time::Mono;

/// Makes the emitter for a configuration (the uinput one in the daemon).
pub type Factory = Box<dyn Fn(ForeignOutput, &[KeyCombo]) -> Result<Box<dyn Emitter>, OutputError>>;

/// What the devices are made from; a change recreates them.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shape {
    foreign: ForeignOutput,
    combos: Vec<KeyCombo>,
}

impl Shape {
    fn of(c: &Config) -> Self {
        Self {
            foreign: c.foreign_output,
            combos: c.gestures.gesture_action_map.combos(),
        }
    }
}

enum State {
    Ready(Box<dyn Emitter>),
    Waiting { retry_at: Mono, backoff: Backoff },
}

/// The emitter, or the wait for one.
pub struct Devices {
    factory: Factory,
    shape: Shape,
    state: State,
}

impl Devices {
    pub fn new(factory: Factory, c: &Config, now: Mono) -> Self {
        let waiting = State::Waiting {
            retry_at: now,
            backoff: Backoff::UINPUT,
        };
        Self {
            factory,
            shape: Shape::of(c),
            state: waiting,
        }
        .poll(now)
    }

    /// Recreates the devices if the configuration changed what they must be.
    pub fn reconfigure(mut self, c: &Config, now: Mono) -> Self {
        let shape = Shape::of(c);
        if shape == self.shape {
            return self;
        }
        self.shape = shape;
        self.state = State::Waiting {
            retry_at: now,
            backoff: Backoff::UINPUT,
        };
        self.poll(now)
    }

    /// Tries again if a retry is due.
    pub fn poll(mut self, now: Mono) -> Self {
        if let State::Waiting { retry_at, backoff } = self.state {
            if now < retry_at {
                return self;
            }
            self.state = match (self.factory)(self.shape.foreign, &self.shape.combos) {
                Ok(e) => State::Ready(e),
                Err(err) => {
                    let (delay, backoff) = backoff.step();
                    eprintln!(
                        "palmrest: virtual input not ready: {err}; retrying in {}s",
                        delay.as_secs()
                    );
                    State::Waiting {
                        retry_at: now.after(delay),
                        backoff,
                    }
                }
            };
        }
        self
    }
}

impl Emitter for Devices {
    fn pad(&mut self, f: &PadFrame) -> Result<(), OutputError> {
        match &mut self.state {
            State::Ready(e) => e.pad(f),
            State::Waiting { .. } => Ok(()),
        }
    }

    fn wheel(&mut self, d: &WheelDelta) -> Result<(), OutputError> {
        match &mut self.state {
            State::Ready(e) => e.wheel(d),
            State::Waiting { .. } => Ok(()),
        }
    }

    fn key(&mut self, k: &KeyCombo) -> Result<(), OutputError> {
        match &mut self.state {
            State::Ready(e) => e.key(k),
            State::Waiting { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outputs::RecordingEmitter;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn waits_then_becomes_ready_and_rebuilds_on_a_shape_change() {
        let calls = Rc::new(Cell::new(0));
        let c2 = calls.clone();
        let factory: Factory = Box::new(move |_, _| {
            c2.set(c2.get() + 1);
            if c2.get() < 2 {
                Err(OutputError::Unavailable("no acl yet".into()))
            } else {
                Ok(Box::new(RecordingEmitter::default()))
            }
        });
        let cfg = Config::default();
        let d = Devices::new(factory, &cfg, Mono(0));
        assert_eq!(calls.get(), 1);
        let mut d = d.poll(Mono::from_millis(500));
        assert_eq!(calls.get(), 1, "not before the backoff");
        assert!(
            d.wheel(&WheelDelta::default()).is_ok(),
            "frames are dropped while waiting"
        );
        let d = d.poll(Mono::from_millis(1000));
        assert_eq!(calls.get(), 2);
        let d = d.reconfigure(&cfg, Mono::from_millis(1100));
        assert_eq!(calls.get(), 2, "same shape, same devices");
        let wheel = Config {
            foreign_output: ForeignOutput::Wheel,
            ..Config::default()
        };
        let _ = d.reconfigure(&wheel, Mono::from_millis(1200));
        assert_eq!(calls.get(), 3);
    }
}
