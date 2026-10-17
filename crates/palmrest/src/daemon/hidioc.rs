//! The one hidraw ioctl: the one module allowed `unsafe` (12.3.12).
//!
//! No safe crate declares `HIDIOCSFEATURE`, so it goes through rustix's typed ioctl pattern;
//! the only unsafety is the promise that the opcode and the argument type match the kernel's
//! declaration. The block states which declaration it matches. The evdev and uinput ioctls are
//! in `sys`, through `input-linux`, with no `unsafe`.
#![allow(unsafe_code)]

use rustix::fd::AsFd;
use rustix::ioctl::{self, Opcode, Updater, opcode};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_opcode_matches_the_kernel_header() {
        // Value from linux/hidraw.h on x86_64.
        assert_eq!(HIDIOCSFEATURE_3, 0xc003_4806);
    }
}
