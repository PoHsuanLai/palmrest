//! The O1 socket end to end on a private path: Hello first, fan-out, filters, and a
//! subscriber's `PointerOver` becoming the tracker's claim (acceptance 10's socket half).

use palmrest::daemon::socket::{self, ToSocket};
use palmrest_protocol::{
    self as protocol, ActiveTouch, DeviceInfo, Filter, Over, PROTOCOL_VERSION, TapKind, ToDaemon,
    ToHost, Topic,
};
use rustix::fd::OwnedFd;
use rustix::net::{AddressFamily, RecvFlags, SendFlags, SocketAddrUnix, SocketType};
use std::sync::mpsc;
use std::time::Duration;

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
    let listener = socket::bind(&path).unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let (claims_tx, claims) = mpsc::channel();
    let handle = socket::spawn(listener, claims_tx).unwrap();

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
    assert_eq!(claims.recv_timeout(Duration::from_secs(2)), Ok(Over::Ours));

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
        Ok(Over::NotOurs),
        "a leaving releases its claim"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
