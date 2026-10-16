//! `palmrest --check-uinput`: whether this machine can make palmrest's virtual devices.

use crate::daemon::uinput::{self, LiveRoute, Origin};

/// Opens a session, makes the touchpad and lets go of both.
pub(super) fn check_uinput(origin: Origin) -> Result<(), String> {
    uinput::probe(&LiveRoute::live(origin)).map_err(|e| e.to_string())?;
    println!("palmrest can make its virtual devices");
    Ok(())
}
