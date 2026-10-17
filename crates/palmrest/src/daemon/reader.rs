//! The reader thread: the sensor's node in, tracker, outputs out (12.3.12). One loop, no async
//! runtime.

use super::devices::{Devices, Health};
use super::discover::Backoff;
use super::dispatch::dispatch;
use super::hidraw::{self, Read};
use super::socket::{FromSocket, SocketHandle, ToSocket};
use super::source::{self, Opened, Purpose};
use crate::action::Desktop;
use crate::config::{self, Loaded};
use crate::recording::Record;
use crate::replay::{Timing, feed_from};
use crate::stamp::StampClock;
use crate::time::Mono;
use crate::tracker::{Out, Tracker};
use std::sync::mpsc::Receiver;
use std::time::Duration;

/// Consecutive failed reads that mean the mouse is gone.
pub const ERROR_THRESHOLD: u32 = 10;
/// The longest the loop sleeps without looking at claims and configuration.
pub const IDLE_POLL: Duration = Duration::from_millis(100);

/// Everything the reader owns.
pub struct Reader {
    pub socket: SocketHandle,
    pub notes: Receiver<FromSocket>,
    pub configs: Receiver<Loaded>,
    pub devices: Devices,
    pub desktop: Desktop,
    pub tracker: Tracker,
    /// The machine's settings text (`/etc/palmrest/gestures.toml`), as last read.
    pub system: String,
    /// The person's settings text, as the shell last sent it; none until it does, and again
    /// when the person leaves the seat.
    pub user: Option<String>,
}

impl Reader {
    /// Finds the configured sensor, reads it until it goes away, and starts over; never
    /// returns. `device.source` is read at each attempt, so a change applies at the next
    /// reconnect.
    pub fn run(mut self) {
        let mut backoff = Backoff::DEVICE;
        loop {
            self = self.housekeeping();
            match source::open(self.tracker.config().device.source, Purpose::Run) {
                Err(e) => eprintln!("palmrest: {e}"),
                Ok(opened) => {
                    backoff = Backoff::DEVICE;
                    self = self.session(&opened);
                }
            }
            let (delay, next) = backoff.step();
            backoff = next;
            eprintln!("palmrest: retrying in {}s", delay.as_secs());
            std::thread::sleep(delay);
        }
    }

    fn session(mut self, opened: &Opened) -> Self {
        let info = opened.info.clone();
        eprintln!("palmrest: connected: {} ({})", info.name, info.path);
        self.socket.send(ToSocket::Device(info));
        let mut sensor = opened.sensor();
        self.tracker = self.tracker.with_surface(sensor.surface());
        let mut clock = StampClock::new();
        let mut errors = 0;
        while errors < ERROR_THRESHOLD {
            self = self.housekeeping();
            let now = hidraw::now();
            let timeout = self
                .tracker
                .deadline()
                .map_or(IDLE_POLL, |d| d.since(now).min(IDLE_POLL));
            match hidraw::read_up_to(&opened.fd, timeout, opened.read_len) {
                Read::Report(bytes) => {
                    errors = 0;
                    let record = Record {
                        at: hidraw::now(),
                        bytes,
                    };
                    let (t, c, step) = feed_from(
                        sensor.as_mut(),
                        self.tracker,
                        clock,
                        &record,
                        Timing::Calibrated,
                    );
                    clock = c;
                    if let Some(e) = step.error {
                        eprintln!("palmrest: dropped a read: {e}");
                    }
                    self = Self { tracker: t, ..self }.emit(step.outs);
                }
                Read::Timeout => self = self.with_tracker(|t| t.tick(hidraw::now())),
                Read::Failed(e) => {
                    errors += 1;
                    eprintln!("palmrest: read failed ({errors}): {e}");
                    std::thread::sleep(IDLE_POLL);
                }
            }
        }
        eprintln!("palmrest: device disconnected");
        let s = self.with_tracker(|t| t.disconnect(hidraw::now()));
        s.socket.send(ToSocket::Gone);
        s
    }

    /// Applies reloaded configuration, claim and lock changes and a due device retry.
    fn housekeeping(mut self) -> Self {
        let now = hidraw::now();
        while let Ok(loaded) = self.configs.try_recv() {
            loaded
                .warnings
                .iter()
                .for_each(|w| eprintln!("palmrest: config: {w}"));
            self.system = loaded.text;
            self = self.with_layers(now);
        }
        while let Ok(note) = self.notes.try_recv() {
            self = match note {
                FromSocket::Pointer(over) => self.with_tracker(|t| t.claim(over)),
                FromSocket::Session(lock) => Self {
                    tracker: self.tracker.with_session_lock(lock),
                    ..self
                },
                FromSocket::UserConfig(text) => {
                    self.user = text;
                    self.with_layers(now)
                }
            };
        }
        self.devices = self.devices.poll(now);
        self
    }

    /// The settings in force: the machine's file with the person's laid over it.
    fn with_layers(mut self, now: Mono) -> Self {
        let (config, warnings) = config::layered(&self.system, self.user.as_deref());
        warnings
            .iter()
            .for_each(|w| eprintln!("palmrest: config: {w}"));
        self.devices = self.devices.reconfigure(&config, now);
        self.tracker = self.tracker.with_config(config);
        self
    }

    fn with_tracker(self, f: impl FnOnce(Tracker) -> (Tracker, Vec<Out>)) -> Self {
        let (tracker, outs) = f(self.tracker);
        Self { tracker, ..self }.emit(outs)
    }

    fn emit(mut self, outs: Vec<Out>) -> Self {
        dispatch(outs, &self.socket, &mut self.devices, self.desktop);
        match self.devices.take_health() {
            Health::Fine => self,
            Health::LostDevices => Self {
                tracker: self.tracker.devices_lost(),
                ..self
            },
        }
    }
}
