//! The devices made by palmrest itself, through the uinput node. This is the way for a machine
//! that has no input service and for development (`--direct-uinput`); a desktop install closes
//! the node to everyone but the service, and this path then fails at once.
//!
//! It speaks the same `Session` as the service path, so the emitter above it is one piece of
//! code. The tests never reach it: they use the fake source.

use super::caps::{Caps, Identity, wanted};
use crate::daemon::sys::{self, AbsInfo, BitSet, InputId, UinputAbsSetup, UinputSetup};
use crate::outputs::events::{EV_ABS, EV_KEY, EV_REL, EV_SYN};
use rustix::fd::OwnedFd;
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;
use std::io;
use turnstile_client::{
    ClientError, Create, DeviceId, DeviceSource, Event, Made, Reason, Sent, Session,
};

/// The uinput control node.
pub const UINPUT: &str = "/dev/uinput";
/// `BUS_VIRTUAL`.
const BUS_VIRTUAL: u16 = 0x06;

/// A created uinput device; destroyed on drop.
#[derive(Debug)]
struct Device {
    fd: OwnedFd,
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = sys::dev_destroy(&self.fd);
    }
}

impl Device {
    /// Writes the events at once. A full queue loses them, as the service path does.
    fn write(&self, events: &[Event]) -> Result<Sent, ClientError> {
        let bytes: Vec<u8> = events.iter().flat_map(|e| e.to_bytes()).collect();
        match rustix::io::write(&self.fd, &bytes) {
            Ok(n) if n == bytes.len() => Ok(Sent::Queued),
            Ok(_) => Err(ClientError::Io(io::Error::new(
                io::ErrorKind::WriteZero,
                "short uinput write",
            ))),
            Err(Errno::AGAIN) => Ok(Sent::Dropped),
            Err(e) => Err(ClientError::Io(e.into())),
        }
    }
}

fn create(id: Identity, caps: &Caps) -> io::Result<Device> {
    let fd = rustix::fs::open(
        UINPUT,
        OFlags::WRONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let kinds = [
        (EV_KEY, !caps.keys.is_empty()),
        (EV_REL, !caps.rels.is_empty()),
        (EV_ABS, !caps.abs.is_empty()),
    ];
    sys::set_bit(&fd, BitSet::Ev, EV_SYN)?;
    for (kind, used) in kinds {
        if used {
            sys::set_bit(&fd, BitSet::Ev, kind)?;
        }
    }
    for &k in &caps.keys {
        sys::set_bit(&fd, BitSet::Key, k)?;
    }
    for &r in &caps.rels {
        sys::set_bit(&fd, BitSet::Rel, r)?;
    }
    for &(code, _) in &caps.abs {
        sys::set_bit(&fd, BitSet::Abs, code)?;
    }
    let abs: Vec<UinputAbsSetup> = caps
        .abs
        .iter()
        .map(|&(code, range)| UinputAbsSetup {
            code,
            absinfo: AbsInfo {
                maximum: range.max,
                resolution: range.resolution,
                ..AbsInfo::default()
            },
        })
        .collect();
    for &p in &caps.props {
        sys::set_bit(&fd, BitSet::Prop, p)?;
    }
    let mut name = [0u8; 80];
    name[..id.name.len()].copy_from_slice(id.name.as_bytes());
    let setup = UinputSetup {
        id: InputId {
            bustype: BUS_VIRTUAL,
            vendor: id.vendor,
            product: id.product,
            version: 1,
        },
        name,
        ff_effects_max: 0,
    };
    sys::dev_create(&fd, setup, &abs)?;
    Ok(Device { fd })
}

/// The uinput node as a source of sessions.
#[derive(Debug, Clone, Copy)]
pub struct DirectSource;

impl DeviceSource for DirectSource {
    type Session = DirectSession;

    /// Opens and closes the node to find out whether this user may make devices at all.
    fn open(&self) -> Result<DirectSession, ClientError> {
        let node = rustix::fs::open(
            UINPUT,
            OFlags::WRONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| {
            ClientError::Io(io::Error::new(
                io::Error::from(e).kind(),
                format!("{UINPUT}: {e}"),
            ))
        })?;
        drop(node);
        Ok(DirectSession::default())
    }
}

/// The devices one direct run made.
#[derive(Debug, Default)]
pub struct DirectSession {
    devices: Vec<(DeviceId, Device)>,
    next: u8,
}

impl Session for DirectSession {
    fn create(&mut self, request: &Create) -> Result<Made, ClientError> {
        let (identity, caps) = wanted(request).ok_or(ClientError::Refused(Reason::NotAllowed))?;
        let device = create(identity, &caps)?;
        let id = DeviceId(self.next);
        self.next = self.next.wrapping_add(1);
        self.devices.push((id, device));
        Ok(Made::Proxied(id))
    }

    fn send(&mut self, device: DeviceId, events: &[Event]) -> Result<Sent, ClientError> {
        match self.devices.iter().find(|(id, _)| *id == device) {
            Some((_, found)) => found.write(events),
            None => Err(ClientError::Refused(Reason::Unknown)),
        }
    }

    fn destroy(&mut self, device: DeviceId) -> Result<(), ClientError> {
        self.devices.retain(|(id, _)| *id != device);
        Ok(())
    }

    fn refusal(&mut self) -> Option<Reason> {
        None
    }
}
