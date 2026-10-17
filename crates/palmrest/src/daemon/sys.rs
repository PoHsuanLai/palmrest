//! The evdev and uinput ioctls, through `input-linux`'s safe handles.
//!
//! No `unsafe` lives here: the crate owns the opcodes and the argument layouts, so this module
//! only turns palmrest's plain values (raw `u16` codes) into its typed ones. A handle borrows
//! the descriptor for one call; the caller keeps ownership, and closing it still destroys a
//! device. The one hidraw ioctl no safe crate has is in `hidioc`.

use input_linux::{
    AbsoluteAxis, AbsoluteInfo, AbsoluteInfoSetup, EvdevHandle, EventKind, InputProperty, Key,
    RelativeAxis, UInputHandle,
};
use rustix::fd::{AsFd, BorrowedFd};
use std::io;

/// Bytes of a uinput device name, the terminating NUL included.
const NAME_BYTES: usize = 80;
/// `CLOCK_MONOTONIC`.
const CLOCK_MONOTONIC: i32 = 1;

/// An evdev handle over a descriptor the caller keeps.
fn evdev(fd: &impl AsFd) -> EvdevHandle<BorrowedFd<'_>> {
    EvdevHandle::new(fd.as_fd())
}

/// A uinput handle over a descriptor the caller keeps.
fn uinput(fd: &impl AsFd) -> UInputHandle<BorrowedFd<'_>> {
    UInputHandle::new(fd.as_fd())
}

/// The error for a raw code that is not in the kernel's range for its kind.
fn out_of_range<E>(_: E) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "code out of range")
}

/// An evdev axis palmrest reads the geometry of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtAxis {
    Slot,
    PositionX,
    PositionY,
}

impl MtAxis {
    fn typed(self) -> AbsoluteAxis {
        match self {
            MtAxis::Slot => AbsoluteAxis::MultitouchSlot,
            MtAxis::PositionX => AbsoluteAxis::MultitouchPositionX,
            MtAxis::PositionY => AbsoluteAxis::MultitouchPositionY,
        }
    }
}

/// Reads one multitouch axis's range and resolution from an evdev fd (`EVIOCGABS`). Read-only:
/// it changes nothing on the device.
pub fn abs_info(fd: impl AsFd, axis: MtAxis) -> io::Result<AbsInfo> {
    evdev(&fd).absolute_info(axis.typed()).map(AbsInfo::from)
}

/// Stamps this fd's events on `CLOCK_MONOTONIC` instead of the wall clock (`EVIOCSCLOCKID`).
/// Per open file: no other reader of the device is affected.
pub fn monotonic_events(fd: impl AsFd) -> io::Result<()> {
    evdev(&fd).set_clock_id(CLOCK_MONOTONIC)
}

/// `struct input_id`.
#[derive(Debug, Clone, Copy)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

impl From<InputId> for input_linux::InputId {
    fn from(id: InputId) -> Self {
        Self {
            bustype: id.bustype,
            vendor: id.vendor,
            product: id.product,
            version: id.version,
        }
    }
}

/// `struct uinput_setup`: the identity a virtual device is created under. The name is NUL
/// padded; it ends at the first NUL.
#[derive(Debug, Clone, Copy)]
pub struct UinputSetup {
    pub id: InputId,
    pub name: [u8; NAME_BYTES],
    pub ff_effects_max: u32,
}

impl UinputSetup {
    /// The name without its padding.
    fn name_bytes(&self) -> &[u8] {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(NAME_BYTES);
        &self.name[..end]
    }
}

/// `struct input_absinfo`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

impl From<AbsoluteInfo> for AbsInfo {
    fn from(info: AbsoluteInfo) -> Self {
        Self {
            value: info.value,
            minimum: info.minimum,
            maximum: info.maximum,
            fuzz: info.fuzz,
            flat: info.flat,
            resolution: info.resolution,
        }
    }
}

impl From<AbsInfo> for AbsoluteInfo {
    fn from(info: AbsInfo) -> Self {
        Self {
            value: info.value,
            minimum: info.minimum,
            maximum: info.maximum,
            fuzz: info.fuzz,
            flat: info.flat,
            resolution: info.resolution,
        }
    }
}

/// `struct uinput_abs_setup`: one absolute axis and its range.
#[derive(Debug, Clone, Copy)]
pub struct UinputAbsSetup {
    pub code: u16,
    pub absinfo: AbsInfo,
}

impl UinputAbsSetup {
    fn typed(self) -> io::Result<AbsoluteInfoSetup> {
        Ok(AbsoluteInfoSetup {
            axis: AbsoluteAxis::from_code(self.code).map_err(out_of_range)?,
            info: self.absinfo.into(),
        })
    }
}

/// Which capability bit set a `UI_SET_*BIT` call writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitSet {
    Ev,
    Key,
    Rel,
    Abs,
    Prop,
}

/// Enables one capability bit on a uinput fd before the device is created.
pub fn set_bit(fd: impl AsFd, set: BitSet, code: u16) -> io::Result<()> {
    let handle = uinput(&fd);
    match set {
        BitSet::Ev => handle.set_evbit(EventKind::try_from(code).map_err(out_of_range)?),
        BitSet::Key => handle.set_keybit(Key::from_code(code).map_err(out_of_range)?),
        BitSet::Rel => handle.set_relbit(RelativeAxis::from_code(code).map_err(out_of_range)?),
        BitSet::Abs => handle.set_absbit(AbsoluteAxis::from_code(code).map_err(out_of_range)?),
        BitSet::Prop => handle.set_propbit(InputProperty::from_code(code).map_err(out_of_range)?),
    }
}

/// Declares the device's identity and absolute axes, then creates it: `UI_DEV_SETUP`, one
/// `UI_ABS_SETUP` per axis, `UI_DEV_CREATE`. The capability bits are already set. Never called
/// by any test.
pub fn dev_create(fd: impl AsFd, setup: UinputSetup, abs: &[UinputAbsSetup]) -> io::Result<()> {
    let abs = abs
        .iter()
        .map(|axis| axis.typed())
        .collect::<io::Result<Vec<_>>>()?;
    uinput(&fd).create(
        &setup.id.into(),
        setup.name_bytes(),
        setup.ff_effects_max,
        &abs,
    )
}

/// Destroys the device.
pub fn dev_destroy(fd: impl AsFd) -> io::Result<()> {
    uinput(&fd).dev_destroy()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_ends_at_its_first_nul() {
        let mut name = [0u8; NAME_BYTES];
        name[..4].copy_from_slice(b"palm");
        let setup = UinputSetup {
            id: InputId {
                bustype: 6,
                vendor: 1,
                product: 2,
                version: 1,
            },
            name,
            ff_effects_max: 0,
        };
        assert_eq!(setup.name_bytes(), b"palm");
    }

    #[test]
    fn an_axis_past_the_kernels_range_is_refused_before_any_ioctl() {
        let err = UinputAbsSetup {
            code: 0x40,
            absinfo: AbsInfo::default(),
        }
        .typed()
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn the_multitouch_axes_carry_the_kernels_codes() {
        assert_eq!(MtAxis::Slot.typed().code(), 0x2f);
        assert_eq!(MtAxis::PositionX.typed().code(), 0x35);
        assert_eq!(MtAxis::PositionY.typed().code(), 0x36);
    }
}
