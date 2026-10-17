//! The socket thread end to end on a private path: Hello first, fan-out, filters, a subscriber's
//! `PointerOver` becoming the tracker's claim (acceptance 10's socket half), and who is served and
//! believed: the person at the seat, and of their programs only the shell about the lock.

use super::limits::{MAX_CLIENTS, PACKETS_PER_PASS, QUEUE_MAX};
use super::server::{Flow, Server};
use super::shell::tests::shell_of;
use super::shell::{Origin, Steering, Vouch};
use super::{FromSocket, SeatFile, SocketHandle, ToSocket, aggregate, aggregate_lock, bind, spawn};
use palmrest_protocol::{
    self as protocol, ActiveTouch, DeviceInfo, Filter, LockState, Over, PROTOCOL_VERSION, TapKind,
    ToDaemon, ToHost, Topic,
};
use rustix::event::{EventfdFlags, eventfd};
use rustix::fd::OwnedFd;
use rustix::net::{
    AddressFamily, RecvFlags, SendFlags, SocketAddrUnix, SocketFlags, SocketType, socketpair,
};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The uid of this test process, who has the seat in these tests.
fn me() -> u32 {
    rustix::process::getuid().as_raw()
}

/// A seat file that names `active` as the person at the seat (none: no session).
fn seat_file(dir: &Path, active: Option<u32>) -> SeatFile {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("seat0");
    match active {
        Some(uid) => std::fs::write(&path, format!("IS_SEAT0=1\nACTIVE_UID={uid}\n")).unwrap(),
        None => std::fs::write(&path, "IS_SEAT0=1\n").unwrap(),
    }
    SeatFile(path)
}

/// What a test connection is to the daemon.
#[derive(Debug, Clone, Copy)]
enum Role {
    /// The shell of the person at the seat.
    Shell,
    /// Another program of theirs.
    Other,
}

/// Names each connection in the order the test makes them.
#[derive(Clone, Default)]
struct Scripted(Arc<Mutex<VecDeque<Role>>>);

impl Vouch for Scripted {
    fn origin(&self, _stream: &rustix::fd::OwnedFd) -> Origin {
        match self.0.lock().unwrap().pop_front() {
            Some(Role::Shell) => shell_of(me()),
            Some(Role::Other) | None => Origin::Other,
        }
    }
}

#[test]
fn the_session_is_locked_unless_a_subscriber_has_said_unlocked_and_none_says_locked() {
    use LockState::{Locked, Unlocked};
    let table: &[(&str, Vec<Option<LockState>>, LockState)] = &[
        ("no subscriber is connected", vec![], Locked),
        ("one that has said nothing", vec![None], Locked),
        ("several that have said nothing", vec![None, None], Locked),
        ("one that says unlocked", vec![Some(Unlocked)], Unlocked),
        (
            "unlocked beside one that is silent",
            vec![None, Some(Unlocked)],
            Unlocked,
        ),
        ("one that says locked", vec![Some(Locked)], Locked),
        (
            "locked beats unlocked",
            vec![Some(Unlocked), Some(Locked)],
            Locked,
        ),
        (
            "locked beats unlocked, either order",
            vec![Some(Locked), Some(Unlocked), None],
            Locked,
        ),
    ];
    for (name, said, want) in table {
        assert_eq!(aggregate_lock(said.iter().copied()), *want, "{name}");
    }
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
        0o666,
        "open to every account; the server decides whom to serve"
    );
    let (claims_tx, claims) = mpsc::channel();
    // Every program of the test user stands for the shell: these clients are in no unit.
    let handle = spawn(
        listener,
        claims_tx,
        seat_file(&dir, Some(me())),
        Steering::SeatUser,
    )
    .unwrap();

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
    roles: Scripted,
}

impl Stepped {
    /// A server where this test's user has the seat.
    fn new(name: &str) -> Self {
        Self::at_seat(name, Some(me()))
    }

