//! The ioctls: the one module allowed `unsafe` (12.3.12).
//!
//! Every call goes through rustix's typed ioctl patterns, so the only unsafety left is the
//! promise that the opcode and the argument type match the kernel's declaration. Each block
//! states which declaration it matches.
#![allow(unsafe_code)]

use rustix::fd::AsFd;
use rustix::ioctl::{self, Getter, IntegerSetter, NoArg, Opcode, Setter, Updater, opcode};
use std::io;

/// `HIDIOCSFEATURE(3)`: send a 3-byte feature report.
const HIDIOCSFEATURE_3: Opcode = opcode::read_write::<[u8; 3]>(b'H', 0x06);

/// Sends a 3-byte feature report (`F1 02 01` enables multitouch reports).
pub fn send_feature(fd: impl AsFd, report: [u8; 3]) -> io::Result<()> {
    let mut buf = report;
    // SAFETY: linux/hidraw.h declares HIDIOCSFEATURE(len) as _IOC(_IOC_WRITE|_IOC_READ, 'H',
    // 0x06, len) taking a `char[len]` buffer; the opcode is built for exactly `[u8; 3]` and the
    // buffer is that array, borrowed mutably for the call.
    unsafe { ioctl::ioctl(fd, Updater::<HIDIOCSFEATURE_3, [u8; 3]>::new(&mut buf)) }
        .map_err(io::Error::from)
}

/// `EVIOCGABS(ABS_MT_SLOT)`, `EVIOCGABS(ABS_MT_POSITION_X)`, `EVIOCGABS(ABS_MT_POSITION_Y)`:
/// read one absolute axis's `struct input_absinfo`.
const EVIOCGABS_MT_SLOT: Opcode = opcode::read::<AbsInfo>(b'E', 0x40 + 0x2f);
const EVIOCGABS_MT_X: Opcode = opcode::read::<AbsInfo>(b'E', 0x40 + 0x35);
const EVIOCGABS_MT_Y: Opcode = opcode::read::<AbsInfo>(b'E', 0x40 + 0x36);
/// `EVIOCSCLOCKID`: which clock this fd's event timestamps use.
const EVIOCSCLOCKID: Opcode = opcode::write::<i32>(b'E', 0xa0);
/// `CLOCK_MONOTONIC`.
const CLOCK_MONOTONIC: i32 = 1;

/// An evdev axis palmrest reads the geometry of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtAxis {
    Slot,
    PositionX,
    PositionY,
}

/// Reads one multitouch axis's range and resolution from an evdev fd. Read-only: it changes
/// nothing on the device.
pub fn abs_info(fd: impl AsFd, axis: MtAxis) -> io::Result<AbsInfo> {
    // SAFETY: linux/input.h declares EVIOCGABS(abs) as _IOR('E', 0x40 + abs, struct
    // input_absinfo); each opcode is built from `AbsInfo`, its #[repr(C)] mirror, and the
    // kernel writes exactly one of them.
    let r = unsafe {
        match axis {
            MtAxis::Slot => ioctl::ioctl(fd, Getter::<EVIOCGABS_MT_SLOT, AbsInfo>::new()),
            MtAxis::PositionX => ioctl::ioctl(fd, Getter::<EVIOCGABS_MT_X, AbsInfo>::new()),
            MtAxis::PositionY => ioctl::ioctl(fd, Getter::<EVIOCGABS_MT_Y, AbsInfo>::new()),
        }
    };
    r.map_err(io::Error::from)
}

/// Stamps this fd's events on `CLOCK_MONOTONIC` instead of the wall clock. Per open file: no
/// other reader of the device is affected.
pub fn monotonic_events(fd: impl AsFd) -> io::Result<()> {
    // SAFETY: linux/input.h declares EVIOCSCLOCKID as _IOW('E', 0xa0, int) and evdev reads an
    // `int` through the pointer; the opcode is sized from `i32` and the argument is one.
    unsafe { ioctl::ioctl(fd, Setter::<EVIOCSCLOCKID, i32>::new(CLOCK_MONOTONIC)) }
        .map_err(io::Error::from)
}

/// `struct input_id`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

/// `struct uinput_setup`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UinputSetup {
    pub id: InputId,
    pub name: [u8; 80],
    pub ff_effects_max: u32,
}

/// `struct input_absinfo`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

/// `struct uinput_abs_setup`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UinputAbsSetup {
    pub code: u16,
    pub absinfo: AbsInfo,
}

const UI_DEV_CREATE: Opcode = opcode::none(b'U', 1);
const UI_DEV_DESTROY: Opcode = opcode::none(b'U', 2);
const UI_DEV_SETUP: Opcode = opcode::write::<UinputSetup>(b'U', 3);
const UI_ABS_SETUP: Opcode = opcode::write::<UinputAbsSetup>(b'U', 4);
const UI_SET_EVBIT: Opcode = opcode::write::<i32>(b'U', 100);
const UI_SET_KEYBIT: Opcode = opcode::write::<i32>(b'U', 101);
const UI_SET_RELBIT: Opcode = opcode::write::<i32>(b'U', 102);
const UI_SET_ABSBIT: Opcode = opcode::write::<i32>(b'U', 103);
const UI_SET_PROPBIT: Opcode = opcode::write::<i32>(b'U', 110);

