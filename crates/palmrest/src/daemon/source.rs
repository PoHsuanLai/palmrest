//! Opening the sensor: the Magic Mouse's hidraw node, or its evdev node when palmrest cannot
//! decode the mouse's report; the sensor that decodes it, and what the socket announces about it.

use super::discover::{self, SYS_HIDRAW};
use super::evdev::{self, SYS_INPUT, UDEV_DATA};
use super::hidraw::{self, Access, READ_LEN};
use crate::error::Error;
use crate::recording::RecordedSource;
use crate::sensor::evdev::EVENT_LEN;
use palmrest_protocol::DeviceInfo;
use rustix::fd::OwnedFd;
use std::path::Path;

/// The most evdev events one read takes.
pub const EVDEV_READ_EVENTS: usize = 64;

/// Why the node is being opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// The daemon: hidraw read-write, so multitouch can be enabled.
    Run,
    /// `palmrest record`: read-only, nothing is sent to the mouse.
    Record,
}

/// An opened node.
pub struct Opened {
    pub fd: OwnedFd,
    /// Which sensor the node needs, and what a recording of it says it came from.
    pub source: RecordedSource,
    pub info: DeviceInfo,
}

impl Opened {
    /// How many bytes one read may take.
    pub fn read_len(&self) -> usize {
        match self.source {
            RecordedSource::MagicMouseHid => READ_LEN,
            RecordedSource::Evdev(_) => EVENT_LEN * EVDEV_READ_EVENTS,
        }
    }
}

/// Finds and opens the mouse: its hidraw node, or its evdev node through the kernel's multitouch
/// driver when palmrest does not decode its report. The error is the line to log.
pub fn open(purpose: Purpose) -> Result<Opened, Error> {
    match open_hidraw(purpose) {
        Err(undecodable @ Error::UndecodableReport { .. }) => {
            eprintln!("palmrest: {undecodable}; reading it through evdev instead");
            open_evdev()
        }
        other => other,
    }
}

fn open_hidraw(purpose: Purpose) -> Result<Opened, Error> {
    let found = discover::find(Path::new(SYS_HIDRAW)).ok_or(Error::NoMagicMouse)?;
    let d = found.device;
    if !d.model.decodable() {
        return Err(Error::UndecodableReport {
            path: found.info.path,
            model: d.model,
        });
    }
    if !d.verified {
        eprintln!(
            "palmrest: {:?} ({:04x}:{:04x}) has never been run with palmrest",
            d.model, d.vendor, d.product
        );
    }
    let want = match purpose {
        Purpose::Run => Access::ReadWrite,
        Purpose::Record => Access::ReadOnly,
    };
    let (fd, access) = hidraw::open(&found.node, want).map_err(|source| Error::HidrawOpen {
        node: found.node.clone(),
        source,
    })?;
    match (purpose, access) {
        (Purpose::Run, Access::ReadWrite) => {
            if let Err(e) = hidraw::enable_multitouch(&fd) {
                eprintln!("palmrest: enabling multitouch failed: {e}");
            }
        }
        (Purpose::Run, Access::ReadOnly) => eprintln!(
            "palmrest: {} is read-only; multitouch left as the kernel set it",
            found.info.path
        ),
        (Purpose::Record, _) => {}
    }
    Ok(Opened {
        fd,
        source: RecordedSource::MagicMouseHid,
        info: found.info,
    })
}

fn open_evdev() -> Result<Opened, Error> {
    let (node, info) =
        evdev::find(Path::new(SYS_INPUT), Path::new(UDEV_DATA)).ok_or(Error::NoEvdevNode)?;
    let (fd, spec) = evdev::open(&node).map_err(|source| Error::EvdevOpen {
        node: node.clone(),
        source,
    })?;
    eprintln!(
        "palmrest: reading {} through evdev (untested beyond the Magic Mouse)",
        info.path
    );
    Ok(Opened {
        fd,
        source: RecordedSource::Evdev(spec),
        info,
    })
}
