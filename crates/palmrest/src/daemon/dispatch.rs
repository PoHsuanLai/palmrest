//! Carrying out a tracker's outputs: socket messages, virtual device frames, desktop actions.

use super::socket::{SocketHandle, ToSocket};
use crate::action::{Desktop, Effect, plan};
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
            Out::Wheel(w) => emitter.wheel(w),
            Out::Key(k) => emitter.key(k),
            Out::Action(a) => match plan(a, desktop) {
                Effect::Key(k) => emitter.key(&k),
                Effect::Spawn(argv) => {
                    spawn(argv);
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

/// Runs a program without waiting on the reader thread; a helper thread reaps it.
fn spawn(argv: Vec<String>) {
    let Some((program, args)) = argv.split_first() else {
        return;
    };
    match std::process::Command::new(program).args(args).spawn() {
        Ok(mut child) => {
            let _ = std::thread::Builder::new()
                .name("palmrest-action".to_owned())
                .spawn(move || child.wait());
        }
        Err(e) => eprintln!("palmrest: {program}: {e}"),
    }
}
