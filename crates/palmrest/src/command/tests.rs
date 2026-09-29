//! Argument parsing.

use super::{Command, DEFAULT_RECORD, ReplayReport, parse};
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
        ("no arguments run the daemon", &[], Some(Command::Daemon)),
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
            "latency wins over a bad curve",
            &["replay", "f", "--curve", "x", "--latency"],
            replay(ReplayReport::Latency, Timing::Arrival),
        ),
        (
            "curve comparison",
            &["replay", "f", "--calibrated", "--curve", "mac-like"],
            replay(ReplayReport::Curves, Timing::Calibrated),
        ),
        ("an unknown curve", &["replay", "f", "--curve", "x"], None),
        ("a curve with no name", &["replay", "f", "--curve"], None),
        ("probe", &["--check-uinput"], Some(Command::CheckUinput)),
        (
            "print config",
            &["--print-config"],
            Some(Command::PrintConfig),
        ),
        ("version", &["--version"], Some(Command::Version)),
        ("an unknown verb", &["frobnicate"], None),
    ];
    for (name, args, want) in cases {
        assert_eq!(&parse(args), want, "{name}");
    }
}
