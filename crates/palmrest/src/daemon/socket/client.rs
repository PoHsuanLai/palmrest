//! One subscriber: its descriptor, what it asked to receive and what it said about the pointer and the lock screen.

use super::limits::PACKETS_PER_PASS;
use palmrest_protocol::{self as protocol, Filter, LockState, Over, ToDaemon, ToHost};
use rustix::fd::{AsFd, OwnedFd};
use rustix::io::Errno;
use rustix::net::{RecvFlags, SendFlags};

/// A subscriber's identity for as long as it is connected. Positions in the list change when a
/// subscriber leaves, so nothing that outlives one pass over the list holds a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ClientId(pub(super) u64);

/// Whether a subscriber is still connected after something was done to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Link {
    Alive,
    Gone,
}

#[derive(Debug)]
pub(super) struct Client {
    pub(super) id: ClientId,
    fd: OwnedFd,
    filter: Filter,
    pub(super) over: Over,
    pub(super) lock: LockState,
}

impl Client {
    pub(super) fn new(id: ClientId, fd: OwnedFd) -> Self {
        Self {
            id,
            fd,
            filter: Filter::All,
            over: Over::NotOurs,
            lock: LockState::Unlocked,
        }
    }

    pub(super) fn fd(&self) -> &OwnedFd {
        &self.fd
    }

    /// Sends `m` if this subscriber's filter admits it. `Gone` if the subscriber is; a full
    /// queue drops the message only.
    pub(super) fn deliver(&self, m: &ToHost) -> Link {
        if self.filter.admits(m) {
            send(&self.fd, m)
        } else {
            Link::Alive
        }
    }

    /// Sends `m` whatever the filter says (the greeting).
    pub(super) fn greet(&self, m: &ToHost) -> Link {
        send(&self.fd, m)
    }

    /// Reads at most `PACKETS_PER_PASS` packets and applies them. `Gone` when the subscriber
    /// closed or reading failed; what is still queued is read next pass, as `poll` reports it.
    pub(super) fn pull(&mut self) -> Link {
        let mut buf = [0u8; protocol::MAX_PACKET + 1];
        for _ in 0..PACKETS_PER_PASS {
            match rustix::net::recv(&self.fd, &mut buf, RecvFlags::DONTWAIT) {
                Ok((0, _)) => return Link::Gone,
                Ok((n, _)) => self.apply(&buf[..n]),
                Err(Errno::AGAIN) => return Link::Alive,
                Err(Errno::INTR) => {}
                Err(_) => return Link::Gone,
            }
        }
        Link::Alive
    }

    fn apply(&mut self, packet: &[u8]) {
        match protocol::decode::<ToDaemon>(packet) {
            Ok(ToDaemon::PointerOver { over }) => self.over = over,
            Ok(ToDaemon::Subscribe { filter }) => self.filter = filter,
            Ok(ToDaemon::SessionLock { state }) => self.lock = state,
            Err(e) => eprintln!("palmrest: ignoring a malformed subscriber message: {e}"),
        }
    }
}

fn send(fd: &OwnedFd, m: &ToHost) -> Link {
    match rustix::net::send(
        fd.as_fd(),
        &protocol::encode(m),
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    ) {
        Ok(_) | Err(Errno::AGAIN) => Link::Alive,
        Err(_) => Link::Gone,
    }
}
