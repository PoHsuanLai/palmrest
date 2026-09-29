//! `palmrest record FILE [SECONDS]`.

use crate::config;
use crate::daemon::Env;
use crate::daemon::record::{self, Limits};
use std::path::Path;
use std::time::Duration;

/// A recording stops after this long with no report.
const IDLE: Duration = Duration::from_secs(20);

/// Records the configured `device.source` into `file` for at most `total`.
pub(super) fn record(file: &Path, total: Duration, env: &Env) -> Result<(), String> {
    let limits = Limits { idle: IDLE, total };
    let source = config::load(env.config_dir.as_deref()).config.device.source;
    let n = record::record(file, source, limits)?;
    eprintln!("palmrest: recorded {n} reads");
    Ok(())
}
