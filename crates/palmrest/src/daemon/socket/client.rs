//! One subscriber: its descriptor, who it is, what it asked to receive and what it said about the
//! pointer and the lock screen.

use super::limits::PACKETS_PER_PASS;
use super::seat::{Peer, peer_of};
use latchkey::guard::Origin;
use palmrest_protocol::{
    self as protocol, Filter, LockState, Over, ToDaemon, ToHost, USER_CONFIG_MAX_BYTES,
};
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
    /// The uid the kernel vouches for, taken when it connected.
    uid: u32,
    /// What it showed itself to be when it connected.
    origin: Origin,
    filter: Filter,
    over: Over,
    /// What it last said about the session's lock; `None` until it says.
    lock: Option<LockState>,
    /// The person's settings it sent since the server last took them.
    config: Option<String>,
}

impl Client {
    pub(super) fn new(id: ClientId, fd: OwnedFd, uid: u32, origin: Origin) -> Self {
        Self {
            id,
            fd,
            uid,
            origin,
            filter: Filter::All,
            over: Over::NotOurs,
            lock: None,
            config: None,
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

    /// What this subscriber is while `active` has the seat.
    pub(super) fn peer(&self, active: Option<u32>) -> Peer {
        peer_of(self.uid, active, self.origin.claim())
    }

    /// Whether the pointer is over this subscriber's surface, as far as it is believed: only the
    /// shell of the person at the seat, and only while its process is the one held.
    pub(super) fn pointer(&self, active: Option<u32>) -> Over {
        match self.peer(active) {
            Peer::Shell => self.over,
            Peer::Plain | Peer::Observer => Over::NotOurs,
        }
    }

    /// What this subscriber says about the lock, as far as it is believed: anyone served may say
    /// locked; only the shell, while its process is the one held, may say unlocked.
    pub(super) fn session(&self, active: Option<u32>) -> Option<LockState> {
        match (self.lock, self.peer(active)) {
            (Some(LockState::Unlocked), Peer::Plain | Peer::Observer) => None,
            (said, _) => said,
        }
    }

    /// The person's settings the shell sent since the last call, if it sent any (the latest one).
    pub(super) fn take_user_config(&mut self) -> Option<String> {
        self.config.take()
    }

    /// Reads at most `PACKETS_PER_PASS` packets and applies them, as `active`'s seat makes this
    /// subscriber's word count. `Gone` when the subscriber closed or reading failed; what is still
    /// queued is read next pass, as `poll` reports it.
    pub(super) fn pull(&mut self, active: Option<u32>) -> Link {
        let mut buf = [0u8; protocol::MAX_PACKET + 1];
        for _ in 0..PACKETS_PER_PASS {
            match rustix::net::recv(&self.fd, &mut buf, RecvFlags::DONTWAIT) {
                Ok((0, _)) => return Link::Gone,
                Ok((n, _)) => self.apply(&buf[..n], self.peer(active)),
                Err(Errno::AGAIN) => return Link::Alive,
                Err(Errno::INTR) => {}
                Err(_) => return Link::Gone,
            }
        }
        Link::Alive
    }

    /// What a subscriber's message does. Anyone served may subscribe and may say the session is
    /// locked (the safe word); only the shell is believed about the pointer, an unlocked session and the person's settings.
    fn apply(&mut self, packet: &[u8], peer: Peer) {
        match (protocol::decode::<ToDaemon>(packet), peer) {
            (Ok(ToDaemon::Subscribe { filter }), Peer::Shell | Peer::Plain) => {
                self.filter = filter;
            }
            (Ok(ToDaemon::PointerOver { over }), Peer::Shell) => self.over = over,
            (Ok(ToDaemon::SessionLock { state }), Peer::Shell) => self.lock = Some(state),
            (
                Ok(ToDaemon::SessionLock {
                    state: LockState::Locked,
                }),
                Peer::Plain,
            ) => {
                self.lock = Some(LockState::Locked);
            }
            (Ok(ToDaemon::UserConfig { toml }), Peer::Shell)
                if toml.len() <= USER_CONFIG_MAX_BYTES =>
            {
                self.config = Some(toml);
            }
            (Ok(ToDaemon::UserConfig { .. }), Peer::Shell) => {
                eprintln!("palmrest: ignoring settings larger than {USER_CONFIG_MAX_BYTES} bytes");
            }
            (Ok(_), _) => {}
            (Err(e), _) => eprintln!("palmrest: ignoring a malformed subscriber message: {e}"),
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
