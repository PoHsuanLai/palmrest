//! Everything with an effect: threads, the socket, the hidraw node, the virtual devices, the
//! clock. The pure core never reaches in here.

pub mod devices;
pub mod discover;
pub mod dispatch;
pub mod hidraw;
pub mod latency;
pub mod reader;
pub mod record;
pub mod socket;

use crate::action::Desktop;
use crate::config::{self, Loaded};
use crate::outputs::Emitter;
use crate::outputs::uinput::UinputEmitter;
use crate::tracker::Tracker;
use devices::{Devices, Factory};
use std::path::PathBuf;
use std::sync::mpsc;

/// Runs the daemon: never returns unless setup fails.
pub fn run() -> Result<(), String> {
    let env = |k: &str| std::env::var(k).ok();
    let dir = config::config_dir(env);
    let loaded: Loaded = config::load(dir.as_deref(), env);
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
    let runtime = env("XDG_RUNTIME_DIR")
        .filter(|s| !s.is_empty())
        .ok_or("XDG_RUNTIME_DIR is not set")?;
    let path = socket::default_path(&PathBuf::from(runtime));
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
        desktop: Desktop::from_env_value(&env("XDG_CURRENT_DESKTOP").unwrap_or_default()),
        tracker: Tracker::new(loaded.config),
    };
    reader.run();
    Ok(())
}
