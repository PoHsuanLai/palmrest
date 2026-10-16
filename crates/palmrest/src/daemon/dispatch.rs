//! Carrying out a tracker's outputs: socket messages, virtual device frames, desktop actions.

use super::socket::{SocketHandle, ToSocket};
use super::spawn::Spawner;
use crate::action::{Desktop, Effect, plan};
use crate::outputs::Emitter;
use crate::tracker::Out;

/// Performs every output, in order. Device write failures are logged and dropped: a missed
/// frame is better than a stalled reader.
pub fn dispatch(
    outs: Vec<Out>,
    socket: &SocketHandle,
    emitter: &mut dyn Emitter,
    spawner: &Spawner,
    desktop: Desktop,
) {
    for o in outs {
        let result = match &o {
            Out::Host(m) => {
                socket.send(ToSocket::Msg(m.clone()));
                Ok(())
            }
            Out::Pad(f) => emitter.pad(f),
            Out::Wheel(w) => emitter.wheel(w),
            Out::Key(k) => emitter.key(k),
            Out::Action(a) => match plan(a, desktop) {
                Effect::Key(k) => emitter.key(&k),
                Effect::Spawn(argv) => {
                    spawner.run(&argv);
                    Ok(())
                }
                Effect::Nothing => Ok(()),
            },
        };
        if let Err(e) = result {
            eprintln!("palmrest: output failed: {e}");
        }
    }
}
