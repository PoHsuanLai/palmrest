//! The `palmrest` command line: the daemon and its record / replay / probe verbs. `run` is the
//! one way in, and the only place that reads the process environment.

mod probe;
mod record;
mod replay;
mod schema;

#[cfg(test)]
mod tests;

use crate::config::{self, Config};
use crate::daemon::uinput::Origin;
use crate::daemon::{self, Env};
use crate::error::Error;
use crate::replay::Timing;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "usage:
  palmrest [--direct-uinput]       run the daemon (--direct-uinput: make the virtual devices
                                   without the input service, for development)
  palmrest record FILE [SECONDS]   record raw reads of the Magic Mouse
                                   (stops after 20 s of no touch)
  palmrest replay FILE [--calibrated] [--latency]
                                   print the outputs a recording produces
  palmrest --check-uinput [--direct-uinput]
                                   can palmrest make its virtual devices? (asks for a touchpad
                                   and lets it go)
  palmrest --print-config          print the default gestures.toml
  palmrest --write-schema DIR      write palmrest.settings.toml, the schema of gestures.toml,
                                   into DIR (for the Settings app)";

/// A recording stops after this long when no length is given.
const DEFAULT_RECORD: Duration = Duration::from_secs(600);

/// What `palmrest replay` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayReport {
    /// Every output, one per line.
    Outputs,
    /// Per-read latency percentiles.
    Latency,
}

/// One invocation of the command.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Daemon {
        origin: Origin,
    },
    Record {
        file: PathBuf,
        total: Duration,
    },
    Replay {
        file: PathBuf,
        report: ReplayReport,
        timing: Timing,
    },
    CheckUinput {
        origin: Origin,
    },
    PrintConfig,
    WriteSchema {
        dir: PathBuf,
    },
    Version,
}

/// The command `args` (without the program name) ask for; `None` is a usage error.
fn parse(args: &[&str]) -> Option<Command> {
    match args {
        [] => Some(Command::Daemon {
            origin: Origin::Service,
        }),
        ["--direct-uinput"] => Some(Command::Daemon {
            origin: Origin::Direct,
        }),
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
        ["--check-uinput"] => Some(Command::CheckUinput {
            origin: Origin::Service,
        }),
        ["--check-uinput", "--direct-uinput"] => Some(Command::CheckUinput {
            origin: Origin::Direct,
        }),
        ["--print-config"] => Some(Command::PrintConfig),
        ["--write-schema", dir] => Some(Command::WriteSchema {
            dir: PathBuf::from(dir),
        }),
        ["--version"] => Some(Command::Version),
        _ => None,
    }
}

/// `--latency` picks the latency report; a flag other than the two replay knows is a usage error.
fn replay_report(flags: &[&str]) -> Option<ReplayReport> {
    if !flags
        .iter()
        .all(|f| matches!(*f, "--calibrated" | "--latency"))
    {
        return None;
    }
    Some(if flags.contains(&"--latency") {
        ReplayReport::Latency
    } else {
        ReplayReport::Outputs
    })
}

/// Runs the command `args` (the program name first) name, with the process environment.
pub fn run(args: impl IntoIterator<Item = String>) -> ExitCode {
    let args: Vec<String> = args.into_iter().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let env = Env::from_lookup(|k| std::env::var(k).ok());
    let result = match parse(&args) {
        None => Err(Error::Usage(USAGE)),
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

fn execute(command: Command, env: &Env) -> Result<(), Error> {
    match command {
        Command::Daemon { origin } => daemon::run(env, origin),
        Command::Record { file, total } => record::record(&file, total),
        Command::Replay {
            file,
            report,
            timing,
        } => replay::replay(&file, report, timing, env),
        Command::CheckUinput { origin } => probe::check_uinput(origin),
        Command::PrintConfig => {
            print!("{}", config::to_toml(&Config::default()));
            Ok(())
        }
        Command::WriteSchema { dir } => schema::write_schema(&dir),
        Command::Version => {
            println!("palmrest {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
