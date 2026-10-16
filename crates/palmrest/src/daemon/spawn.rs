//! Programs a gesture runs (a mapped command, `sill workspace ...`): started without waiting
//! for them, and reaped by one thread, so a burst of gestures never costs a thread each and a
//! failed thread spawn cannot leave a zombie.

use std::io;
use std::process::Child;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

/// How often running programs are checked for having exited.
const REAP_EVERY: Duration = Duration::from_millis(500);

/// Starts programs and hands them to the reaper.
#[derive(Debug, Clone)]
pub struct Spawner {
    children: Sender<Child>,
}

impl Spawner {
    /// Starts the reaper thread.
    pub fn start() -> io::Result<Self> {
        let (children, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("palmrest-reaper".to_owned())
            .spawn(move || reap(rx))?;
        Ok(Self { children })
    }

    /// Runs `argv` (program first) without waiting; an empty list does nothing.
    pub fn run(&self, argv: &[String]) {
        let Some((program, args)) = argv.split_first() else {
            return;
        };
        match std::process::Command::new(program).args(args).spawn() {
            Ok(child) => {
                let _ = self.children.send(child);
            }
            Err(e) => eprintln!("palmrest: {program}: {e}"),
        }
    }
}

/// Waits for the programs it is given, all of them from this one thread. It sleeps until the
/// first arrives, polls while any runs, and ends when the spawner is gone and none is left.
fn reap(rx: Receiver<Child>) {
    let mut running: Vec<Child> = Vec::new();
    loop {
        let next = if running.is_empty() {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            rx.recv_timeout(REAP_EVERY)
        };
        match next {
            Ok(child) => running.push(child),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) if running.is_empty() => return,
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(REAP_EVERY),
        }
        running.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_program_is_reaped_and_a_missing_one_is_only_logged() {
        let (children, rx) = mpsc::channel();
        let spawner = Spawner { children };
        spawner.run(&[]);
        spawner.run(&["/nonexistent/palmrest-test-program".to_owned()]);
        spawner.run(&["true".to_owned()]);
        drop(spawner);
        let child = rx.try_recv().expect("the one program that exists");
        assert!(rx.try_recv().is_err(), "nothing else was started");
        let mut running = vec![child];
        for _ in 0..200 {
            running.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
            if running.is_empty() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("`true` never exited");
    }
}
