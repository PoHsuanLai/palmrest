//! The reader thread: hidraw in, tracker, outputs out (12.3.12). One loop, no async runtime.

use super::devices::Devices;
use super::discover::{self, Backoff, SYS_HIDRAW};
use super::dispatch::dispatch;
use super::hidraw::{self, Access, Read};
use super::socket::{SocketHandle, ToSocket};
use crate::action::Desktop;
use crate::config::Loaded;
use crate::protocol::{DeviceInfo, Over};
use crate::recording::Record;
use crate::replay::{Timing, feed};
use crate::stamp::StampClock;
use crate::tracker::{Out, Tracker};
use rustix::fd::OwnedFd;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::Duration;

/// Consecutive failed reads that mean the mouse is gone (as the Python daemon).
pub const ERROR_THRESHOLD: u32 = 10;
/// The longest the loop sleeps without looking at claims and configuration.
pub const IDLE_POLL: Duration = Duration::from_millis(100);

/// Everything the reader owns.
pub struct Reader {
    pub socket: SocketHandle,
    pub claims: Receiver<Over>,
    pub configs: Receiver<Loaded>,
    pub devices: Devices,
    pub desktop: Desktop,
    pub tracker: Tracker,
}

impl Reader {
    /// Finds the mouse, reads it until it goes away, and starts over; never returns.
    pub fn run(mut self) {
        let mut backoff = Backoff::DEVICE;
        loop {
            self = self.housekeeping();
            match discover::find(Path::new(SYS_HIDRAW)) {
                None => eprintln!("palmrest: Magic Mouse not found"),
                Some((path, info)) => match hidraw::open(&path, Access::ReadWrite) {
                    Err(e) => eprintln!(
                        "palmrest: cannot read {}: {e}; no gestures until it can be read",
                        path.display()
                    ),
                    Ok((fd, access)) => {
                        backoff = Backoff::DEVICE;
                        self = self.session(&fd, access, info);
                    }
                },
            }
            let (delay, next) = backoff.step();
            backoff = next;
            eprintln!("palmrest: retrying in {}s", delay.as_secs());
            std::thread::sleep(delay);
        }
    }

    fn session(mut self, fd: &OwnedFd, access: Access, info: DeviceInfo) -> Self {
        match access {
            Access::ReadWrite => {
                if let Err(e) = hidraw::enable_multitouch(fd) {
                    eprintln!("palmrest: enabling multitouch failed: {e}");
                }
            }
            Access::ReadOnly => eprintln!(
                "palmrest: {} is read-only; multitouch left as the kernel set it",
                info.path
            ),
        }
        eprintln!("palmrest: connected: {} ({})", info.name, info.path);
        self.socket.send(ToSocket::Device(info));
        let mut clock = StampClock::new();
        let mut errors = 0;
        while errors < ERROR_THRESHOLD {
            self = self.housekeeping();
            let now = hidraw::now();
            let timeout = self
                .tracker
                .deadline()
                .map_or(IDLE_POLL, |d| d.since(now).min(IDLE_POLL));
            match hidraw::read(fd, timeout) {
                Read::Report(bytes) => {
                    errors = 0;
                    let record = Record {
                        at: hidraw::now(),
                        bytes,
                    };
                    let (t, c, step) = feed(self.tracker, clock, &record, Timing::Calibrated);
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

    /// Applies reloaded configuration, claim changes and a due device retry.
    fn housekeeping(mut self) -> Self {
        let now = hidraw::now();
        while let Ok(loaded) = self.configs.try_recv() {
            loaded
                .warnings
                .iter()
                .for_each(|w| eprintln!("palmrest: config: {w}"));
            self.devices = self.devices.reconfigure(&loaded.config, now);
            self.tracker = self.tracker.with_config(loaded.config);
        }
        while let Ok(over) = self.claims.try_recv() {
            self = self.with_tracker(|t| t.claim(over));
        }
        self.devices = self.devices.poll(now);
        self
    }

    fn with_tracker(self, f: impl FnOnce(Tracker) -> (Tracker, Vec<Out>)) -> Self {
        let (tracker, outs) = f(self.tracker);
        Self { tracker, ..self }.emit(outs)
    }

    fn emit(mut self, outs: Vec<Out>) -> Self {
        dispatch(outs, &self.socket, &mut self.devices, self.desktop);
        self
    }
}
