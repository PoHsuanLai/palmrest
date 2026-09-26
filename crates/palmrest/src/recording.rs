//! The `palmrest record` file: raw hidraw reads with their monotonic arrival times.
//!
//! Raw bytes, not decoded reports, so a recording also exercises the decoder and survives a
//! decoder change. Layout: the magic `PALMREC\x01`, then per read `u64 LE ns`, `u16 LE len`,
//! `len` bytes.

use crate::time::Mono;
use thiserror::Error;

/// The file's first eight bytes.
pub const MAGIC: &[u8; 8] = b"PALMREC\x01";

/// One hidraw read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub at: Mono,
    pub bytes: Vec<u8>,
}

/// A recording that cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecordingError {
    #[error("not a palmrest recording (bad magic)")]
    Magic,
    #[error("recording truncated at byte {0}")]
    Truncated(usize),
}

/// Serialises records.
pub fn to_bytes(records: &[Record]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    for r in records {
        out.extend(encode_record(r));
    }
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

/// Parses a whole recording.
pub fn parse(bytes: &[u8]) -> Result<Vec<Record>, RecordingError> {
    let body = bytes
        .strip_prefix(MAGIC.as_slice())
        .ok_or(RecordingError::Magic)?;
    let mut records = Vec::new();
    let mut at = 0;
    while at < body.len() {
        let err = RecordingError::Truncated(MAGIC.len() + at);
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

    #[test]
    fn round_trip_and_truncation() {
        let recs = vec![
            Record {
                at: Mono(5),
                bytes: vec![0x12; 22],
            },
            Record {
                at: Mono(9),
                bytes: vec![],
            },
        ];
        let bytes = to_bytes(&recs);
        assert_eq!(parse(&bytes), Ok(recs));
        assert_eq!(
            parse(&bytes[..bytes.len() - 1]),
            Err(RecordingError::Truncated(40))
        );
        assert_eq!(parse(b"nope"), Err(RecordingError::Magic));
    }
}
