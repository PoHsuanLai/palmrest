//! `palmrest --check-uinput`: whether this machine can make palmrest's virtual devices.

use crate::daemon::uinput::{self, LiveRoute, Origin};
use crate::error::Error;

/// Opens a session, makes the touchpad and lets go of both.
pub(super) fn check_uinput(origin: Origin) -> Result<(), Error> {
    uinput::probe(&LiveRoute::live(origin))?;
    println!("palmrest can make its virtual devices");
    Ok(())
}
