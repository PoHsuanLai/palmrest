//! Carrying out a tracker's outputs: socket messages, virtual device frames, and the actions that
//! are keys or are asked of the shell.

use super::socket::{SocketHandle, ToSocket};
use crate::action::{Desktop, plan};
use crate::outputs::Emitter;
use crate::tracker::Out;

/// Performs every output, in order. Device write failures are logged and dropped: a missed
/// frame is better than a stalled reader.
pub fn dispatch(
    outs: Vec<Out>,
    socket: &SocketHandle,
    emitter: &mut dyn Emitter,
    desktop: Desktop,
) {
    for o in outs {
        let result = match &o {
            Out::Host(m) => {
                socket.send(ToSocket::Msg(m.clone()));
                Ok(())
            }
            Out::Pad(f) => emitter.pad(f),
            Out::Key(k) => emitter.key(k),
            Out::Deferred(action) => {
                if let Some(host) = plan(action, desktop) {
                    socket.send(ToSocket::Run(host));
                }
                Ok(())
            }
        };
        if let Err(e) = result {
            eprintln!("palmrest: output failed: {e}");
        }
    }
}
