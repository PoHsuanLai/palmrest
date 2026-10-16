//! The real emitter: palmrest's virtual touchpad, wheel and keyboard, through a `Session`.
//!
//! A pad frame is one call to `Session::send`, so one batch. What goes wrong is told to the
//! owner of the emitter (`Devices`) through `OutputError`, which decides when to rebuild and
//! when to stop: a lost frame here is counted and nothing more.

use crate::config::ForeignOutput;
use crate::feel::WheelDelta;
use crate::keys::KeyCombo;
use crate::outputs::events::{InputEvent, PadEncoder, key_events, wheel_events};
use crate::outputs::pad::PadFrame;
use crate::outputs::{Emitter, OutputError};
use turnstile_client::{
    ClientError, Create, DeviceId, DeviceSource, Event, Made, Reason, Sent, Session,
};

#[cfg(test)]
mod tests;

/// Frames the service had no room for, since the emitter was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FramesLost(u64);

impl FramesLost {
    fn one_more(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// The 1st, 2nd, 4th, 8th... loss is worth a line in the log; the rest would only fill it.
    fn worth_saying(self) -> bool {
        self.0.is_power_of_two()
    }
}

/// The devices of one daemon run.
pub struct UinputEmitter<S: Session> {
    session: S,
    touchpad: Option<DeviceId>,
    wheel: Option<DeviceId>,
    keys: Option<DeviceId>,
    encoder: PadEncoder,
    lost: FramesLost,
}

impl<S: Session> UinputEmitter<S> {
    /// Opens a session on `source` and makes the devices `foreign` needs, plus a keyboard for
    /// `combos` (none if empty).
    pub fn create<D: DeviceSource<Session = S>>(
        source: &D,
        foreign: ForeignOutput,
        combos: &[KeyCombo],
    ) -> Result<Self, OutputError> {
        let mut session = source.open().map_err(|e| match e {
            ClientError::Io(io) => OutputError::Unavailable(format!("{UNREACHABLE}: {io}")),
            other => OutputError::Unavailable(other.to_string()),
        })?;
        let touchpad = match foreign {
            ForeignOutput::Touchpad => Some(make(&mut session, &Create::touchpad())?),
            _ => None,
        };
        let wheel = match foreign {
            ForeignOutput::Wheel => Some(make(&mut session, &Create::wheel())?),
            _ => None,
        };
        let keys = match key_codes(combos) {
            codes if codes.is_empty() => None,
            codes => Some(make(&mut session, &Create::keys(codes))?),
        };
        Ok(Self {
            session,
            touchpad,
            wheel,
            keys,
            encoder: PadEncoder::default(),
            lost: FramesLost::default(),
        })
    }

    /// Sends one frame's events to `device` (if the device exists) in one batch.
    fn send(&mut self, device: Option<DeviceId>, events: &[InputEvent]) -> Result<(), OutputError> {
        let Some(device) = device else {
            return Ok(());
        };
        if events.is_empty() {
            return Ok(());
        }
        let batch: Vec<Event> = events.iter().map(wire_event).collect();
        match self.session.send(device, &batch) {
            Ok(Sent::Queued) => Ok(()),
            Ok(Sent::Dropped) => {
                self.lost = self.lost.one_more();
                if self.lost.worth_saying() {
                    eprintln!(
                        "palmrest: the input service is behind; frames lost so far: {}",
                        self.lost.0
                    );
                }
                Ok(())
            }
            Err(e) => Err(explain(&mut self.session, e)),
        }
    }
}

impl<S: Session> Emitter for UinputEmitter<S> {
    fn pad(&mut self, f: &PadFrame) -> Result<(), OutputError> {
        let (encoder, events) = self.encoder.encode(f);
        self.encoder = encoder;
        self.send(self.touchpad, &events)
    }

    fn wheel(&mut self, d: &WheelDelta) -> Result<(), OutputError> {
        self.send(self.wheel, &wheel_events(d))
    }

    fn key(&mut self, k: &KeyCombo) -> Result<(), OutputError> {
        self.send(self.keys, &key_events(k))
    }
}

/// Every key the combos press, once each and in code order.
fn key_codes(combos: &[KeyCombo]) -> Vec<u16> {
    let mut codes: Vec<u16> = combos.iter().flat_map(|k| k.codes()).collect();
    codes.sort_unstable();
    codes.dedup();
    codes
}

fn wire_event(e: &InputEvent) -> Event {
    Event::new(e.kind, e.code, e.value)
}

/// What a person reads when the way to the virtual devices cannot be opened at all.
const UNREACHABLE: &str = "the virtual devices cannot be reached";

/// Asks for one device.
fn make<S: Session>(session: &mut S, request: &Create) -> Result<DeviceId, OutputError> {
    match session.create(request) {
        Ok(Made::Proxied(id)) => Ok(id),
        Err(ClientError::Io(io)) => Err(OutputError::Unavailable(format!("{UNREACHABLE}: {io}"))),
        Ok(Made::Handed { .. }) => Err(OutputError::Cut(
            "the input service sent a device palmrest did not ask for".to_owned(),
        )),
        Err(e) => Err(explain(session, e)),
    }
}

/// What a failed request means for palmrest, in words a person can read in the log. A service
/// that turned palmrest away says why; that reason is asked for here, while the session that
/// heard it is still at hand.
fn explain<S: Session>(session: &mut S, e: ClientError) -> OutputError {
    match e {
        ClientError::Absent | ClientError::Busy => OutputError::Unavailable(e.to_string()),
        ClientError::Io(io) => OutputError::Write(io),
        ClientError::Refused(reason) => OutputError::Cut(turned_away(reason)),
        ClientError::Closed | ClientError::Timeout => {
            OutputError::Cut(session.refusal().map_or_else(|| e.to_string(), turned_away))
        }
        other => OutputError::Cut(other.to_string()),
    }
}

/// The service's reason for refusing, in plain words.
fn turned_away(reason: Reason) -> String {
    let why = match reason {
        Reason::NotAllowed => {
            "this program is not on the list of programs that may add input devices"
        }
        Reason::TooMany => "it already has as many input devices as it may have",
        Reason::BadCaps => "the device it asked for is not one it may add",
        Reason::Unavailable => "the system would not make the device",
        _ => "it gave no reason",
    };
    format!("the input service turned palmrest away: {why}")
}
