//! The real emitter: uinput devices for O2 (touchpad), O3 (keys) and O4 (wheel).
//!
//! Compiled and reviewed but deliberately unexercised by the test suite: every test uses
//! `RecordingEmitter`. The event sequences it writes are the pure `events` functions, which
//! are tested; what is untested here is only device creation (acceptance 8-11, manual).

use super::sys::{self, AbsInfo, BitSet, InputId, UinputAbsSetup, UinputSetup};
use crate::config::ForeignOutput;
use crate::feel::WheelDelta;
use crate::keys::KeyCombo;
use crate::outputs::events::{
    ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_SLOT, ABS_MT_TRACKING_ID, ABS_X, ABS_Y, BTN_LEFT,
    BTN_TOOL_DOUBLETAP, BTN_TOOL_FINGER, BTN_TOOL_QUADTAP, BTN_TOOL_TRIPLETAP, BTN_TOUCH, EV_ABS,
    EV_KEY, EV_REL, EV_SYN, InputEvent, PAD_SLOTS, PadEncoder, REL_HWHEEL, REL_HWHEEL_HI_RES,
    REL_WHEEL, REL_WHEEL_HI_RES, key_events, wheel_events,
};
use crate::outputs::pad::{PAD_X_MAX, PAD_Y_MAX, PadFrame, UNITS_PER_MM};
use crate::outputs::{Emitter, OutputError};
use rustix::fd::OwnedFd;
use rustix::fs::{Mode, OFlags};
use std::io;
use std::path::Path;

/// The uinput control node.
pub const UINPUT: &str = "/dev/uinput";
/// `BUS_VIRTUAL`.
pub const BUS_VIRTUAL: u16 = 0x06;
/// `INPUT_PROP_POINTER`: a touchpad, not a touchscreen.
pub const INPUT_PROP_POINTER: u16 = 0x00;
/// The environment variable that must be `1` before `--check-uinput` opens anything.
pub const PROBE_PERMIT: &str = "PALMREST_ALLOW_UINPUT_PROBE";

/// One identity a virtual device is created under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
}

/// O2's identity.
pub const TOUCHPAD: Identity = Identity {
    name: "palmrest touchpad",
    vendor: 0x0001,
    product: 0x0002,
};
/// O4's identity.
pub const WHEEL: Identity = Identity {
    name: "palmrest wheel",
    vendor: 0x0001,
    product: 0x0001,
};
/// O3's identity.
pub const KEYS: Identity = Identity {
    name: "palmrest keys",
    vendor: 0x0001,
    product: 0x0003,
};

/// A created uinput device; destroyed on drop.
#[derive(Debug)]
struct Device {
    fd: OwnedFd,
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = sys::dev_destroy(&self.fd);
    }
}

impl Device {
    fn write(&self, events: &[InputEvent]) -> io::Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let bytes: Vec<u8> = events.iter().flat_map(|e| e.to_bytes()).collect();
        let n = rustix::io::write(&self.fd, &bytes)?;
        if n == bytes.len() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "short uinput write",
            ))
        }
    }
}

/// The capabilities one device is created with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Caps {
    keys: Vec<u16>,
    rels: Vec<u16>,
    abs: Vec<(u16, AbsRange)>,
    props: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AbsRange {
    max: i32,
    resolution: i32,
}

fn touchpad_caps() -> Caps {
    let x = AbsRange {
        max: PAD_X_MAX,
        resolution: UNITS_PER_MM,
    };
    let y = AbsRange {
        max: PAD_Y_MAX,
        resolution: UNITS_PER_MM,
    };
    Caps {
        keys: vec![
            BTN_LEFT,
            BTN_TOUCH,
            BTN_TOOL_FINGER,
            BTN_TOOL_DOUBLETAP,
            BTN_TOOL_TRIPLETAP,
            BTN_TOOL_QUADTAP,
        ],
        rels: vec![],
        abs: vec![
            (ABS_X, x),
            (ABS_Y, y),
            (
                ABS_MT_SLOT,
                AbsRange {
                    max: i32::from(PAD_SLOTS) - 1,
                    resolution: 0,
                },
            ),
            (
                ABS_MT_TRACKING_ID,
                AbsRange {
                    max: 0xffff,
                    resolution: 0,
                },
            ),
            (ABS_MT_POSITION_X, x),
            (ABS_MT_POSITION_Y, y),
        ],
        props: vec![INPUT_PROP_POINTER],
    }
}

fn wheel_caps() -> Caps {
    Caps {
        rels: vec![REL_WHEEL, REL_HWHEEL, REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES],
        ..Caps::default()
    }
}

fn key_caps(combos: &[KeyCombo]) -> Caps {
    let mut keys: Vec<u16> = combos
        .iter()
        .flat_map(|k| k.codes().collect::<Vec<_>>())
        .collect();
    keys.sort_unstable();
    keys.dedup();
    Caps {
        keys,
        ..Caps::default()
    }
}

