//! The mouse's hidraw node: opening it, enabling multitouch, reading one report. `read_up_to`
//! also serves the evdev node, whose reads are runs of events.

use super::hidioc;
use crate::time::Mono;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fd::OwnedFd;
use rustix::fs::{Mode, OFlags};
use rustix::time::{ClockId, Timespec, clock_gettime};
use std::io;
use std::path::Path;
use std::time::Duration;

/// The feature report that turns on multitouch reports (R12).
pub const ENABLE_MULTITOUCH: [u8; 3] = [0xf1, 0x02, 0x01];
/// The largest read the mouse produces.
pub const READ_LEN: usize = 64;

/// How the node was opened: read-write can enable multitouch, read-only cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    ReadWrite,
    ReadOnly,
}

/// Now on `CLOCK_MONOTONIC`.
pub fn now() -> Mono {
    let t = clock_gettime(ClockId::Monotonic);
    Mono(
        u64::try_from(t.tv_sec).unwrap_or(0) * 1_000_000_000
            + u64::try_from(t.tv_nsec).unwrap_or(0),
    )
}

/// Opens the node read-write (falling back to read-only).
pub fn open(path: &Path, want: Access) -> io::Result<(OwnedFd, Access)> {
    let flags = OFlags::CLOEXEC | OFlags::NONBLOCK;
    if want == Access::ReadWrite
        && let Ok(fd) = rustix::fs::open(path, flags | OFlags::RDWR, Mode::empty())
    {
        return Ok((fd, Access::ReadWrite));
    }
    Ok((
        rustix::fs::open(path, flags | OFlags::RDONLY, Mode::empty())?,
        Access::ReadOnly,
    ))
}

/// Sends `F1 02 01`; the kernel sends it at probe and resume too, so repeating it is harmless.
pub fn enable_multitouch(fd: &OwnedFd) -> io::Result<()> {
    hidioc::send_feature(fd, ENABLE_MULTITOUCH)
}

/// What a wait on the node produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    Report(Vec<u8>),
    Timeout,
    Failed(String),
}

/// Waits up to `timeout` for one read of at most `len` bytes.
pub fn read_up_to(fd: &OwnedFd, timeout: Duration, len: usize) -> Read {
    let ts = Timespec {
        tv_sec: timeout.as_secs() as i64,
        tv_nsec: i64::from(timeout.subsec_nanos()),
    };
    let mut fds = [PollFd::new(fd, PollFlags::IN)];
    match poll(&mut fds, Some(&ts)) {
        Ok(0) => return Read::Timeout,
        Ok(_) => {}
        Err(e) if e == rustix::io::Errno::INTR => return Read::Timeout,
        Err(e) => return Read::Failed(e.to_string()),
    }
    let mut buf = vec![0u8; len];
    match rustix::io::read(fd, &mut buf) {
        Ok(0) => Read::Failed("end of file".to_owned()),
        Ok(n) => Read::Report(buf[..n].to_vec()),
        Err(rustix::io::Errno::AGAIN) => Read::Timeout,
        Err(e) => Read::Failed(e.to_string()),
    }
}
