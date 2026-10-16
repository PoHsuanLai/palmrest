//! The emitter against the fake service: nothing here opens a socket or a device.

use super::*;
use crate::keys::Key;
use crate::outputs::pad::{CENTRE, PadFrame};
use turnstile_client::fake::{FakeSource, OpenMode, SendMode};

const LAND: PadFrame = PadFrame::Land {
    fingers: 2,
    at: CENTRE,
    spacing: 400,
};

fn scroll_one() -> WheelDelta {
    WheelDelta {
        vertical: 1,
        vertical_hi_res: 120,
        ..WheelDelta::default()
    }
}

fn emitter(
    source: &FakeSource,
    foreign: ForeignOutput,
    combos: &[KeyCombo],
) -> UinputEmitter<<FakeSource as DeviceSource>::Session> {
    UinputEmitter::create(source, foreign, combos).expect("the fake service makes devices")
}

#[test]
fn devices_are_asked_for_by_need_and_keys_only_for_combos() {
    let both = [KeyCombo::alt(Key::LEFT), KeyCombo::alt(Key::RIGHT)];
    let cases: &[(&str, ForeignOutput, &[KeyCombo], Vec<Create>)] = &[
        (
            "a touchpad and no combos",
            ForeignOutput::Touchpad,
            &[],
            vec![Create::touchpad()],
        ),
        (
            "a wheel and the keys of two combos, once each",
            ForeignOutput::Wheel,
            &both,
            vec![Create::wheel(), Create::keys(vec![56, 105, 106])],
        ),
        (
            "nothing foreign, but keys",
            ForeignOutput::Off,
            &both[..1],
            vec![Create::keys(vec![56, 105])],
        ),
        ("nothing at all", ForeignOutput::Off, &[], vec![]),
    ];
    for (name, foreign, combos, want) in cases {
        let source = FakeSource::new();
        let _ = emitter(&source, *foreign, combos);
        assert_eq!(&source.state().created, want, "{name}");
    }
}

#[test]
fn a_pad_frame_is_one_batch_of_its_encoded_events() {
    let source = FakeSource::new();
    let mut e = emitter(&source, ForeignOutput::Touchpad, &[]);
    e.pad(&LAND).unwrap();
    e.pad(&PadFrame::Lift).unwrap();
    let (_, land) = PadEncoder::default().encode(&LAND);
    let state = source.state();
    assert_eq!(state.sent.len(), 2, "one datagram per frame");
    assert_eq!(
        state.sent[0].1,
        land.iter().map(wire_event).collect::<Vec<_>>()
    );
}

#[test]
fn a_key_combo_goes_out_whole_to_the_keyboard() {
    let source = FakeSource::new();
    let back = KeyCombo::alt(Key::LEFT);
    let mut e = emitter(&source, ForeignOutput::Off, std::slice::from_ref(&back));
    e.key(&back).unwrap();
    let state = source.state();
    let want: Vec<Event> = key_events(&back).iter().map(wire_event).collect();
    assert_eq!(state.sent.len(), 1);
    assert_eq!(state.sent[0].1, want);
}

#[test]
fn a_device_that_was_not_made_takes_no_frames() {
    let source = FakeSource::new();
    let mut e = emitter(&source, ForeignOutput::Off, &[]);
    e.pad(&LAND).unwrap();
    e.wheel(&scroll_one()).unwrap();
    e.key(&KeyCombo::alt(Key::LEFT)).unwrap();
    assert!(source.state().sent.is_empty());
}

#[test]
fn frames_the_service_has_no_room_for_are_counted_and_the_pad_goes_on() {
    let source = FakeSource::new();
    let mut e = emitter(&source, ForeignOutput::Wheel, &[]);
    source.state().send_mode = SendMode::Drop;
    for _ in 0..3 {
        assert!(
            e.wheel(&scroll_one()).is_ok(),
            "a lost frame is not an error"
        );
    }
    assert_eq!(e.lost, FramesLost(3));
    source.state().send_mode = SendMode::Accept;
    e.wheel(&scroll_one()).unwrap();
    assert_eq!(source.state().sent.len(), 1, "the next frame goes through");
    assert_eq!(e.lost, FramesLost(3));
}

#[test]
fn a_closed_connection_is_a_cut() {
    let source = FakeSource::new();
    let mut e = emitter(&source, ForeignOutput::Wheel, &[]);
    source.state().send_mode = SendMode::Close;
    let err = e.wheel(&scroll_one()).unwrap_err();
    assert!(matches!(err, OutputError::Cut(_)), "{err}");
}

#[test]
fn a_refusal_is_a_cut_that_says_why_in_plain_words() {
    let cases = [
        (Reason::NotAllowed, "list of programs"),
        (Reason::TooMany, "as many input devices"),
        (Reason::BadCaps, "not one it may add"),
        (Reason::Unavailable, "would not make"),
        (Reason::Unknown, "no reason"),
    ];
    for (reason, words) in cases {
        let source = FakeSource::new();
        source.state().refuse_next = Some(reason);
        let Err(err) = UinputEmitter::create(&source, ForeignOutput::Touchpad, &[]) else {
            panic!("{reason:?} should refuse");
        };
        let message = match err {
            OutputError::Cut(message) => message,
            other => panic!("{reason:?}: a refusal is a cut, not {other}"),
        };
        assert!(message.contains(words), "{reason:?}: {message}");
        let shown = message.to_lowercase();
        for developer_word in ["uinput", "socket", "peephole", "ceiling"] {
            assert!(!shown.contains(developer_word), "{message}");
        }
    }
}

#[test]
fn no_service_is_not_a_cut() {
    let source = FakeSource::new();
    source.state().open_mode = OpenMode::Absent;
    let Err(err) = UinputEmitter::create(&source, ForeignOutput::Touchpad, &[]) else {
        panic!("an absent service makes no devices");
    };
    assert!(matches!(err, OutputError::Unavailable(_)), "{err}");
}