    /// A server where `active` has the seat.
    fn at_seat(name: &str, active: Option<u32>) -> Self {
        let dir = std::env::temp_dir().join(format!("palmrest-{name}-{}", std::process::id()));
        let listener = bind(&dir.join("events.sock")).unwrap();
        let wake = Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK).unwrap());
        let (tx, rx) = mpsc::sync_channel(QUEUE_MAX);
        let (claims_tx, claims) = mpsc::channel();
        let roles = Scripted::default();
        Self {
            server: Server::new(
                listener,
                wake.clone(),
                rx,
                claims_tx,
                seat_file(&dir, active),
                Box::new(roles.clone()),
            ),
            handle: SocketHandle { tx, wake },
            claims,
            dir,
            roles,
        }
    }

    /// Connects the shell through a socket pair and returns its end.
    fn subscribe(&mut self) -> OwnedFd {
        self.connect(Role::Shell)
    }

    /// Connects a program that is `role` through a socket pair and returns its end.
    fn connect(&mut self, role: Role) -> OwnedFd {
        let (ours, theirs) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        self.roles.0.lock().unwrap().push_back(role);
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
fn the_session_is_unlocked_only_by_the_shell_and_any_subscriber_can_lock_it() {
    let mut s = Stepped::new("lock");
    let shell = s.subscribe();
    let other = s.connect(Role::Other);
    let tell = |fd: &OwnedFd, state| {
        rustix::net::send(
            fd,
            &protocol::encode(&ToDaemon::SessionLock { state }),
            SendFlags::NOSIGNAL,
        )
        .unwrap();
    };
    // No step yet: with nothing to read it would wait for ever.
    assert!(
        s.claims.try_recv().is_err(),
        "subscribers that have said nothing leave it locked, and that is no news"
    );

    tell(&other, LockState::Unlocked);
    assert_eq!(s.server.step(), Flow::Continue);
    assert!(
        s.claims.try_recv().is_err(),
        "a program that is not the shell cannot unlock the session"
    );

    tell(&shell, LockState::Unlocked);
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(
        s.claims.try_recv(),
        Ok(FromSocket::Session(LockState::Unlocked))
    );

    tell(&other, LockState::Locked);
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(
        s.claims.try_recv(),
        Ok(FromSocket::Session(LockState::Locked)),
        "any program of the person can lock it"
    );

    drop(other);
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(
        s.claims.try_recv(),
        Ok(FromSocket::Session(LockState::Unlocked)),
        "the one that locked it left, and the shell still says unlocked"
    );

    drop(shell);
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(
        s.claims.try_recv(),
        Ok(FromSocket::Session(LockState::Locked)),
        "with no subscriber connected the session counts as locked"
    );
}

#[test]
fn only_the_shell_is_believed_about_the_pointer() {
    let mut s = Stepped::new("pointer");
    let other = s.connect(Role::Other);
    rustix::net::send(
        &other,
        &protocol::encode(&ToDaemon::PointerOver { over: Over::Ours }),
        SendFlags::NOSIGNAL,
    )
    .unwrap();
    assert_eq!(s.server.step(), Flow::Continue);
    assert!(s.claims.try_recv().is_err());
}

#[test]
fn only_the_person_at_the_seat_is_served() {
    for (name, active) in [("another-user", Some(me() + 1)), ("no-session", None)] {
        let mut s = Stepped::at_seat(name, active);
        let stranger = s.connect(Role::Shell);
        assert_eq!(s.server.subscribers(), 0, "{name}");
        assert!(queued(&stranger).is_empty(), "{name}: no greeting");
    }
}

#[test]
fn a_user_switch_closes_the_connections_of_the_one_who_left() {
    let mut s = Stepped::new("switch");
    let shell = s.subscribe();
    rustix::net::send(
        &shell,
        &protocol::encode(&ToDaemon::SessionLock {
            state: LockState::Unlocked,
        }),
        SendFlags::NOSIGNAL,
    )
    .unwrap();
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(
        s.claims.try_recv(),
        Ok(FromSocket::Session(LockState::Unlocked))
    );

    seat_file(&s.dir, Some(me() + 1));
    assert_eq!(s.server.look_at_seat(), Some(me() + 1));
    assert_eq!(s.server.subscribers(), 0);
    s.handle.send(ToSocket::Msg(TOUCHES));
    assert_eq!(s.server.step(), Flow::Continue);
    assert_eq!(
        s.claims.try_recv(),
        Ok(FromSocket::Session(LockState::Locked)),
        "the session is locked again for whoever comes next"
    );
}
