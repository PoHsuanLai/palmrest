//! The socket thread's loop: one pass polls the wake-up, the listener and every subscriber,
//! then accepts, delivers what the reader queued, reads what subscribers said and publishes their joint state. Every step
//! is bounded, so no subscriber can keep the others, or the reader's stream, waiting.

use super::client::{Client, ClientId, Link};
use super::limits::{
    ACCEPTS_PER_PASS, AcceptFailure, Complaints, Intake, MAX_CLIENTS, SEAT_RECHECK, Voice,
    accept_failure,
};
use super::seat::{Peer, SeatFile, peer_of};
use super::shell::Vouch;
use super::{FromSocket, ToSocket, aggregate, aggregate_lock};
use palmrest_protocol::{DeviceInfo, LockState, Over, PROTOCOL_VERSION, ToHost};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fd::OwnedFd;
use rustix::io::Errno;
use rustix::net::SocketFlags;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

/// Why the loop ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Exit {
    /// The reader dropped its end of the channel: the daemon is shutting down.
    ReaderGone,
    /// Waiting for events failed for good: the socket can no longer be served.
    PollFailed,
}

/// What a pass leaves to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Flow {
    Continue,
    Stop(Exit),
}

/// Whether poll found a descriptor ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Readiness {
    Quiet,
    Readable,
}

impl Readiness {
    fn of(fd: &PollFd<'_>) -> Self {
        if fd.revents().is_empty() {
            Readiness::Quiet
        } else {
            Readiness::Readable
        }
    }
}

/// What a poll found: the wake-up and listener, and the subscribers that have something to
/// read or have hung up, by id.
struct Ready {
    listener: Readiness,
    wake: Readiness,
    clients: Vec<ClientId>,
}

pub(super) struct Server {
    listener: OwnedFd,
    wake: Arc<OwnedFd>,
    rx: Receiver<ToSocket>,
    clients: Vec<Client>,
    next_client: u64,
    /// Whether the listener is polled: not while `accept` is failing for want of descriptors.
    intake: Intake,
    complaints: Complaints,
    device: Option<DeviceInfo>,
    notes: Sender<FromSocket>,
    claim: Over,
    lock: LockState,
    seat: SeatFile,
    /// Who has the seat as of the last look.
    active: Option<u32>,
    /// When the seat file was last read.
    looked: Instant,
    /// Says which connection is the shell.
    vouch: Box<dyn Vouch>,
}

impl Server {
    pub(super) fn new(
        listener: OwnedFd,
        wake: Arc<OwnedFd>,
        rx: Receiver<ToSocket>,
        notes: Sender<FromSocket>,
        seat: SeatFile,
        vouch: Box<dyn Vouch>,
    ) -> Self {
        Self {
            listener,
            wake,
            rx,
            clients: Vec::new(),
            next_client: 0,
            intake: Intake::Open,
            complaints: Complaints::default(),
            device: None,
            notes,
            claim: Over::NotOurs,
            // Locked until a subscriber says otherwise: the tracker starts locked too, so the two
            // agree and nothing is announced for a state nobody changed.
            lock: LockState::Locked,
            active: seat.active_uid(),
            looked: Instant::now(),
            seat,
            vouch,
        }
    }

    pub(super) fn run(mut self) -> Exit {
        loop {
            if let Flow::Stop(exit) = self.step() {
                return exit;
            }
        }
    }

