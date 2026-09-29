//! `palmrest --check-uinput`: whether this user may create virtual devices.

use crate::daemon::Env;
use crate::daemon::uinput::{self, UINPUT};
use std::path::Path;

/// Opens and closes the uinput node when the environment permits it.
pub(super) fn check_uinput(env: &Env) -> Result<(), String> {
    uinput::probe(env.probe, Path::new(UINPUT)).map_err(|e| e.to_string())?;
    println!("uinput preflight OK: {UINPUT} can be opened for writing");
    Ok(())
}
