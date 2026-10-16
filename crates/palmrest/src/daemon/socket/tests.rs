//! The socket thread end to end on a private path: Hello first, fan-out, filters, and a
//! subscriber's `PointerOver` becoming the tracker's claim (acceptance 10's socket half).

use super::limits::{MAX_CLIENTS, PACKETS_PER_PASS, QUEUE_MAX};
use super::server::{Flow, Server};
use super::{FromSocket, SocketHandle, ToSocket, aggregate, aggregate_lock, bind, spawn};
use palmrest_protocol::{
    self as protocol, ActiveTouch, DeviceInfo, Filter, LockState, Over, PROTOCOL_VERSION, TapKind,
    ToDaemon, ToHost, Topic,
};
use rustix::event::{EventfdFlags, eventfd};
use rustix::fd::OwnedFd;
use rustix::net::{
    AddressFamily, RecvFlags, SendFlags, SocketAddrUnix, SocketFlags, SocketType, socketpair,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

#[test]
fn any_lock_wins() {
    assert_eq!(aggregate_lock([]), LockState::Unlocked);
    assert_eq!(
        aggregate_lock([LockState::Unlocked, LockState::Locked]),
        LockState::Locked
    );
}

#[test]
fn any_claim_wins() {
    assert_eq!(aggregate([]), Over::NotOurs);
    assert_eq!(aggregate([Over::NotOurs, Over::Ours]), Over::Ours);
    assert_eq!(aggregate([Over::NotOurs, Over::NotOurs]), Over::NotOurs);
}

fn connect(path: &std::path::Path) -> OwnedFd {
    let fd = rustix::net::socket(AddressFamily::UNIX, SocketType::SEQPACKET, None).unwrap();
    rustix::net::connect(&fd, &SocketAddrUnix::new(path).unwrap()).unwrap();
    rustix::net::sockopt::set_socket_timeout(
        &fd,
        rustix::net::sockopt::Timeout::Recv,
        Some(Duration::from_secs(2)),
    )
    .unwrap();
    fd
}

fn recv(fd: &OwnedFd) -> ToHost {
    let mut buf = [0u8; protocol::MAX_PACKET];
    let (n, _) = rustix::net::recv(fd, &mut buf, RecvFlags::empty()).unwrap();
    protocol::decode(&buf[..n]).unwrap()
}

fn send(fd: &OwnedFd, m: &ToDaemon) {
    rustix::net::send(fd, &protocol::encode(m), SendFlags::NOSIGNAL).unwrap();
}

#[test]
fn hello_fanout_filters_and_claims() {
    let dir = std::env::temp_dir().join(format!("palmrest-sock-{}", std::process::id()));
    let path = dir.join("events.sock");
    let listener = bind(&path).unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let (claims_tx, claims) = mpsc::channel();
    let handle = spawn(listener, claims_tx).unwrap();

    let a = connect(&path);
    assert!(matches!(
        recv(&a),
        ToHost::Hello {
            version: PROTOCOL_VERSION,
            ..
        }
    ));
    assert_eq!(recv(&a), ToHost::DeviceGone, "no mouse yet");

    let device = DeviceInfo {
        name: "mouse".into(),
        path: "/dev/hidraw9".into(),
        vendor: 0x4c,
        product: 0x269,
    };
    handle.send(ToSocket::Device(device.clone()));
    assert_eq!(
        recv(&a),
        ToHost::Hello {
            version: PROTOCOL_VERSION,
            device: device.clone()
        }
    );

    let b = connect(&path);
    assert_eq!(
        recv(&b),
        ToHost::Hello {
            version: PROTOCOL_VERSION,
            device
        },
        "late subscribers learn the device"
    );
    send(
        &b,
        &ToDaemon::Subscribe {
            filter: Filter::Only(vec![Topic::Tap]),
        },
    );

    send(&a, &ToDaemon::PointerOver { over: Over::Ours });
    assert_eq!(
        claims.recv_timeout(Duration::from_secs(2)),
        Ok(FromSocket::Pointer(Over::Ours))
    );

    std::thread::sleep(Duration::from_millis(50)); // let b's Subscribe land before the fan-out
    let touches = ToHost::Touches {
        active: ActiveTouch::Down,
    };
    let tap = ToHost::Tap {
        kind: TapKind::Two,
        t_ns: 1,
    };
    handle.send(ToSocket::Msg(touches.clone()));
    handle.send(ToSocket::Msg(tap.clone()));
    assert_eq!(recv(&a), touches);
    assert_eq!(recv(&a), tap);
    assert_eq!(recv(&b), tap, "b filtered Touches out");

    drop(a);
    assert_eq!(
        claims.recv_timeout(Duration::from_secs(2)),
        Ok(FromSocket::Pointer(Over::NotOurs)),
        "a leaving releases its claim"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A server driven one pass at a time on the test's thread, over a private listener.
struct Stepped {
    server: Server,
    handle: SocketHandle,
    claims: Receiver<FromSocket>,
    dir: PathBuf,
}

impl Stepped {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("palmrest-{name}-{}", std::process::id()));
        let listener = bind(&dir.join("events.sock")).unwrap();
        let wake = Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK).unwrap());
        let (tx, rx) = mpsc::sync_channel(QUEUE_MAX);
        let (claims_tx, claims) = mpsc::channel();
        Self {
            server: Server::new(listener, wake.clone(), rx, claims_tx),
            handle: SocketHandle { tx, wake },
            claims,
            dir,
        }
    }

    /// Connects a subscriber through a socket pair and returns its end.
    fn subscribe(&mut self) -> OwnedFd {
        let (ours, theirs) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        self.server.admit(ours);
        theirs
    }
}

