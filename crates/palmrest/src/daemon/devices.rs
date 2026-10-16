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
    /// A write failed (the device was destroyed, the module reloaded): the next `poll` starts
    /// the wait for a rebuilt one.
    Lost,
    Waiting {
        retry_at: Mono,
        backoff: Backoff,
    },
}

/// Whether the devices were lost since the reader last asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Fine,
    /// A write failed. What the tracker believes is down on the virtual devices is not.
    LostDevices,
}

/// The emitter, or the wait for one.
pub struct Devices {
    factory: Factory,
    shape: Shape,
    state: State,
    health: Health,
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
            health: Health::Fine,
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

    /// Whether a write failed since the last call; asking resets it.
    pub fn take_health(&mut self) -> Health {
        std::mem::replace(&mut self.health, Health::Fine)
    }

    /// Starts the wait after a failed write, and tries again if a retry is due. A rebuilt
    /// device is not tried before the first backoff delay, so a device that is created but
    /// cannot be written is rebuilt once a second at most.
    pub fn poll(mut self, now: Mono) -> Self {
        if let State::Lost = self.state {
            let (delay, backoff) = Backoff::UINPUT.step();
            self.state = State::Waiting {
                retry_at: now.after(delay),
                backoff,
            };
            eprintln!(
                "palmrest: virtual input lost; rebuilding in {}s",
                delay.as_secs()
            );
        }
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

impl Devices {
    /// Runs `write` on the ready emitter. A failure drops the emitter (which destroys its
    /// devices) and is returned once; frames meanwhile are dropped without a message.
    fn write(
        &mut self,
        write: impl FnOnce(&mut dyn Emitter) -> Result<(), OutputError>,
    ) -> Result<(), OutputError> {
        let State::Ready(emitter) = &mut self.state else {
            return Ok(());
        };
        let result = write(emitter.as_mut());
        if result.is_err() {
            self.state = State::Lost;
            self.health = Health::LostDevices;
        }
        result
    }
}

impl Emitter for Devices {
    fn pad(&mut self, f: &PadFrame) -> Result<(), OutputError> {
        self.write(|e| e.pad(f))
    }

    fn wheel(&mut self, d: &WheelDelta) -> Result<(), OutputError> {
        self.write(|e| e.wheel(d))
    }

    fn key(&mut self, k: &KeyCombo) -> Result<(), OutputError> {
        self.write(|e| e.key(k))
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

    /// An emitter whose every write fails, as a destroyed uinput device does.
    struct Destroyed;

    impl Emitter for Destroyed {
        fn pad(&mut self, _: &PadFrame) -> Result<(), OutputError> {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound).into())
        }
        fn wheel(&mut self, _: &WheelDelta) -> Result<(), OutputError> {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound).into())
        }
        fn key(&mut self, _: &KeyCombo) -> Result<(), OutputError> {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound).into())
        }
    }

    #[test]
    fn a_failed_write_rebuilds_the_devices_and_reports_the_loss_once() {
        let built = Rc::new(Cell::new(0));
        let b = built.clone();
        let factory: Factory = Box::new(move |_, _| {
            b.set(b.get() + 1);
            if b.get() == 1 {
                Ok(Box::new(Destroyed))
            } else {
                Ok(Box::new(RecordingEmitter::default()))
            }
        });
        let mut d = Devices::new(factory, &Config::default(), Mono(0));
        assert_eq!(built.get(), 1);
        assert_eq!(d.take_health(), Health::Fine);

        assert!(
            d.wheel(&WheelDelta::default()).is_err(),
            "the failure is returned once"
        );
        assert_eq!(d.take_health(), Health::LostDevices);
        assert_eq!(d.take_health(), Health::Fine, "asking resets it");
        assert!(
            d.wheel(&WheelDelta::default()).is_ok(),
            "frames while rebuilding are dropped without another error"
        );

        let d = d.poll(Mono::from_millis(10));
        assert_eq!(built.get(), 1, "not before the backoff");
        let mut d = d.poll(Mono::from_millis(1010));
        assert_eq!(built.get(), 2, "rebuilt once the delay has passed");
        assert!(d.wheel(&WheelDelta::default()).is_ok());
        assert_eq!(d.take_health(), Health::Fine);
    }
}
