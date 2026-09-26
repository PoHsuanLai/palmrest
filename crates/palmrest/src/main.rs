//! The `palmrest` command: the daemon, and its record / replay / probe tools.

use palmrest::compare::compare_curves;
use palmrest::config::{self, Config};
use palmrest::daemon::{self, latency, record};
use palmrest::outputs::uinput::{self, PROBE_PERMIT, ProbePermit};
use palmrest::protocol;
use palmrest::recording;
use palmrest::replay::{Timing, replay};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "usage:
  palmrest                         run the daemon
  palmrest record FILE [SECONDS]   record raw reports (stops after 20 s of no touch)
  palmrest replay FILE [--calibrated] [--latency]
                                   print the outputs a recording produces
  palmrest replay FILE [--calibrated] --curve mac-like
                                   cumulative px and peak gain under both curves
  palmrest --check-uinput          can this user create virtual devices? (needs
                                   PALMREST_ALLOW_UINPUT_PROBE=1; opens and closes only)
  palmrest --print-config          print the default gestures.toml";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        [] => daemon::run(),
        ["record", file] => record_cmd(file, 600),
        ["record", file, secs] => secs
            .parse()
            .map_err(|_| USAGE.to_owned())
            .and_then(|s| record_cmd(file, s)),
        ["replay", file, flags @ ..] => replay_cmd(file, flags),
        ["--check-uinput"] => check_uinput(),
        ["--print-config"] => {
            print!("{}", config::to_toml(&Config::default()));
            Ok(())
        }
        ["--version"] => {
            println!("palmrest {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("palmrest: {e}");
            ExitCode::FAILURE
        }
    }
}

fn record_cmd(file: &str, total_secs: u64) -> Result<(), String> {
    let limits = record::Limits {
        idle: Duration::from_secs(20),
        total: Duration::from_secs(total_secs),
    };
    let n = record::record(Path::new(file), limits)?;
    eprintln!("palmrest: recorded {n} reads");
    Ok(())
}

fn replay_cmd(file: &str, flags: &[&str]) -> Result<(), String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let records = recording::parse(&bytes).map_err(|e| format!("{file}: {e}"))?;
    let env = |k: &str| std::env::var(k).ok();
    let cfg = config::load(config::config_dir(env).as_deref(), env).config;
    if flags.contains(&"--latency") {
        let r = latency::measure(&records, cfg);
        println!("reads {}", r.n);
        println!(
            "socket  p50 {:?} p95 {:?} p99 {:?} max {:?}",
            r.socket.p50, r.socket.p95, r.socket.p99, r.socket.max
        );
        println!(
            "uinput  p50 {:?} p95 {:?} p99 {:?} max {:?}",
            r.uinput.p50, r.uinput.p95, r.uinput.p99, r.uinput.max
        );
        return Ok(());
    }
    let timing = if flags.contains(&"--calibrated") {
        Timing::Calibrated
    } else {
        Timing::Arrival
    };
    match flags.iter().position(|f| *f == "--curve") {
        None => {}
        Some(i) if flags.get(i + 1) == Some(&"mac-like") => {
            print!("{}", compare_curves(&records, &cfg, timing).table());
            return Ok(());
        }
        Some(_) => return Err(USAGE.to_owned()),
    }
    for step in replay(&records, cfg, timing) {
        for o in step.outs {
            println!(
                "{} {}",
                step.at.0,
                String::from_utf8_lossy(&protocol::encode(&o))
            );
        }
        if let Some(e) = step.error {
            println!("{} error {e}", step.at.0);
        }
    }
    Ok(())
}

fn check_uinput() -> Result<(), String> {
    let permit = ProbePermit::from_env_value(std::env::var(PROBE_PERMIT).ok().as_deref());
    uinput::probe(permit, Path::new(uinput::UINPUT)).map_err(|e| e.to_string())?;
    println!(
        "uinput preflight OK: {} can be opened for writing",
        uinput::UINPUT
    );
    Ok(())
}
