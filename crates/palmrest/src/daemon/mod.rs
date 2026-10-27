//! Everything with an effect: threads, the socket, the sensor's node (hidraw or evdev), the
//! virtual devices, the clock. The pure core never reaches in here.

pub mod devices;
pub mod discover;
pub mod dispatch;
mod env;
pub mod evdev;
pub mod hidioc;
pub mod hidraw;
pub mod latency;
pub mod reader;
pub mod record;
pub mod socket;
pub mod source;
pub(crate) mod sys;
pub mod uinput;

use crate::config::{self, Loaded};
use crate::error::Error;
use crate::outputs::Emitter;
use crate::tracker::Tracker;
use devices::{Devices, Factory};
pub use env::Env;
use palmrest_protocol::LockState;
use std::sync::mpsc;
use uinput::{LiveRoute, Origin, UinputEmitter};

/// Runs the daemon, making its virtual devices from `origin`: never returns unless setup fails.
pub fn run(env: &Env, origin: Origin) -> Result<(), Error> {
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
        Some(d) => Some(config::watch::spawn(d, config_tx).map_err(Error::ConfigWatch)?),
        None => None,
    };
    let path = socket::default_path(&env.runtime_dir);
    let listener = socket::bind(&path).map_err(|source| Error::SocketBind {
        path: path.clone(),
        source,
    })?;
    let (notes_tx, notes_rx) = mpsc::channel();
    let socket = socket::spawn(listener, notes_tx, env.seat_file.clone(), env.steering)
        .map_err(Error::SocketThread)?;
    eprintln!("palmrest: listening on {}", path.display());
    let route = LiveRoute::live(origin);
    let factory: Factory = Box::new(move |combos| {
        Ok(Box::new(UinputEmitter::create(&route, combos)?) as Box<dyn Emitter>)
    });
    let now = hidraw::now();
    let reader = reader::Reader {
        socket,
        notes: notes_rx,
        configs: config_rx,
        devices: Devices::new(factory, &loaded.config, now),
        desktop: env.desktop,
        // Locked until the shell says otherwise: a gesture runs no key or desktop action on a
        // session whose state nobody has reported.
        tracker: Tracker::new(loaded.config).with_session_lock(LockState::Locked),
        system: loaded.text,
        user: None,
    };
    reader.run();
    Ok(())
}
