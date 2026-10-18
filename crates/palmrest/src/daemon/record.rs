//! `palmrest record FILE`: raw reads of the configured sensor with their arrival times, for
//! replay and tuning (a v1 file for hidraw, v2 naming the node's geometry for evdev).
//!
//! Opens the node read-only: it only listens, exactly as the kernel and any other reader do,
//! and sends nothing to the mouse.

use super::hidraw::{self, Read};
use super::source::{self, Purpose};
use crate::config::DeviceSource;
use crate::error::Error;
use crate::recording::{Record, encode_record, header};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
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

/// Mode of a new recording file: touch traces are the person's, not the machine's.
const RECORDING_MODE: u32 = 0o600;

/// Creates (or empties) `path` for writing; a new file is readable by its owner only.
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(RECORDING_MODE)
        .open(path)
}

/// Records until a limit is reached; returns the number of reads captured.
pub fn record(out: &Path, source: DeviceSource, limits: Limits) -> Result<usize, Error> {
    let opened = source::open(source, Purpose::Record)?;
    let info = &opened.info;
    let mut file = create_private(out).map_err(|source| Error::File {
        path: out.to_path_buf(),
        source,
    })?;
    file.write_all(&header(&opened.recorded()))?;
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
                file.write_all(&encode_record(&Record { at: last, bytes }))?;
                count += 1;
            }
            Read::Timeout => {}
            Read::Failed(e) => return Err(Error::Read(e)),
        }
    }
    file.flush()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_new_recording_is_private_whatever_the_umask() {
        let dir = std::env::temp_dir().join(format!("palmrest-rec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trace.bin");
        drop(create_private(&path).unwrap());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        // The process umask can only remove bits, never add group or other access.
        assert_eq!(mode & 0o077, 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
