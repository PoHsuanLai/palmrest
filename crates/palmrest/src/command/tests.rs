//! Argument parsing.

use super::{Command, DEFAULT_RECORD, Origin, ReplayReport, parse};
use crate::replay::Timing;
use std::path::PathBuf;
use std::time::Duration;

fn replay(report: ReplayReport, timing: Timing) -> Option<Command> {
    Some(Command::Replay {
        file: PathBuf::from("f"),
        report,
        timing,
    })
}

#[test]
fn arguments_parse_to_commands() {
    let cases: &[(&str, &[&str], Option<Command>)] = &[
        (
            "no arguments run the daemon on the input service",
            &[],
            Some(Command::Daemon {
                origin: Origin::Service,
            }),
        ),
        (
            "the direct flag runs it without the service",
            &["--direct-uinput"],
            Some(Command::Daemon {
                origin: Origin::Direct,
            }),
        ),
        (
            "record without a length",
            &["record", "f"],
            Some(Command::Record {
                file: PathBuf::from("f"),
                total: DEFAULT_RECORD,
            }),
        ),
        (
            "record with seconds",
            &["record", "f", "30"],
            Some(Command::Record {
                file: PathBuf::from("f"),
                total: Duration::from_secs(30),
            }),
        ),
        ("record with a bad length", &["record", "f", "x"], None),
        (
            "replay outputs",
            &["replay", "f"],
            replay(ReplayReport::Outputs, Timing::Arrival),
        ),
        (
            "replay calibrated",
            &["replay", "f", "--calibrated"],
            replay(ReplayReport::Outputs, Timing::Calibrated),
        ),
        (
            "latency",
            &["replay", "f", "--latency"],
            replay(ReplayReport::Latency, Timing::Arrival),
        ),
        ("an unknown flag", &["replay", "f", "--curve", "x"], None),
        (
            "probe",
            &["--check-uinput"],
            Some(Command::CheckUinput {
                origin: Origin::Service,
            }),
        ),
        (
            "probe without the service",
            &["--check-uinput", "--direct-uinput"],
            Some(Command::CheckUinput {
                origin: Origin::Direct,
            }),
        ),
        (
            "print config",
            &["--print-config"],
            Some(Command::PrintConfig),
        ),
        (
            "write schema",
            &["--write-schema", "d"],
            Some(Command::WriteSchema { dir: "d".into() }),
        ),
        (
            "write schema without a directory",
            &["--write-schema"],
            None,
        ),
        ("version", &["--version"], Some(Command::Version)),
        ("an unknown verb", &["frobnicate"], None),
    ];
    for (name, args, want) in cases {
        assert_eq!(&parse(args), want, "{name}");
    }
}
