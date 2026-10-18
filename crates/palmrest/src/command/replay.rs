//! `palmrest replay FILE`: a recording through the core, printed as outputs, latency or curves.

use super::ReplayReport;
use crate::config::{self, Config};
use crate::daemon::Env;
use crate::daemon::latency::{self, LatencyReport};
use crate::error::Error;
use crate::recording::{self, Record, RecordedSource};
use crate::replay::compare::compare_curves_from;
use crate::replay::{Timing, replay_from};
use std::path::Path;

/// Replays `file` with the user's config and prints `report`.
pub(super) fn replay(
    file: &Path,
    report: ReplayReport,
    timing: Timing,
    env: &Env,
) -> Result<(), Error> {
    let bytes = std::fs::read(file).map_err(|source| Error::File {
        path: file.to_path_buf(),
        source,
    })?;
    let rec = recording::read(&bytes).map_err(|source| Error::Recording {
        path: file.to_path_buf(),
        source,
    })?;
    let cfg = config::load(env.config_dir.as_deref()).config;
    match report {
        ReplayReport::Latency => {
            print_latency(&latency::measure(&rec.source, &rec.records, cfg));
        }
        ReplayReport::Curves => print!(
            "{}",
            compare_curves_from(&rec.source, &rec.records, &cfg, timing).table()
        ),
        ReplayReport::Outputs => print_outputs(&rec.source, &rec.records, cfg, timing),
    }
    Ok(())
}

fn print_latency(r: &LatencyReport) {
    println!("reads {}", r.n);
    println!(
        "socket  p50 {:?} p95 {:?} p99 {:?} max {:?}",
        r.socket.p50, r.socket.p95, r.socket.p99, r.socket.max
    );
    println!(
        "uinput  p50 {:?} p95 {:?} p99 {:?} max {:?}",
        r.uinput.p50, r.uinput.p95, r.uinput.p99, r.uinput.max
    );
}

fn print_outputs(source: &RecordedSource, records: &[Record], cfg: Config, timing: Timing) {
    for step in replay_from(source, records, cfg, timing).1 {
        for o in step.outs {
            println!(
                "{} {}",
                step.at.0,
                String::from_utf8_lossy(&palmrest_protocol::encode(&o))
            );
        }
        if let Some(e) = step.error {
            println!("{} error {e}", step.at.0);
        }
    }
}
