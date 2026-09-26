//! `palmrest record FILE`: raw reads of the configured sensor with their arrival times, for
//! replay and tuning (a v1 file for hidraw, v2 naming the node's geometry for evdev).
//!
//! Opens the node read-only: it only listens, exactly as the kernel and any other reader do,
//! and sends nothing to the mouse.

use super::hidraw::{self, Read};
use super::source::{self, Purpose};
use crate::config::DeviceSource;
use crate::recording::{Record, encode_record, header};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

/// When a recording stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Stop after this long with no report (a still mouse).
    pub idle: Duration,
    /// Stop after this long in total.
    pub total: Duration,
}

/// Records until a limit is reached; returns the number of reads captured.
pub fn record(out: &Path, source: DeviceSource, limits: Limits) -> Result<usize, String> {
    let opened = source::open(source, Purpose::Record)?;
    let info = &opened.info;
    let mut file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    file.write_all(&header(&opened.recorded()))
        .map_err(|e| e.to_string())?;
    eprintln!(
        "palmrest: recording {} ({}) to {}",
        info.name,
        info.path,
        out.display()
    );
    let start = hidraw::now();
    let mut last = start;
    let mut count = 0;
    loop {
        let now = hidraw::now();
        if now.since(last) >= limits.idle || now.since(start) >= limits.total {
            break;
        }
        match hidraw::read_up_to(&opened.fd, Duration::from_millis(100), opened.read_len) {
            Read::Report(bytes) => {
                last = hidraw::now();
                file.write_all(&encode_record(&Record { at: last, bytes }))
                    .map_err(|e| e.to_string())?;
                count += 1;
            }
            Read::Timeout => {}
            Read::Failed(e) => return Err(e),
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(count)
}