/// Which capability bit set a `UI_SET_*BIT` call writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitSet {
    Ev,
    Key,
    Rel,
    Abs,
    Prop,
}

/// Enables one capability bit on a uinput fd before `UI_DEV_CREATE`.
pub fn set_bit(fd: impl AsFd, set: BitSet, code: u16) -> io::Result<()> {
    let value = usize::from(code);
    // SAFETY: linux/uinput.h declares UI_SET_{EV,KEY,REL,ABS,PROP}BIT as _IOW('U', n, int) but
    // the kernel reads the argument as an integer value, not a pointer (uinput_ioctl_handler
    // passes `arg` straight to set_bit). IntegerSetter passes exactly that integer.
    let r = unsafe {
        match set {
            BitSet::Ev => ioctl::ioctl(&fd, IntegerSetter::<UI_SET_EVBIT>::new_usize(value)),
            BitSet::Key => ioctl::ioctl(&fd, IntegerSetter::<UI_SET_KEYBIT>::new_usize(value)),
            BitSet::Rel => ioctl::ioctl(&fd, IntegerSetter::<UI_SET_RELBIT>::new_usize(value)),
            BitSet::Abs => ioctl::ioctl(&fd, IntegerSetter::<UI_SET_ABSBIT>::new_usize(value)),
            BitSet::Prop => ioctl::ioctl(&fd, IntegerSetter::<UI_SET_PROPBIT>::new_usize(value)),
        }
    };
    r.map_err(io::Error::from)
}

/// Declares one absolute axis's range and resolution.
pub fn abs_setup(fd: impl AsFd, setup: UinputAbsSetup) -> io::Result<()> {
    // SAFETY: UI_ABS_SETUP is _IOW('U', 4, struct uinput_abs_setup); `UinputAbsSetup` is its
    // #[repr(C)] mirror and the opcode is sized from it.
    unsafe { ioctl::ioctl(fd, Setter::<UI_ABS_SETUP, UinputAbsSetup>::new(setup)) }
        .map_err(io::Error::from)
}

/// Declares the device's identity.
pub fn dev_setup(fd: impl AsFd, setup: UinputSetup) -> io::Result<()> {
    // SAFETY: UI_DEV_SETUP is _IOW('U', 3, struct uinput_setup); `UinputSetup` is its
    // #[repr(C)] mirror and the opcode is sized from it.
    unsafe { ioctl::ioctl(fd, Setter::<UI_DEV_SETUP, UinputSetup>::new(setup)) }
        .map_err(io::Error::from)
}

/// Creates the device. Never called by any test.
pub fn dev_create(fd: impl AsFd) -> io::Result<()> {
    // SAFETY: UI_DEV_CREATE is _IO('U', 1): no argument.
    unsafe { ioctl::ioctl(fd, NoArg::<UI_DEV_CREATE>::new()) }.map_err(io::Error::from)
}

/// Destroys the device.
pub fn dev_destroy(fd: impl AsFd) -> io::Result<()> {
    // SAFETY: UI_DEV_DESTROY is _IO('U', 2): no argument.
    unsafe { ioctl::ioctl(fd, NoArg::<UI_DEV_DESTROY>::new()) }.map_err(io::Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcodes_match_the_kernel_headers() {
        // Values from linux/uinput.h, linux/hidraw.h and linux/input.h on x86_64.
        assert_eq!(HIDIOCSFEATURE_3, 0xc003_4806);
        assert_eq!(UI_DEV_CREATE, 0x5501);
        assert_eq!(UI_DEV_DESTROY, 0x5502);
        assert_eq!(UI_DEV_SETUP, 0x405c_5503);
        assert_eq!(UI_ABS_SETUP, 0x401c_5504);
        assert_eq!(UI_SET_EVBIT, 0x4004_5564);
        assert_eq!(UI_SET_KEYBIT, 0x4004_5565);
        assert_eq!(UI_SET_RELBIT, 0x4004_5566);
        assert_eq!(UI_SET_ABSBIT, 0x4004_5567);
        assert_eq!(UI_SET_PROPBIT, 0x4004_556e);
        assert_eq!(EVIOCGABS_MT_SLOT, 0x8018_456f);
        assert_eq!(EVIOCGABS_MT_X, 0x8018_4575);
        assert_eq!(EVIOCGABS_MT_Y, 0x8018_4576);
        assert_eq!(EVIOCSCLOCKID, 0x4004_45a0);
        assert_eq!(std::mem::size_of::<AbsInfo>(), 24);
        assert_eq!(std::mem::size_of::<UinputSetup>(), 92);
        assert_eq!(std::mem::size_of::<UinputAbsSetup>(), 28);
    }
}
