//! O1: the socket thread (12.3.10). Accepts subscribers, fans messages out, and turns their
//! `PointerOver` reports into the one claim the tracker needs (11.3.14: while any subscriber
//! has the pointer, foreign outputs are silent).
//!
//! The reader thread hands messages over a bounded channel and wakes this thread with an
//! eventfd, so the reader never blocks on a slow subscriber: a subscriber whose queue is full
//! misses that message rather than delaying everyone else's scroll, and a full channel drops
//! the message rather than stalling the reader. Subscribers are known by id, never by their
//! position in the list, and their number is capped.

mod client;
mod limits;
mod server;

use limits::QUEUE_MAX;
use palmrest_protocol::{DeviceInfo, Over, ToHost};
use rustix::event::{EventfdFlags, eventfd};
use rustix::fd::OwnedFd;
use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType};
use server::{Exit, Server};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender, SyncSender, TrySendError};

/// The process exit status when the socket can no longer be served; systemd restarts the unit.
const SOCKET_LOST_STATUS: i32 = 70;

/// What the reader thread tells the socket thread.
#[derive(Debug, Clone, PartialEq)]
pub enum ToSocket {
    /// A mouse was opened: remembered for every later `Hello`, and announced now.
    Device(DeviceInfo),
    /// The mouse went away (after the tracker's `DeviceGone`).
    Gone,
    /// A message for every admitting subscriber.
    Msg(ToHost),
}

/// The reader's end: a channel plus the eventfd that wakes the socket thread.
#[derive(Debug, Clone)]
pub struct SocketHandle {
    tx: SyncSender<ToSocket>,
    wake: Arc<OwnedFd>,
}

impl SocketHandle {
    /// Queues `m` and wakes the socket thread. A full queue (a stuck thread) drops `m`; a dead
    /// socket thread ends the process by itself, so there is nothing to report here.
    pub fn send(&self, m: ToSocket) {
        match self.tx.try_send(m) {
            Ok(()) | Err(TrySendError::Full(_)) => {
                let _ = rustix::io::write(&*self.wake, &1u64.to_ne_bytes());
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

/// The pointer is ours if any subscriber says so.
pub fn aggregate(overs: impl IntoIterator<Item = Over>) -> Over {
    if overs.into_iter().any(|o| o == Over::Ours) {
        Over::Ours
    } else {
        Over::NotOurs
    }
}

/// `$XDG_RUNTIME_DIR/palmrest/events.sock`.
pub fn default_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("palmrest").join("events.sock")
}

/// Binds the listening socket: directory 0700, socket 0600, a stale socket file replaced.
pub fn bind(path: &Path) -> io::Result<OwnedFd> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let fd = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )?;
    rustix::net::bind(&fd, &SocketAddrUnix::new(path)?)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    rustix::net::listen(&fd, 16)?;
    Ok(fd)
}

/// Starts the socket thread on a bound listener. Claims go to `claims`. If the thread can no
/// longer wait for events the process exits, so the unit restarts instead of running on with a
/// dead socket.
pub fn spawn(listener: OwnedFd, claims: Sender<Over>) -> io::Result<SocketHandle> {
    let wake = Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?);
    let (tx, rx) = mpsc::sync_channel(QUEUE_MAX);
    let server = Server::new(listener, wake.clone(), rx, claims);
    std::thread::Builder::new()
        .name("palmrest-socket".to_owned())
        .spawn(move || {
            if server.run() == Exit::PollFailed {
                std::process::exit(SOCKET_LOST_STATUS);
            }
        })?;
    Ok(SocketHandle { tx, wake })
}

#[cfg(test)]
mod tests;
