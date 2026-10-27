//! `palmrest record FILE [SECONDS]`.

use crate::daemon::record::{self, Limits};
use crate::error::Error;
use std::path::Path;
use std::time::Duration;

/// A recording stops after this long with no report.
const IDLE: Duration = Duration::from_secs(20);

/// Records the Magic Mouse into `file` for at most `total`.
pub(super) fn record(file: &Path, total: Duration) -> Result<(), Error> {
    let limits = Limits { idle: IDLE, total };
    let n = record::record(file, limits)?;
    eprintln!("palmrest: recorded {n} reads");
    Ok(())
}
