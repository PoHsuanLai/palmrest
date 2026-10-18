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
        ToHost::Run {
            action: HostAction::Command {
                argv: vec!["notify-send".into(), "hi \"there\"".into()],
            },
        },
        ToHost::Run {
            action: HostAction::Workspace(Workspace::Overview),
        },
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
        ToDaemon::UserConfig {
            toml: "[scroll]\nspeed = 30\n".into(),
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
    assert_eq!(got, [true, true, false, false, false, true, true]);
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

#[test]
fn the_largest_settings_text_fits_a_packet_with_every_character_escaped() {
    let worst = ToDaemon::UserConfig {
        toml: "\"\n".repeat(USER_CONFIG_MAX_BYTES / 2),
    };
    assert!(encode(&worst).len() <= MAX_PACKET);
}

#[test]
fn an_action_fits_when_it_has_words_in_bounds() {
    let words = |n: usize, len: usize| vec!["w".repeat(len); n];
    let cases = [
        (
            "a workspace step",
            HostAction::Workspace(Workspace::Next),
            true,
        ),
        (
            "a short command",
            HostAction::Command { argv: words(3, 8) },
            true,
        ),
        (
            "the most words",
            HostAction::Shell {
                words: words(ACTION_WORDS_MAX, 1),
            },
            true,
        ),
        (
            "one word too many",
            HostAction::Shell {
                words: words(ACTION_WORDS_MAX + 1, 1),
            },
            false,
        ),
        (
            "a word too long",
            HostAction::Command {
                argv: words(1, ACTION_WORD_MAX_BYTES + 1),
            },
            false,
        ),
        ("no words", HostAction::Command { argv: vec![] }, false),
    ];
    for (name, action, want) in cases {
        assert_eq!(action.fits(), want, "{name}");
    }
}