impl Drop for Stepped {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Everything queued for a subscriber, decoded.
fn queued(fd: &OwnedFd) -> Vec<ToHost> {
    let mut buf = [0u8; protocol::MAX_PACKET];
    let mut out = Vec::new();
    while let Ok((n, _)) = rustix::net::recv(fd, &mut buf, RecvFlags::DONTWAIT) {
        if n == 0 {
            break;
        }
        out.push(protocol::decode(&buf[..n]).unwrap());
    }
    out
}

const TOUCHES: ToHost = ToHost::Touches {
    active: ActiveTouch::Down,
};

#[test]
fn a_subscriber_closing_during_a_broadcast_is_dropped_not_indexed() {
    let mut s = Stepped::new("close-in-broadcast");
    let stays = s.subscribe();
    let leaves = s.subscribe();
    drop(leaves);
    // The closed peer is readable (hang-up) and fails the broadcast in the same pass.
    s.handle.send(ToSocket::Msg(TOUCHES));
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(s.server.subscribers(), 1);
    assert_eq!(queued(&stays).last(), Some(&TOUCHES));
}

#[test]
fn subscribers_beyond_the_cap_are_closed_at_once() {
    let mut s = Stepped::new("cap");
    let peers: Vec<OwnedFd> = (0..MAX_CLIENTS + 3).map(|_| s.subscribe()).collect();
    assert_eq!(s.server.subscribers(), MAX_CLIENTS);
    let refused = &peers[MAX_CLIENTS];
    assert!(queued(refused).is_empty(), "no greeting for a refused one");
    let mut buf = [0u8; 8];
    assert_eq!(
        rustix::net::recv(refused, &mut buf, RecvFlags::DONTWAIT).map(|(n, _)| n),
        Ok(0),
        "the refused connection is closed"
    );
}

#[test]
fn a_flood_is_read_a_budget_at_a_time() {
    let mut s = Stepped::new("flood");
    let chatty = s.subscribe();
    let say = |over| {
        rustix::net::send(
            &chatty,
            &protocol::encode(&ToDaemon::PointerOver { over }),
            SendFlags::NOSIGNAL,
        )
        .unwrap();
    };
    (0..PACKETS_PER_PASS).for_each(|_| say(Over::NotOurs));
    say(Over::Ours);
    assert_eq!(s.server.step(), Flow::Continue);
    assert!(
        s.claims.try_recv().is_err(),
        "the last packet waits for the next pass"
    );
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(s.claims.try_recv(), Ok(FromSocket::Pointer(Over::Ours)));
}

#[test]
fn a_full_queue_drops_messages_instead_of_blocking_the_reader() {
    let mut s = Stepped::new("queue");
    let _peer = s.subscribe();
    for _ in 0..QUEUE_MAX + 10 {
        s.handle.send(ToSocket::Msg(TOUCHES));
    }
    assert_eq!(s.server.step(), Flow::Continue);
}

#[test]
fn the_session_is_locked_while_a_subscriber_says_so() {
    let mut s = Stepped::new("lock");
    let shell = s.subscribe();
    let other = s.subscribe();
    let tell = |fd: &OwnedFd, state| {
        rustix::net::send(
            fd,
            &protocol::encode(&ToDaemon::SessionLock { state }),
            SendFlags::NOSIGNAL,
        )
        .unwrap();
    };
    tell(&shell, LockState::Locked);
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(
        s.claims.try_recv(),
        Ok(FromSocket::Session(LockState::Locked))
    );

    tell(&other, LockState::Unlocked);
    assert_eq!(s.server.step(), Flow::Continue);
    assert!(
        s.claims.try_recv().is_err(),
        "another subscriber cannot unlock it"
    );

    drop(shell);
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(
        s.claims.try_recv(),
        Ok(FromSocket::Session(LockState::Unlocked)),
        "a subscriber that leaves stops saying it"
    );
}
