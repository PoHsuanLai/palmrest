//! The devices against recording emitters and the fake input service.

use super::*;
use crate::daemon::uinput::UinputEmitter;
use crate::outputs::RecordingEmitter;
use std::cell::Cell;
use std::rc::Rc;
use turnstile_client::Reason;
use turnstile_client::fake::{FakeSource, SendMode};

#[test]
fn waits_then_becomes_ready_and_rebuilds_on_a_shape_change() {
    let calls = Rc::new(Cell::new(0));
    let c2 = calls.clone();
    let factory: Factory = Box::new(move |_| {
        c2.set(c2.get() + 1);
        if c2.get() < 2 {
            Err(OutputError::Unavailable("no acl yet".into()))
        } else {
            Ok(Box::new(RecordingEmitter::default()))
        }
    });
    let cfg = Config::default();
    let d = Devices::new(factory, &cfg, Mono(0));
    assert_eq!(calls.get(), 1);
    let mut d = d.poll(Mono::from_millis(500));
    assert_eq!(calls.get(), 1, "not before the backoff");
    assert!(d.pad(&land()).is_ok(), "frames are dropped while waiting");
    let d = d.poll(Mono::from_millis(1000));
    assert_eq!(calls.get(), 2);
    let d = d.reconfigure(&cfg, Mono::from_millis(1100));
    assert_eq!(calls.get(), 2, "same shape, same devices");
    let _ = d.reconfigure(&other_gestures(), Mono::from_millis(1200));
    assert_eq!(calls.get(), 3);
}

fn land() -> PadFrame {
    PadFrame::Land {
        fingers: 2,
        at: crate::outputs::pad::CENTRE,
        spacing: 400,
    }
}

/// A configuration whose gestures press other keys than the default's.
fn other_gestures() -> Config {
    let mut c = Config::default();
    c.gestures.gesture_action_map = c.gestures.gesture_action_map.with(
        crate::action::Gesture::OneFingerSwipeLeft,
        crate::action::Action::None,
    );
    c
}

/// An emitter whose every write fails, as a destroyed uinput device does.
struct Destroyed;

impl Emitter for Destroyed {
    fn pad(&mut self, _: &PadFrame) -> Result<(), OutputError> {
        Err(std::io::Error::from(std::io::ErrorKind::NotFound).into())
    }
    fn key(&mut self, _: &KeyCombo) -> Result<(), OutputError> {
        Err(std::io::Error::from(std::io::ErrorKind::NotFound).into())
    }
}

#[test]
fn a_failed_write_rebuilds_the_devices_and_reports_the_loss_once() {
    let built = Rc::new(Cell::new(0));
    let b = built.clone();
    let factory: Factory = Box::new(move |_| {
        b.set(b.get() + 1);
        if b.get() == 1 {
            Ok(Box::new(Destroyed))
        } else {
            Ok(Box::new(RecordingEmitter::default()))
        }
    });
    let mut d = Devices::new(factory, &Config::default(), Mono(0));
    assert_eq!(built.get(), 1);
    assert!(!d.take_lost());

    assert!(d.pad(&land()).is_err(), "the failure is returned once");
    assert!(d.take_lost());
    assert!(!d.take_lost(), "asking resets it");
    assert!(
        d.pad(&land()).is_ok(),
        "frames while rebuilding are dropped without another error"
    );

    let d = d.poll(Mono::from_millis(10));
    assert_eq!(built.get(), 1, "not before the backoff");
    let mut d = d.poll(Mono::from_millis(1010));
    assert_eq!(built.get(), 2, "rebuilt once the delay has passed");
    assert!(d.pad(&land()).is_ok());
    assert!(!d.take_lost());
}
/// Devices whose emitter talks to `source`, as the daemon's do to the real service.
fn service_devices(source: &FakeSource, c: &Config) -> Devices {
    let source = source.clone();
    let factory: Factory = Box::new(move |combos| {
        Ok(Box::new(UinputEmitter::create(&source, combos)?) as Box<dyn Emitter>)
    });
    Devices::new(factory, c, Mono(0))
}

/// Lets the wait after a loss pass and the rebuild happen, `round` * 10 s into the run.
fn settle(d: Devices, round: u64) -> Devices {
    let at = Mono::from_millis(round * 10_000);
    d.poll(at).poll(at.after(std::time::Duration::from_secs(2)))
}

#[test]
fn a_closed_connection_is_rebuilt_after_the_pause() {
    let source = FakeSource::new();
    let mut d = service_devices(&source, &Config::default());
    assert_eq!(source.state().opened, 1);

    source.state().send_mode = SendMode::Close;
    assert!(d.pad(&land()).is_err());
    assert!(d.take_lost());

    source.state().send_mode = SendMode::Accept;
    let d = d.poll(Mono::from_millis(10));
    assert_eq!(source.state().opened, 1, "not before the pause");
    let mut d = d.poll(Mono::from_millis(1010));
    assert_eq!(source.state().opened, 2, "a new connection");
    assert!(d.pad(&land()).is_ok());
    assert_eq!(source.state().sent.len(), 1, "and frames flow again");
}

#[test]
fn frames_the_service_drops_cost_no_rebuild() {
    let source = FakeSource::new();
    let mut d = service_devices(&source, &Config::default());
    source.state().send_mode = SendMode::Drop;
    assert!(d.pad(&land()).is_ok());
    assert!(!d.take_lost());
    assert_eq!(source.state().opened, 1);
}

#[test]
fn a_refusal_ends_the_devices_after_three_more_tries_until_the_settings_change() {
    let source = FakeSource::new();
    source.state().refuse_next = Some(Reason::NotAllowed);
    let mut d = service_devices(&source, &Config::default());
    for round in 1..=8 {
        source.state().refuse_next = Some(Reason::NotAllowed);
        d = d.poll(Mono::from_millis(round * 10_000));
    }
    assert_eq!(
        source.state().created.len(),
        4,
        "the first try and three more, then no more"
    );
    assert!(d.pad(&land()).is_ok(), "a dead pad drops frames quietly");
    assert!(source.state().sent.is_empty());

    source.state().refuse_next = None;
    let mut d = d.reconfigure(&other_gestures(), Mono::from_millis(100_000));
    assert_eq!(
        source.state().created.len(),
        6,
        "a change of settings tries again: the touchpad and the keys of the default gestures"
    );
    assert!(d.pad(&land()).is_ok());
    assert_eq!(source.state().sent.len(), 1);
}

#[test]
fn a_service_that_keeps_closing_ends_the_devices() {
    let source = FakeSource::new();
    let mut d = service_devices(&source, &Config::default());
    source.state().send_mode = SendMode::Close;
    for round in 1..=8 {
        let _ = d.pad(&land());
        d = settle(d, round);
    }
    assert_eq!(
        source.state().opened,
        4,
        "the first connection and three more"
    );
}

#[test]
fn a_frame_that_goes_through_clears_the_count() {
    let source = FakeSource::new();
    let mut d = service_devices(&source, &Config::default());
    for round in 1..=6 {
        source.state().send_mode = SendMode::Accept;
        assert!(d.pad(&land()).is_ok());
        source.state().send_mode = SendMode::Close;
        assert!(d.pad(&land()).is_err());
        d = settle(d, round);
    }
    assert_eq!(
        source.state().opened,
        7,
        "each loss is rebuilt, none gives up"
    );
}
