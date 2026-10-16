//! Round trips, the wire form, filters and the version check.

use crate::*;

fn samples() -> Vec<ToHost> {
    vec![
        ToHost::Hello {
            version: PROTOCOL_VERSION,
            device: DeviceInfo {
                name: "mouse".into(),
                path: "/dev/hidraw3".into(),
                vendor: 0x4c,
                product: 0x269,
            },
        },
        ToHost::Scroll {
            gesture: GestureId(7),
            phase: Phase::Changed,
            dx_px: 0.0,
            dy_px: -12.25,
            fingers: 1,
            axis: Some(Axis::Y),
            t_ns: 123,
        },
        ToHost::Swipe {
            gesture: GestureId(8),
            phase: Phase::Began,
            fingers: 2,
            dx_units: 0.0,
            t_ns: 5,
        },
        ToHost::Tap {
            kind: TapKind::Two,
            t_ns: 9,
        },
        ToHost::Touches {
            active: ActiveTouch::Down,
        },
        ToHost::DeviceGone,
    ]
}

#[test]
fn every_message_round_trips() {
    for m in samples() {
        assert_eq!(decode::<ToHost>(&encode(&m)).unwrap(), m);
        assert!(encode(&m).len() < MAX_PACKET);
    }
    for m in [
        ToDaemon::PointerOver { over: Over::Ours },
        ToDaemon::Subscribe {
            filter: Filter::Only(vec![Topic::Scroll, Topic::Touches]),
        },
        ToDaemon::SessionLock {
            state: LockState::Locked,
        },
    ] {
        assert_eq!(decode::<ToDaemon>(&encode(&m)).unwrap(), m);
    }
}

#[test]
fn wire_form_is_adjacently_tagged() {
    let bytes = encode(&ToDaemon::PointerOver { over: Over::Ours });
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        r#"{"kind":"pointer_over","v":{"over":"ours"}}"#
    );
    assert_eq!(
        String::from_utf8(encode(&ToHost::DeviceGone)).unwrap(),
        r#"{"kind":"device_gone"}"#
    );
}

#[test]
fn filters_admit_by_topic() {
    let only = Filter::Only(vec![Topic::Scroll]);
    let got: Vec<bool> = samples().iter().map(|m| only.admits(m)).collect();
    assert_eq!(got, [true, true, false, false, false, true]);
    assert!(samples().iter().all(|m| Filter::All.admits(m)));
}

#[test]
fn junk_is_an_error_not_a_panic() {
    assert!(decode::<ToDaemon>(b"{").is_err());
    assert!(decode::<ToDaemon>(&[b' '; MAX_PACKET + 1]).is_err());
}

#[test]
fn only_a_hello_of_another_version_is_incompatible() {
    let hello = |version| ToHost::Hello {
        version,
        device: DeviceInfo {
            name: String::new(),
            path: String::new(),
            vendor: 0,
            product: 0,
        },
    };
    assert_eq!(compat(&hello(PROTOCOL_VERSION)), Compat::Compatible);
    assert_eq!(
        compat(&hello(PROTOCOL_VERSION + 1)),
        Compat::Incompatible {
            version: PROTOCOL_VERSION + 1
        }
    );
    assert_eq!(compat(&ToHost::DeviceGone), Compat::Compatible);
}
