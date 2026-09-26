//! The `palmrest record` file: raw reads with their monotonic arrival times.
//!
//! Raw bytes, not decoded frames, so a recording also exercises the sensor's decoder and
//! survives a decoder change. Two versions:
//!
//! - **v1**, `PALMREC\x01`: Magic Mouse hidraw reads. Every recording made before sensors
//!   existed, every fixture, and still what `palmrest record` writes for hidraw.
//! - **v2**, `PALMREC\x02`: a `u16 LE` length and that many bytes of JSON naming the source
//!   (`RecordedSource`: which sensor, and for evdev the node's geometry, which the bytes alone
//!   do not carry), then the same records.
//!
//! Records: `u64 LE ns`, `u16 LE len`, `len` bytes.

use crate::sensor::{EvdevMultitouch, EvdevSpec, MagicMouseHid, Sensor};
use crate::time::Mono;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The first eight bytes of a v1 (Magic Mouse hidraw) recording.
pub const MAGIC: &[u8; 8] = b"PALMREC\x01";
/// The first eight bytes of a v2 recording.
pub const MAGIC_V2: &[u8; 8] = b"PALMREC\x02";

/// One read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub at: Mono,
    pub bytes: Vec<u8>,
}

/// Which sensor the reads came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum RecordedSource {
    MagicMouseHid,
    Evdev(EvdevSpec),
}

impl RecordedSource {
    /// A fresh sensor to replay the reads through.
    pub fn sensor(&self) -> Box<dyn Sensor> {
        match self {
            RecordedSource::MagicMouseHid => Box::new(MagicMouseHid::default()),
            RecordedSource::Evdev(spec) => Box::new(EvdevMultitouch::new(*spec)),
        }
    }
}

/// A whole recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording {
    pub source: RecordedSource,
    pub records: Vec<Record>,
}

/// A recording that cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecordingError {
    #[error("not a palmrest recording (bad magic)")]
    Magic,
    #[error("recording truncated at byte {0}")]
    Truncated(usize),
    #[error("recording header is not a known source: {0}")]
    Header(String),
    #[error("this recording is from {0:?}, not the Magic Mouse's hidraw node")]
    NotHidraw(RecordedSource),
}

/// Serialises Magic Mouse hidraw records as v1.
pub fn to_bytes(records: &[Record]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend(records.iter().flat_map(encode_record));
    out
}

/// The bytes a recording starts with: v1 for hidraw (readable by every palmrest), v2 else.
pub fn header(source: &RecordedSource) -> Vec<u8> {
    match source {
        RecordedSource::MagicMouseHid => MAGIC.to_vec(),
        RecordedSource::Evdev(_) => {
            let json = serde_json::to_vec(source).unwrap_or_default(); // plain data; cannot fail
            let len = u16::try_from(json.len()).unwrap_or(u16::MAX);
            let mut out = MAGIC_V2.to_vec();
            out.extend(len.to_le_bytes());
            out.extend(&json[..usize::from(len)]);
            out
        }
    }
}

/// Serialises a whole recording.
pub fn recording_to_bytes(r: &Recording) -> Vec<u8> {
    let mut out = header(&r.source);
    out.extend(r.records.iter().flat_map(encode_record));
    out
}

/// One record's bytes, for appending while recording.
pub fn encode_record(r: &Record) -> Vec<u8> {
    let len = u16::try_from(r.bytes.len()).unwrap_or(u16::MAX);
    let mut out = Vec::with_capacity(10 + usize::from(len));
    out.extend(r.at.0.to_le_bytes());
    out.extend(len.to_le_bytes());
    out.extend(&r.bytes[..usize::from(len)]);
    out
}

/// Parses a Magic Mouse hidraw recording (v1, or v2 naming hidraw).
pub fn parse(bytes: &[u8]) -> Result<Vec<Record>, RecordingError> {
    let r = read(bytes)?;
    match r.source {
        RecordedSource::MagicMouseHid => Ok(r.records),
        other => Err(RecordingError::NotHidraw(other)),
    }
}

/// Parses a recording of either version.
pub fn read(bytes: &[u8]) -> Result<Recording, RecordingError> {
    if let Some(body) = bytes.strip_prefix(MAGIC.as_slice()) {
        return Ok(Recording {
            source: RecordedSource::MagicMouseHid,
            records: records(body, MAGIC.len())?,
        });
    }
    let body = bytes
        .strip_prefix(MAGIC_V2.as_slice())
        .ok_or(RecordingError::Magic)?;
    let head = MAGIC_V2.len();
    let len = body
        .get(..2)
        .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
        .ok_or(RecordingError::Truncated(head))?;
    let json = body
        .get(2..2 + len)
        .ok_or(RecordingError::Truncated(head + 2))?;
    let source: RecordedSource =
        serde_json::from_slice(json).map_err(|e| RecordingError::Header(e.to_string()))?;
    Ok(Recording {
        source,
        records: records(&body[2 + len..], head + 2 + len)?,
    })
}

fn records(body: &[u8], offset: usize) -> Result<Vec<Record>, RecordingError> {
    let mut records = Vec::new();
    let mut at = 0;
    while at < body.len() {
        let err = RecordingError::Truncated(offset + at);
        let head = body.get(at..at + 10).ok_or(err.clone())?;
        let ns = u64::from_le_bytes(head[..8].try_into().map_err(|_| err.clone())?);
        let len = usize::from(u16::from_le_bytes([head[8], head[9]]));
        let data = body.get(at + 10..at + 10 + len).ok_or(err)?;
        records.push(Record {
            at: Mono(ns),
            bytes: data.to_vec(),
        });
        at += 10 + len;
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensor::evdev::AbsAxis;

    fn recs() -> Vec<Record> {
        vec![
            Record {
                at: Mono(5),
                bytes: vec![0x12; 22],
            },
            Record {
                at: Mono(9),
                bytes: vec![],
            },
        ]
    }

    #[test]
    fn round_trip_and_truncation() {
        let recs = recs();
        let bytes = to_bytes(&recs);
        assert_eq!(parse(&bytes), Ok(recs));
        assert_eq!(
            parse(&bytes[..bytes.len() - 1]),
            Err(RecordingError::Truncated(40))
        );
        assert_eq!(parse(b"nope"), Err(RecordingError::Magic));
    }

    #[test]
    fn v2_carries_the_evdev_geometry_and_v1_stays_v1() {
        let axis = |min, max, resolution| AbsAxis {
            min,
            max,
            resolution,
        };
        let spec = EvdevSpec::new(axis(0, 1000, 10), axis(0, 600, 12), 9).unwrap();
        let r = Recording {
            source: RecordedSource::Evdev(spec),
            records: recs(),
        };
        let bytes = recording_to_bytes(&r);
        assert!(bytes.starts_with(MAGIC_V2));
        assert_eq!(read(&bytes), Ok(r.clone()));
        assert_eq!(
            parse(&bytes),
            Err(RecordingError::NotHidraw(r.source)),
            "an evdev recording is never decoded as hidraw"
        );
        let hid = Recording {
            source: RecordedSource::MagicMouseHid,
            records: recs(),
        };
        assert_eq!(recording_to_bytes(&hid), to_bytes(&recs()));
        assert_eq!(read(&to_bytes(&recs())), Ok(hid));
    }
}
