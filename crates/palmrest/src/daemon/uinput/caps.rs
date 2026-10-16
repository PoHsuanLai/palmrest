//! What each of palmrest's three virtual devices is: its identity and its capabilities.
//!
//! The input service builds the same three from constants of its own, so a change here is a
//! change there. Only the direct path (`direct.rs`) uses the capabilities; the service path
//! sends just which device it wants.

use crate::outputs::events::{
    ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_SLOT, ABS_MT_TRACKING_ID, ABS_X, ABS_Y, BTN_LEFT,
    BTN_TOOL_DOUBLETAP, BTN_TOOL_FINGER, BTN_TOOL_QUADTAP, BTN_TOOL_TRIPLETAP, BTN_TOUCH,
    PAD_SLOTS, REL_HWHEEL, REL_HWHEEL_HI_RES, REL_WHEEL, REL_WHEEL_HI_RES,
};
use crate::outputs::pad::{PAD_X_MAX, PAD_Y_MAX, UNITS_PER_MM};
use turnstile_client::{Create, Detail, Kind};

/// `INPUT_PROP_POINTER`: a touchpad, not a touchscreen.
pub(super) const INPUT_PROP_POINTER: u16 = 0x00;

/// One identity a virtual device is created under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Identity {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
}

/// O2's identity.
pub(super) const TOUCHPAD: Identity = Identity {
    name: "palmrest touchpad",
    vendor: 0x0001,
    product: 0x0002,
};
/// O4's identity.
pub(super) const WHEEL: Identity = Identity {
    name: "palmrest wheel",
    vendor: 0x0001,
    product: 0x0001,
};
/// O3's identity.
pub(super) const KEYS: Identity = Identity {
    name: "palmrest keys",
    vendor: 0x0001,
    product: 0x0003,
};

/// The capabilities one device is created with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Caps {
    pub keys: Vec<u16>,
    pub rels: Vec<u16>,
    pub abs: Vec<(u16, AbsRange)>,
    pub props: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AbsRange {
    pub max: i32,
    pub resolution: i32,
}

pub(super) fn touchpad_caps() -> Caps {
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

pub(super) fn wheel_caps() -> Caps {
    Caps {
        rels: vec![REL_WHEEL, REL_HWHEEL, REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES],
        ..Caps::default()
    }
}

/// The identity and capabilities `request` asks for; none for a device that is not palmrest's.
pub(super) fn wanted(request: &Create) -> Option<(Identity, Caps)> {
    match (request.kind, &request.detail) {
        (Kind::PalmrestTouchpad, _) => Some((TOUCHPAD, touchpad_caps())),
        (Kind::PalmrestWheel, _) => Some((WHEEL, wheel_caps())),
        (Kind::PalmrestKeys, Detail::Keys(codes)) => Some((
            KEYS,
            Caps {
                keys: codes.clone(),
                ..Caps::default()
            },
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn only_palmrests_own_devices_are_wanted() {
        let keys = wanted(&Create::keys(vec![56, 105])).map(|(id, caps)| (id.name, caps.keys));
        assert_eq!(keys, Some(("palmrest keys", vec![56, 105])));
        assert_eq!(
            wanted(&Create::touchpad()).map(|(id, _)| id.name),
            Some("palmrest touchpad")
        );
        assert_eq!(
            wanted(&Create::wheel()).map(|(id, _)| id.name),
            Some("palmrest wheel")
        );
        assert!(
            wanted(&Create::keyboard()).is_none(),
            "keycap's keyboard is not ours"
        );
    }
}
