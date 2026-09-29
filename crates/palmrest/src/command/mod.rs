//! The `palmrest` command line: the daemon and its record / replay / probe verbs. `run` is the
//! one way in, and the only place that reads the process environment.

mod probe;
mod record;
mod replay;

#[cfg(test)]
mod tests;

use crate::config::{self, Config};
use crate::daemon::{self, Env};
use crate::replay::Timing;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "usage:
  palmrest                         run the daemon
  palmrest record FILE [SECONDS]   record raw reads of the configured device.source
                                   (stops after 20 s of no touch)
  palmrest replay FILE [--calibrated] [--latency]
                                   print the outputs a recording produces
  palmrest replay FILE [--calibrated] --curve mac-like
                                   cumulative px and peak gain under both curves
  palmrest --check-uinput          can this user create virtual devices? (needs
                                   PALMREST_ALLOW_UINPUT_PROBE=1; opens and closes only)
  palmrest --print-config          print the default gestures.toml";

/// A recording stops after this long when no length is given.
const DEFAULT_RECORD: Duration = Duration::from_secs(600);

/// What `palmrest replay` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayReport {
    /// Every output, one per line.
    Outputs,
    /// Per-read latency percentiles.
    Latency,
    /// Cumulative px and peak gain under both curves.
    Curves,
}

/// One invocation of the command.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Daemon,
    Record {
        file: PathBuf,
        total: Duration,
    },
    Replay {
        file: PathBuf,
        report: ReplayReport,
        timing: Timing,
    },
    CheckUinput,
    PrintConfig,
    Version,
}

/// The command `args` (without the program name) ask for; `None` is a usage error.
fn parse(args: &[&str]) -> Option<Command> {
    match args {
        [] => Some(Command::Daemon),
        ["record", file] => Some(Command::Record {
            file: PathBuf::from(file),
            total: DEFAULT_RECORD,
        }),
        ["record", file, secs] => secs.parse().ok().map(|s| Command::Record {
            file: PathBuf::from(file),
            total: Duration::from_secs(s),
        }),
        ["replay", file, flags @ ..] => replay_report(flags).map(|report| Command::Replay {
            file: PathBuf::from(file),
            report,
            timing: if flags.contains(&"--calibrated") {
                Timing::Calibrated
            } else {
                Timing::Arrival
            },
        }),
        ["--check-uinput"] => Some(Command::CheckUinput),
        ["--print-config"] => Some(Command::PrintConfig),
        ["--version"] => Some(Command::Version),
        _ => None,
    }
}

/// `--latency` wins over everything; `--curve` must name `mac-like`.
fn replay_report(flags: &[&str]) -> Option<ReplayReport> {
    if flags.contains(&"--latency") {
        return Some(ReplayReport::Latency);
    }
    match flags.iter().position(|f| *f == "--curve") {
        None => Some(ReplayReport::Outputs),
        Some(i) if flags.get(i + 1) == Some(&"mac-like") => Some(ReplayReport::Curves),
        Some(_) => None,
    }
}

/// Runs the command `args` (the program name first) name, with the process environment.
pub fn run(args: impl IntoIterator<Item = String>) -> ExitCode {
    let args: Vec<String> = args.into_iter().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let env = Env::from_lookup(|k| std::env::var(k).ok());
    let result = match parse(&args) {
        None => Err(USAGE.to_owned()),
        Some(command) => execute(command, &env),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("palmrest: {e}");
            ExitCode::FAILURE
        }
    }
}

fn execute(command: Command, env: &Env) -> Result<(), String> {
    match command {
        Command::Daemon => daemon::run(env),
        Command::Record { file, total } => record::record(&file, total, env),
        Command::Replay {
            file,
            report,
            timing,
        } => replay::replay(&file, report, timing, env),
        Command::CheckUinput => probe::check_uinput(env),
        Command::PrintConfig => {
            print!("{}", config::to_toml(&Config::default()));
            Ok(())
        }
        Command::Version => {
            println!("palmrest {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
