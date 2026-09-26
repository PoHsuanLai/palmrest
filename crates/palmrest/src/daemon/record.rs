//! `palmrest record FILE`: raw reports with their arrival times, for replay and tuning.
//!
//! Opens the node read-only: it only listens, exactly as the kernel and any other reader do,
//! and sends nothing to the mouse.

use super::discover::{self, SYS_HIDRAW};
use super::hidraw::{self, Access, Read};
use crate::recording::{MAGIC, Record, encode_record};
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
pub fn record(out: &Path, limits: Limits) -> Result<usize, String> {
    let (path, info) = discover::find(Path::new(SYS_HIDRAW)).ok_or("no Magic Mouse found")?;
    let (fd, _) =
        hidraw::open(&path, Access::ReadOnly).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    file.write_all(MAGIC).map_err(|e| e.to_string())?;
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
        match hidraw::read(&fd, Duration::from_millis(100)) {
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
