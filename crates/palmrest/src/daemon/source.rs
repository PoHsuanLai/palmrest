//! Opening the configured sensor (`device.source`): the node, the sensor that decodes it, and
//! what the socket announces about it.

use super::discover::{self, SYS_HIDRAW};
use super::evdev::{self, SYS_INPUT, UDEV_DATA};
use super::hidraw::{self, Access, READ_LEN};
use crate::config::DeviceSource;
use crate::error::Error;
use crate::recording::RecordedSource;
use crate::sensor::magic_mouse::{Layout, Support};
use crate::sensor::{EvdevMultitouch, EvdevSpec, MagicMouseHid, Sensor, evdev::EVENT_LEN};
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

/// Which sensor an opened node needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Hid(MagicMouseHid),
    Evdev(EvdevSpec),
}

/// An opened node.
pub struct Opened {
    pub fd: OwnedFd,
    pub kind: Kind,
    pub info: DeviceInfo,
    /// How many bytes one read may take.
    pub read_len: usize,
}

impl Opened {
    /// A fresh sensor for this node.
    pub fn sensor(&self) -> Box<dyn Sensor> {
        match self.kind {
            Kind::Hid(s) => Box::new(s),
            Kind::Evdev(spec) => Box::new(EvdevMultitouch::new(spec)),
        }
    }

    /// What a recording of this node says it came from.
    pub fn recorded(&self) -> RecordedSource {
        match self.kind {
            Kind::Hid(_) => RecordedSource::MagicMouseHid,
            Kind::Evdev(spec) => RecordedSource::Evdev(spec),
        }
    }
}

/// Finds and opens the configured source; the error is the line to log.
pub fn open(source: DeviceSource, purpose: Purpose) -> Result<Opened, Error> {
    match source {
        DeviceSource::Hidraw => open_hidraw(purpose),
        DeviceSource::Evdev => open_evdev(),
    }
}

fn open_hidraw(purpose: Purpose) -> Result<Opened, Error> {
    let found = discover::find(Path::new(SYS_HIDRAW)).ok_or(Error::NoMagicMouse)?;
    let d = found.device;
    if d.layout == Layout::Report29 {
        return Err(Error::UndecodableReport {
            path: found.info.path,
            model: d.model,
        });
    }
    if d.support == Support::Untested {
        eprintln!(
            "palmrest: {:?} ({:04x}:{:04x}) has never been run with palmrest; \
             if gestures misbehave, try [device] source = \"evdev\"",
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
        kind: Kind::Hid(MagicMouseHid::new(d.model)),
        info: found.info,
        read_len: READ_LEN,
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
        kind: Kind::Evdev(spec),
        info,
        read_len: EVENT_LEN * EVDEV_READ_EVENTS,
    })
}