    /// One pass: wait, accept, deliver, read, publish.
    pub(super) fn step(&mut self) -> Flow {
        self.intake = self.intake.at(Instant::now());
        let ready = match self.wait() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("palmrest: socket poll failed: {e}");
                return Flow::Stop(Exit::PollFailed);
            }
        };
        self.watch_seat();
        if ready.listener == Readiness::Readable {
            self.accept();
        }
        if ready.wake == Readiness::Readable && !self.drain() {
            return Flow::Stop(Exit::ReaderGone);
        }
        ready.clients.into_iter().for_each(|id| self.receive(id));
        self.publish();
        Flow::Continue
    }

    fn wait(&self) -> Result<Ready, Errno> {
        let mut fds = vec![PollFd::new(&*self.wake, PollFlags::IN)];
        let listener_slot = (self.intake == Intake::Open).then(|| {
            fds.push(PollFd::new(&self.listener, PollFlags::IN));
            fds.len() - 1
        });
        let first_client = fds.len();
        fds.extend(
            self.clients
                .iter()
                .map(|c| PollFd::new(c.fd(), PollFlags::IN)),
        );
        // Never asleep for longer than the seat recheck: a user switch with no traffic is noticed.
        let timeout = Some(timespec(
            self.intake
                .sleep(Instant::now())
                .map_or(SEAT_RECHECK, |pause| pause.min(SEAT_RECHECK)),
        ));
        match poll(&mut fds, timeout.as_ref()) {
            Ok(_) | Err(Errno::INTR) => {}
            Err(e) => return Err(e),
        }
        Ok(Ready {
            listener: listener_slot.map_or(Readiness::Quiet, |slot| Readiness::of(&fds[slot])),
            wake: Readiness::of(&fds[0]),
            clients: self
                .clients
                .iter()
                .zip(&fds[first_client..])
                .filter(|(_, f)| Readiness::of(f) == Readiness::Readable)
                .map(|(c, _)| c.id)
                .collect(),
        })
    }

    /// Accepts what is waiting, a bounded number per pass. A failure that retrying cannot fix
    /// (no descriptors left) leaves the connection pending, so the listener is not polled for a
    /// while instead of waking the loop in a spin.
    fn accept(&mut self) {
        for _ in 0..ACCEPTS_PER_PASS {
            let accepted = rustix::net::accept_with(
                &self.listener,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            );
            match accepted {
                Ok(fd) => self.admit(fd),
                Err(e) => match accept_failure(e) {
                    AcceptFailure::NothingPending => return,
                    AcceptFailure::Retry => {}
                    AcceptFailure::Pause => {
                        eprintln!("palmrest: accept failed: {e}; not accepting for a moment");
                        self.intake = Intake::paused_at(Instant::now());
                        return;
                    }
                },
            }
        }
    }

    /// Reads the seat file when it is due.
    fn watch_seat(&mut self) {
        if Instant::now().saturating_duration_since(self.looked) >= SEAT_RECHECK {
            self.look_at_seat();
        }
    }

    /// Reads the seat file now. When another user has the seat, every connection that is not
    /// theirs is closed, and with it whatever it said about the lock. Returns who has the seat.
    pub(super) fn look_at_seat(&mut self) -> Option<u32> {
        let active = self.seat.active_uid();
        self.looked = Instant::now();
        if active != self.active {
            self.active = active;
            self.clients.retain(|c| c.peer(active) != Peer::Observer);
        }
        active
    }

    /// Lets a connection in after its greeting, or closes it at once when it is not the person at
    /// the seat's, or when the daemon has enough.
    pub(super) fn admit(&mut self, fd: OwnedFd) {
        let Ok(cred) = rustix::net::sockopt::socket_peercred(&fd) else {
            return;
        };
        let uid = cred.uid.as_raw();
        // Asked of every connection, whoever has the seat now: the shell may connect a moment
        // before logind names its user the seat's owner.
        let origin = self.vouch.origin(&fd);
        if peer_of(uid, self.look_at_seat(), origin.claim()) == Peer::Observer {
            return;
        }
        if self.clients.len() >= MAX_CLIENTS {
            let (complaints, voice) = self.complaints.note(Instant::now());
            self.complaints = complaints;
            if voice == Voice::Speak {
                eprintln!("palmrest: refused a subscriber: {MAX_CLIENTS} are connected");
            }
            return;
        }
        let client = Client::new(ClientId(self.next_client), fd, uid, origin);
        self.next_client += 1;
        let hello = ToHost::Hello {
            version: PROTOCOL_VERSION,
            device: self.device.clone().unwrap_or(DeviceInfo {
                name: String::new(),
                path: String::new(),
                vendor: 0,
                product: 0,
            }),
        };
        let alive = client.greet(&hello) == Link::Alive
            && (self.device.is_some() || client.greet(&ToHost::DeviceGone) == Link::Alive);
        if alive {
            self.clients.push(client);
        }
    }

    /// Delivers everything queued; false when the reader has gone (shutdown).
    fn drain(&mut self) -> bool {
        let mut buf = [0u8; 8];
        let _ = rustix::io::read(&*self.wake, &mut buf);
        loop {
            match self.rx.try_recv() {
                Ok(ToSocket::Device(d)) => {
                    self.device = Some(d.clone());
                    self.broadcast(&ToHost::Hello {
                        version: PROTOCOL_VERSION,
                        device: d,
                    });
                }
                Ok(ToSocket::Gone) => self.device = None,
                Ok(ToSocket::Msg(m)) => self.broadcast(&m),
                Err(mpsc::TryRecvError::Empty) => return true,
                Err(mpsc::TryRecvError::Disconnected) => return false,
            }
        }
    }

    fn broadcast(&mut self, m: &ToHost) {
        self.clients.retain(|c| c.deliver(m) == Link::Alive);
    }

    /// Reads what subscriber `id` sent. The subscriber may have left since the poll (a failed
    /// broadcast removes it), so it is looked up by id and a missing one is skipped.
    fn receive(&mut self, id: ClientId) {
        let active = self.active;
        let gone = self
            .clients
            .iter_mut()
            .find(|c| c.id == id)
            .is_some_and(|c| c.pull(active) == Link::Gone);
        if gone {
            self.clients.retain(|c| c.id != id);
        }
    }

    /// Tells the reader what the subscribers jointly say, when it changed.
    fn publish(&mut self) {
        let claim = aggregate(self.clients.iter().map(|c| c.pointer(self.active)));
        if claim != self.claim {
            self.claim = claim;
            let _ = self.notes.send(FromSocket::Pointer(claim));
        }
        let lock = aggregate_lock(self.clients.iter().map(|c| c.session(self.active)));
        if lock != self.lock {
            self.lock = lock;
            let _ = self.notes.send(FromSocket::Session(lock));
        }
    }

    /// The subscribers connected now (for the pass-level tests).
    #[cfg(test)]
    pub(super) fn subscribers(&self) -> usize {
        self.clients.len()
    }
}

fn timespec(d: Duration) -> Timespec {
    Timespec {
        tv_sec: d.as_secs().try_into().unwrap_or(1),
        tv_nsec: i64::from(d.subsec_nanos()),
    }
}
