//! Everything with an effect: threads, the socket, the sensor's node (hidraw or evdev), the
//! virtual devices, the clock. The pure core never reaches in here.

pub mod devices;
pub mod discover;
pub mod dispatch;
mod env;
pub mod evdev;
pub mod hidraw;
pub mod latency;
pub mod reader;
pub mod record;
pub mod socket;
pub mod source;
pub(crate) mod sys;
pub mod uinput;

use crate::config::{self, Loaded};
use crate::outputs::Emitter;
use crate::tracker::Tracker;
use devices::{Devices, Factory};
pub use env::Env;
use std::sync::mpsc;
use uinput::UinputEmitter;

/// Runs the daemon: never returns unless setup fails.
pub fn run(env: &Env) -> Result<(), String> {
    let dir = env.config_dir.clone();
    let loaded: Loaded = config::load(dir.as_deref());
    loaded
        .warnings
        .iter()
        .for_each(|w| eprintln!("palmrest: config: {w}"));
    if let Some(src) = &loaded.source {
        eprintln!("palmrest: config from {}", src.display());
    }
    let (config_tx, config_rx) = mpsc::channel();
    let _watcher = match dir {
        Some(d) => {
            Some(config::watch::spawn(d, config_tx).map_err(|e| format!("config watch: {e}"))?)
        }
        None => None,
    };
    let runtime = env
        .runtime_dir
        .as_deref()
        .ok_or("XDG_RUNTIME_DIR is not set")?;
    let path = socket::default_path(runtime);
    let listener = socket::bind(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (claims_tx, claims_rx) = mpsc::channel();
    let socket = socket::spawn(listener, claims_tx).map_err(|e| format!("socket thread: {e}"))?;
    eprintln!("palmrest: listening on {}", path.display());
    let factory: Factory = Box::new(|foreign, combos| {
        Ok(Box::new(UinputEmitter::create(foreign, combos)?) as Box<dyn Emitter>)
    });
    let now = hidraw::now();
    let reader = reader::Reader {
        socket,
        claims: claims_rx,
        configs: config_rx,
        devices: Devices::new(factory, &loaded.config, now),
        desktop: env.desktop,
        tracker: Tracker::new(loaded.config),
    };
    reader.run();
    Ok(())
}
