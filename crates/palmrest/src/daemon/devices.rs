//! The virtual devices of a daemon run, created lazily and retried while they are not ready.
//!
//! At boot the input service can arrive after the user session starts. The reader keeps running
//! meanwhile, so the socket stream to our own surfaces never waits on the virtual devices.
//!
//! When the service cuts palmrest off (closes the connection, refuses it, stops answering), the
//! devices are rebuilt after a pause, a few times; a service that keeps cutting us off ends the
//! virtual devices until the settings change, and the log says why.

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

/// Times the service cut palmrest off since a frame last went through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Cuts(u8);

impl Cuts {
    /// Rebuilds tried after a cut before the devices are given up: a service restarting or
    /// a daemon reloaded answers within a few seconds, and one that does not is not coming back.
    const RECONNECT_TRIES: u8 = 3;

    fn one_more(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// The first cut is the loss itself; each rebuild that fails or is cut again is another.
    fn spent(self) -> bool {
        self.0 > Self::RECONNECT_TRIES
    }
}

enum State {
    Ready {
        emitter: Box<dyn Emitter>,
        cuts: Cuts,
    },
    /// A write failed (the device was destroyed, the module reloaded): the next `poll` starts
    /// the wait for a rebuilt one.
    Lost { cuts: Cuts },
    Waiting {
        retry_at: Mono,
        backoff: Backoff,
        cuts: Cuts,
    },
    /// The service kept cutting palmrest off: frames are dropped until the shape changes.
    Dead,
}

impl State {
    /// The wait before the first rebuild.
    fn waiting(retry_at: Mono, cuts: Cuts) -> Self {
        Self::Waiting {
            retry_at,
            backoff: Backoff::UINPUT,
            cuts,
        }
    }
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
        Self {
            factory,
            shape: Shape::of(c),
            state: State::waiting(now, Cuts::default()),
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
        self.state = State::waiting(now, Cuts::default());
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
        if let State::Lost { cuts } = self.state {
            let (delay, backoff) = Backoff::UINPUT.step();
            self.state = State::Waiting {
                retry_at: now.after(delay),
                backoff,
                cuts,
            };
            eprintln!(
                "palmrest: virtual input lost; rebuilding in {}s",
                delay.as_secs()
            );
        }
        if let State::Waiting {
            retry_at,
            backoff,
            cuts,
        } = self.state
        {
            if now < retry_at {
                return self;
            }
            self.state = match (self.factory)(self.shape.foreign, &self.shape.combos) {
                Ok(emitter) => State::Ready { emitter, cuts },
                Err(err) => failed_build(&err, now, backoff, cuts),
            };
        }
        self
    }
}

/// The state after a rebuild failed: wait and try again, unless the service has cut palmrest
/// off too often.
fn failed_build(err: &OutputError, now: Mono, backoff: Backoff, cuts: Cuts) -> State {
    let cuts = match err {
        OutputError::Cut(_) => cuts.one_more(),
        _ => cuts,
    };
    if cuts.spent() {
        return gave_up(err);
    }
    let (delay, backoff) = backoff.step();
    eprintln!(
        "palmrest: virtual input not ready: {err}; retrying in {}s",
        delay.as_secs()
    );
    State::Waiting {
        retry_at: now.after(delay),
        backoff,
        cuts,
    }
}

fn gave_up(err: &OutputError) -> State {
    eprintln!(
        "palmrest: virtual input stopped: {err}. It starts again after a change of settings or \
         a restart of palmrest."
    );
    State::Dead
}

impl Devices {
    /// Runs `write` on the ready emitter. A failure drops the emitter (which destroys its
    /// devices) and is returned once; frames meanwhile are dropped without a message.
    fn write(
        &mut self,
        write: impl FnOnce(&mut dyn Emitter) -> Result<(), OutputError>,
    ) -> Result<(), OutputError> {
        let State::Ready { emitter, cuts } = &mut self.state else {
            return Ok(());
        };
        let result = write(emitter.as_mut());
        match &result {
            Ok(()) => *cuts = Cuts::default(),
            Err(err) => {
                let cuts = match err {
                    OutputError::Cut(_) => cuts.one_more(),
                    _ => *cuts,
                };
                self.state = if cuts.spent() {
                    gave_up(err)
                } else {
                    State::Lost { cuts }
                };
                self.health = Health::LostDevices;
            }
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
mod tests;
