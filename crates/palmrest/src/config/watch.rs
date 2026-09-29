//! Live reload: a directory watch with a 30 ms debounce (22-SETTINGS §2).
//!
//! The directory, not the file, is watched: editors and the Settings app's atomic writer both
//! replace the inode by rename, which a file watch would lose.

use super::load::{FILE_NAME, Loaded, load};
use notify::{RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

/// Quiet time after the last change before the file is re-read.
pub const DEBOUNCE: Duration = Duration::from_millis(30);

/// Whether a changed path is our file.
pub fn concerns_us(p: &Path) -> bool {
    p.file_name().is_some_and(|n| n == FILE_NAME)
}

/// Watches `dir` on a thread of its own and sends every settled reload to `out`. Returns the
/// watcher, which must be kept alive; dropping it ends the thread.
pub fn spawn(dir: PathBuf, out: Sender<Loaded>) -> notify::Result<notify::RecommendedWatcher> {
    std::fs::create_dir_all(&dir).map_err(notify::Error::io)?;
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |ev: notify::Result<notify::Event>| {
        if let Ok(ev) = ev
            && ev.paths.iter().any(|p| concerns_us(p))
        {
            let _ = tx.send(());
        }
    })?;
    watcher.watch(&dir, RecursiveMode::NonRecursive)?;
    std::thread::Builder::new()
        .name("palmrest-config".to_owned())
        .spawn(move || debounce_loop(&rx, &dir, &out))
        .map_err(notify::Error::io)?;
    Ok(watcher)
}

fn debounce_loop(rx: &mpsc::Receiver<()>, dir: &Path, out: &Sender<Loaded>) {
    while rx.recv().is_ok() {
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(()) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        if out.send(load(Some(dir))).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_file_name_matters() {
        assert!(concerns_us(Path::new("/c/palmrest/gestures.toml")));
        assert!(!concerns_us(Path::new("/c/palmrest/config.toml")));
        assert!(!concerns_us(Path::new("/c/palmrest/gestures.toml.swp")));
        assert!(!concerns_us(Path::new("/c/palmrest/.gestures.toml.tmp")));
    }
}
