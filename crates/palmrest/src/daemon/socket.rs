//! O1: the socket thread (12.3.10). Accepts subscribers, fans messages out, and turns their
//! `PointerOver` reports into the one claim the tracker needs (11.3.14: while any subscriber
//! has the pointer, foreign outputs are silent).
//!
//! The reader thread hands messages over a channel and wakes this thread with an eventfd, so
//! the reader never blocks on a slow subscriber: a subscriber whose queue is full misses that
//! message rather than delaying everyone else's scroll.

use palmrest_protocol::{
    self as protocol, DeviceInfo, Filter, Over, PROTOCOL_VERSION, ToDaemon, ToHost,
};
use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::fd::{AsFd, OwnedFd};
use rustix::io::Errno;
use rustix::net::{AddressFamily, RecvFlags, SendFlags, SocketAddrUnix, SocketFlags, SocketType};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

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
    tx: Sender<ToSocket>,
    wake: Arc<OwnedFd>,
}

impl SocketHandle {
    /// Queues `m` and wakes the socket thread. A dead socket thread is logged by its own exit.
    pub fn send(&self, m: ToSocket) {
        if self.tx.send(m).is_ok() {
            let _ = rustix::io::write(&*self.wake, &1u64.to_ne_bytes());
        }
    }
}

/// One subscriber.
#[derive(Debug)]
struct Client {
    fd: OwnedFd,
    filter: Filter,
    over: Over,
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

/// Starts the socket thread on a bound listener. Claims go to `claims`.
pub fn spawn(listener: OwnedFd, claims: Sender<Over>) -> io::Result<SocketHandle> {
    let wake = Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?);
    let (tx, rx) = mpsc::channel();
    let server = Server {
        listener,
        wake: wake.clone(),
        rx,
        clients: Vec::new(),
        device: None,
        claims,
        claim: Over::NotOurs,
    };
    std::thread::Builder::new()
        .name("palmrest-socket".to_owned())
        .spawn(move || server.run())?;
    Ok(SocketHandle { tx, wake })
}

struct Server {
    listener: OwnedFd,
    wake: Arc<OwnedFd>,
    rx: Receiver<ToSocket>,
    clients: Vec<Client>,
    device: Option<DeviceInfo>,
    claims: Sender<Over>,
    claim: Over,
}

impl Server {
    fn run(mut self) {
        loop {
            let ready = match self.wait() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("palmrest: socket poll failed: {e}");
                    return;
                }
            };
            if ready.listener {
                self.accept();
            }
            if ready.wake && !self.drain() {
                return;
            }
            for i in ready.clients.into_iter().rev() {
                self.receive(i);
            }
            self.publish_claim();
        }
    }

    fn wait(&self) -> io::Result<Ready> {
        let mut fds: Vec<PollFd<'_>> = vec![
            PollFd::new(&self.listener, PollFlags::IN),
            PollFd::new(&*self.wake, PollFlags::IN),
        ];
        fds.extend(
            self.clients
                .iter()
                .map(|c| PollFd::new(&c.fd, PollFlags::IN)),
        );
        poll(&mut fds, None)?;
        let hit = |f: &PollFd<'_>| !f.revents().is_empty();
        Ok(Ready {
            listener: hit(&fds[0]),
            wake: hit(&fds[1]),
            clients: fds[2..]
                .iter()
                .enumerate()
                .filter(|(_, f)| hit(f))
                .map(|(i, _)| i)
                .collect(),
        })
    }

    fn accept(&mut self) {
        while let Ok(fd) =
            rustix::net::accept_with(&self.listener, SocketFlags::CLOEXEC | SocketFlags::NONBLOCK)
        {
            let client = Client {
                fd,
                filter: Filter::All,
                over: Over::NotOurs,
            };
            let hello = ToHost::Hello {
                version: PROTOCOL_VERSION,
                device: self.device.clone().unwrap_or(DeviceInfo {
                    name: String::new(),
                    path: String::new(),
                    vendor: 0,
                    product: 0,
                }),
            };
            let alive = send(&client, &hello)
                && (self.device.is_some() || send(&client, &ToHost::DeviceGone));
            if alive {
                self.clients.push(client);
            }
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
        self.clients.retain(|c| !c.filter.admits(m) || send(c, m));
    }

    fn receive(&mut self, i: usize) {
        let mut buf = [0u8; protocol::MAX_PACKET];
        let keep = loop {
            match rustix::net::recv(&self.clients[i].fd, &mut buf, RecvFlags::DONTWAIT) {
                Ok((0, _)) => break false,
                Ok((n, _)) => match protocol::decode::<ToDaemon>(&buf[..n]) {
                    Ok(ToDaemon::PointerOver { over }) => self.clients[i].over = over,
                    Ok(ToDaemon::Subscribe { filter }) => self.clients[i].filter = filter,
                    Err(e) => eprintln!("palmrest: ignoring a malformed subscriber message: {e}"),
                },
                Err(Errno::AGAIN) => break true,
                Err(_) => break false,
            }
        };
        if !keep {
            self.clients.remove(i);
        }
    }

    fn publish_claim(&mut self) {
        let now = aggregate(self.clients.iter().map(|c| c.over));
        if now != self.claim {
            self.claim = now;
            let _ = self.claims.send(now);
        }
    }
}

struct Ready {
    listener: bool,
    wake: bool,
    clients: Vec<usize>,
}

/// Sends one message; false if the subscriber is gone. A full queue drops the message only.
fn send(c: &Client, m: &ToHost) -> bool {
    match rustix::net::send(
        c.fd.as_fd(),
        &protocol::encode(m),
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    ) {
        Ok(_) | Err(Errno::AGAIN) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests;