fn create(id: Identity, caps: &Caps) -> io::Result<Device> {
    let fd = rustix::fs::open(
        UINPUT,
        OFlags::WRONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let kinds = [
        (EV_KEY, !caps.keys.is_empty()),
        (EV_REL, !caps.rels.is_empty()),
        (EV_ABS, !caps.abs.is_empty()),
    ];
    sys::set_bit(&fd, BitSet::Ev, EV_SYN)?;
    for (kind, used) in kinds {
        if used {
            sys::set_bit(&fd, BitSet::Ev, kind)?;
        }
    }
    for &k in &caps.keys {
        sys::set_bit(&fd, BitSet::Key, k)?;
    }
    for &r in &caps.rels {
        sys::set_bit(&fd, BitSet::Rel, r)?;
    }
    for &(code, range) in &caps.abs {
        sys::set_bit(&fd, BitSet::Abs, code)?;
        let absinfo = AbsInfo {
            maximum: range.max,
            resolution: range.resolution,
            ..AbsInfo::default()
        };
        sys::abs_setup(&fd, UinputAbsSetup { code, absinfo })?;
    }
    for &p in &caps.props {
        sys::set_bit(&fd, BitSet::Prop, p)?;
    }
    let mut name = [0u8; 80];
    name[..id.name.len()].copy_from_slice(id.name.as_bytes());
    let setup = UinputSetup {
        id: InputId {
            bustype: BUS_VIRTUAL,
            vendor: id.vendor,
            product: id.product,
            version: 1,
        },
        name,
        ff_effects_max: 0,
    };
    sys::dev_setup(&fd, setup)?;
    sys::dev_create(&fd)?;
    Ok(Device { fd })
}

/// The devices of one daemon run.
#[derive(Debug)]
pub struct UinputEmitter {
    touchpad: Option<Device>,
    wheel: Option<Device>,
    keys: Option<Device>,
    encoder: PadEncoder,
}

impl UinputEmitter {
    /// Creates the devices `foreign` needs, plus a keyboard for `combos` (none if empty).
    pub fn create(foreign: ForeignOutput, combos: &[KeyCombo]) -> Result<Self, OutputError> {
        let unavailable = |e: io::Error| OutputError::Unavailable(e.to_string());
        let touchpad = match foreign {
            ForeignOutput::Touchpad => {
                Some(create(TOUCHPAD, &touchpad_caps()).map_err(unavailable)?)
            }
            _ => None,
        };
        let wheel = match foreign {
            ForeignOutput::Wheel => Some(create(WHEEL, &wheel_caps()).map_err(unavailable)?),
            _ => None,
        };
        let keys = if combos.is_empty() {
            None
        } else {
            Some(create(KEYS, &key_caps(combos)).map_err(unavailable)?)
        };
        Ok(Self {
            touchpad,
            wheel,
            keys,
            encoder: PadEncoder::default(),
        })
    }
}

impl Emitter for UinputEmitter {
    fn pad(&mut self, f: &PadFrame) -> Result<(), OutputError> {
        let (encoder, events) = self.encoder.encode(f);
        self.encoder = encoder;
        match &self.touchpad {
            Some(d) => Ok(d.write(&events)?),
            None => Ok(()),
        }
    }

    fn wheel(&mut self, d: &WheelDelta) -> Result<(), OutputError> {
        match &self.wheel {
            Some(dev) => Ok(dev.write(&wheel_events(d))?),
            None => Ok(()),
        }
    }

    fn key(&mut self, k: &KeyCombo) -> Result<(), OutputError> {
        match &self.keys {
            Some(dev) => Ok(dev.write(&key_events(k))?),
            None => Ok(()),
        }
    }
}

/// Whether `--check-uinput` may open the node: only when the permit variable is exactly `1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbePermit {
    Granted,
    Refused,
}

impl ProbePermit {
    pub fn from_env_value(v: Option<&str>) -> Self {
        if v == Some("1") {
            Self::Granted
        } else {
            Self::Refused
        }
    }
}

/// Opens and closes the uinput node without creating anything: can this user make devices?
pub fn probe(permit: ProbePermit, node: &Path) -> Result<(), OutputError> {
    match permit {
        ProbePermit::Refused => Err(OutputError::Unavailable(format!(
            "set {PROBE_PERMIT}=1 to let --check-uinput open {UINPUT}"
        ))),
        ProbePermit::Granted => {
            let fd = rustix::fs::open(
                node,
                OFlags::WRONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| OutputError::Unavailable(format!("{}: {e}", node.display())))?;
            drop(fd);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Key;

    #[test]
    fn probe_is_refused_without_the_permit() {
        assert_eq!(ProbePermit::from_env_value(None), ProbePermit::Refused);
        assert_eq!(
            ProbePermit::from_env_value(Some("yes")),
            ProbePermit::Refused
        );
        assert_eq!(ProbePermit::from_env_value(Some("1")), ProbePermit::Granted);
        // Refused never touches the path, so a node that would work is not opened.
        assert!(probe(ProbePermit::Refused, Path::new(UINPUT)).is_err());
    }

    #[test]
    fn capabilities_are_minimal() {
        let pad = touchpad_caps();
        assert!(pad.rels.is_empty(), "the touchpad cannot scroll by wheel");
        assert_eq!(pad.props, [INPUT_PROP_POINTER]);
        let wheel = wheel_caps();
        assert!(
            wheel.keys.is_empty() && wheel.abs.is_empty(),
            "the wheel cannot click or point"
        );
        let keys = key_caps(&[KeyCombo::alt(Key::LEFT), KeyCombo::alt(Key::RIGHT)]);
        assert_eq!(keys.keys, [56, 105, 106]);
    }
}
