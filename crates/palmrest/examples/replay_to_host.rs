//! `replay_to_host [--held] [FIXTURE.bin ...]`: recorded reads through the pure core, printed as
//! the `ToHost` JSON lines a subscriber receives, one packet per line.
//!
//! This is the stream casement subscribes to (it is written to
//! `casement-testkit/fixtures/palmrest/*.jsonl` per fixture). With no arguments every Magic
//! Mouse fixture in `tests/fixtures` is replayed in name order, each introduced by a `# name`
//! line on stderr so stdout stays pure JSON lines. `--held` replays as if a host held the
//! pointer, which is when taps are sent.
//!
//! A dev example only: it reads files and never opens a device (no hidraw, evdev or uinput),
//! and it starts no daemon.

use palmrest::config::Config;
use palmrest::recording::{self, Record, RecordedSource};
use palmrest::replay::{Timing, feed_from};
use palmrest::stamp::StampClock;
use palmrest::{Mono, Out, Tracker};
use palmrest_protocol::{Over, ToHost};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The most ticks one gap may take, as in `palmrest::replay`.
const MAX_TICKS: usize = 100_000;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("replay_to_host: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let (held, files) = split_args(args);
    let files = if files.is_empty() {
        default_fixtures()?
    } else {
        files
    };
    for file in files {
        eprintln!("# {}", file.display());
        for line in to_host_lines(&file, held)? {
            println!("{line}");
        }
    }
    Ok(())
}

/// Who holds the pointer for the replay, and the fixture paths.
fn split_args(args: Vec<String>) -> (Over, Vec<PathBuf>) {
    let held = match args.iter().any(|a| a == "--held") {
        true => Over::Ours,
        false => Over::NotOurs,
    };
    let files = args
        .into_iter()
        .filter(|a| a != "--held")
        .map(PathBuf::from)
        .collect();
    (held, files)
}

/// Every `.bin` fixture of palmrest's tests, in name order.
fn default_fixtures() -> Result<Vec<PathBuf>, String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .collect();
    files.sort();
    Ok(files)
}

/// The `ToHost` packets of one recording, as JSON lines.
fn to_host_lines(file: &Path, held: Over) -> Result<Vec<String>, String> {
    let name = file.display();
    let bytes = std::fs::read(file).map_err(|e| format!("{name}: {e}"))?;
    let rec = recording::read(&bytes).map_err(|e| format!("{name}: {e}"))?;
    Ok(to_host(&rec.source, &rec.records, held)
        .iter()
        .map(|m| String::from_utf8_lossy(&palmrest_protocol::encode(m)).into_owned())
        .collect())
}

/// Every `ToHost` the core sends for `records`, in order, ticking at its deadlines between
/// reads and after the last one until it has none left.
fn to_host(source: &RecordedSource, records: &[Record], held: Over) -> Vec<ToHost> {
    let mut sensor = source.sensor();
    let (mut tracker, claim) = Tracker::on(Config::default(), sensor.surface()).claim(held);
    let mut clock = StampClock::new();
    let mut outs = claim;
    for r in records {
        let (t, ticked) = ticks_until(tracker, Some(r.at));
        outs.extend(ticked);
        let (t, c, step) = feed_from(&mut sensor, t, clock, r, Timing::Arrival);
        (tracker, clock) = (t, c);
        outs.extend(step.outs);
    }
    outs.extend(ticks_until(tracker, None).1);
    outs.into_iter()
        .filter_map(|o| match o {
            Out::Host(h) => Some(h),
            _ => None,
        })
        .collect()
}

/// Ticks `tracker` at every deadline before `until` (all of them if `None`).
fn ticks_until(mut tracker: Tracker, until: Option<Mono>) -> (Tracker, Vec<Out>) {
    let mut outs = Vec::new();
    for _ in 0..MAX_TICKS {
        let Some(d) = tracker.deadline() else { break };
        if until.is_some_and(|u| d >= u) {
            break;
        }
        let (t, o) = tracker.tick(d);
        tracker = t;
        outs.extend(o);
        if tracker.deadline().is_some_and(|next| next <= d) {
            break; // a deadline that does not move on would never let the replay finish
        }
    }
    (tracker, outs)
}
